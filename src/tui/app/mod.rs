//! Application state and event handling (TEA/Elm architecture).

use crate::chapters::CHAPTERS;
use crate::error::Result;
use crate::search;

mod app_state;
pub use app_state::{
    ExerciseCtx, HeaderCache, OverlayState, PiscineCtx, PiscineTimerCache, ProgressCtx, SessionCtx,
};

mod handlers_overlay;
mod handlers_piscine;
mod handlers_watch;

pub use crate::tui::ui_messages::{ActiveOverlay, ListDisplayItem, Msg};
pub use handlers_piscine::format_remaining_secs;

/// État centralisé TEA/Elm — gère tous les modes (watch/piscine/exam/run).
#[derive(Debug)]
pub struct AppState {
    /// Contexte exercice (progression, résultats, indices).
    pub ex: ExerciseCtx,
    /// Contexte piscine/examen (timer, fail count).
    pub piscine: PiscineCtx,
    /// Contexte session runtime (flags, messages, éditeur).
    pub session: SessionCtx,
    /// Contexte progression (mastery, révisions, curriculum).
    pub progress: ProgressCtx,
    /// État des overlays (help, list, search, solution, visualizer, libsys, nav_confirm).
    pub overlay: OverlayState,
    /// Cache du header (mini-map, counter, mastery display).
    pub header_cache: HeaderCache,
    /// Cache du timer piscine (elapsed, remaining).
    pub timer_cache: PiscineTimerCache,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            ex: ExerciseCtx::default(),
            piscine: PiscineCtx::default(),
            session: SessionCtx::default(),
            progress: ProgressCtx::default(),
            overlay: OverlayState::default(),
            header_cache: HeaderCache::default(),
            timer_cache: PiscineTimerCache::default(),
        }
    }

    /// Nombre de sujets dont la révision est due (days_until_due ≤ 0).
    /// Uses cached value if available.
    pub fn due_count(&self) -> usize {
        if let Some(cached) = self.progress.cached_due_count {
            return cached;
        }
        self.progress
            .review_map
            .values()
            .filter(|v| v.map(|d| d <= 0).unwrap_or(false))
            .count()
    }

    /// Returns the current exercise, or None if the index is out of bounds.
    pub fn current_ex(&self) -> Option<&crate::models::Exercise> {
        self.ex.exercises.get(self.ex.current_index)
    }

    /// Cache due_count calculation (call once per frame in dispatch loop).
    pub fn update_due_count_cache(&mut self) {
        let count = self
            .progress
            .review_map
            .values()
            .filter(|v| v.map(|d| d <= 0).unwrap_or(false))
            .count();
        self.progress.cached_due_count = Some(count);
    }
}

#[derive(Debug)]
pub struct App {
    pub state: AppState,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
        }
    }

    /// Invalide les caches de header. Appeler après changement d'exercice ou mise à jour mastery.
    /// Optimisé : ne recompute que si les valeurs de tracking ont changé.
    fn invalidate_header_cache(state: &mut AppState) {
        let idx = state.ex.current_index;
        let total = state.ex.exercises.len();
        let mastery = state
            .progress
            .mastery_map
            .get(
                state
                    .ex
                    .exercises
                    .get(idx)
                    .map(|e| e.subject.as_str())
                    .unwrap_or(""),
            )
            .copied()
            .unwrap_or(0.0);

        state.header_cache.invalidate(
            idx,
            total,
            mastery,
            &state.ex.exercises,
            &state.ex.completed,
        );
    }

    /// Boucle principale watch avec Ratatui.
    ///
    /// Paramètres :
    /// - `terminal` : terminal Ratatui initialisé
    /// - `conn` : connexion SQLite (pour compile_and_run + record_attempt)
    pub fn run_watch(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
        conn: &rusqlite::Connection,
    ) -> Result<()> {
        use std::time::Duration;

        // Prépare le premier exercice
        self.reset_state_and_load_exercise(conn)?;

        let rx = match &self.state.ex.source_path {
            Some(p) => crate::tui::events::spawn_event_reader(p.clone()),
            None => return Ok(()),
        };

        // Boucle TEA
        loop {
            self.state.update_due_count_cache();
            terminal
                .draw(|f| crate::tui::ui_watch::view(f, &mut self.state))
                .inspect_err(|e| {
                    eprintln!("[clings] draw error: {e}");
                    ratatui::restore();
                })?;

            match rx.recv_timeout(Duration::from_millis(
                crate::constants::RENDER_RECV_TIMEOUT_MS,
            )) {
                Ok(msg) => self.update_watch(msg, conn),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if self.state.session.should_quit {
                break;
            }
        }
        Ok(())
    }

    /// Load current exercise and reset all related state.
    /// Resets: hint_index, overlay states, status_msg, run_result, description_scroll,
    /// consecutive_failures, already_recorded, vis state, etc.
    pub fn reset_state_and_load_exercise(&mut self, conn: &rusqlite::Connection) -> Result<()> {
        let idx = self.state.ex.current_index;
        if idx >= self.state.ex.exercises.len() {
            self.state.session.should_quit = true;
            return Ok(());
        }
        let exercise = &self.state.ex.exercises[idx];
        self.state.session.skip_file_changed = true;
        let (source_path, stage) = crate::runner::prepare_exercise_source(conn, exercise)?;

        // Restart watcher if we change exercise
        self.state.ex.source_path = Some(source_path);
        self.state.ex.current_stage = stage;
        self.state.ex.run_result = None;
        self.state.ex.hint_index = 0;
        self.state.overlay.active = ActiveOverlay::None;
        self.state.session.compile_pending = false;
        self.state.overlay.vis_step = 0;
        self.state.ex.already_recorded = false;
        self.state.ex.consecutive_failures = 0;
        self.state.session.status_msg = None;
        self.state.ex.description_scroll = 0;

        // Open/update neovim pane in tmux
        if let Some(ref path) = self.state.ex.source_path {
            let pane =
                crate::tmux::update_editor_pane(self.state.session.editor_pane.as_deref(), path);
            self.state.session.editor_pane = pane;
        }

        Self::invalidate_header_cache(&mut self.state);
        Ok(())
    }

    /// Recompute search results from current query (indices into state.ex.exercises).
    fn rebuild_search(state: &mut AppState) {
        let subject_filter = if state.overlay.search_subject_filter {
            state
                .ex
                .exercises
                .get(state.ex.current_index)
                .map(|ex| ex.subject.as_str())
        } else {
            None
        };
        if state.overlay.search_query.is_empty() && subject_filter.is_none() {
            state.overlay.search_results = (0..state.ex.exercises.len()).collect();
        } else if state.overlay.search_query.is_empty() {
            state.overlay.search_results = state
                .ex
                .exercises
                .iter()
                .enumerate()
                .filter(|(_, ex)| subject_filter.is_none_or(|s| ex.subject == s))
                .map(|(i, _)| i)
                .collect();
        } else {
            state.overlay.search_results = search::search_exercises(
                &state.ex.exercises,
                &state.overlay.search_query,
                subject_filter,
            )
            .into_iter()
            .map(|(idx, _score)| idx)
            .collect();
        }
        state.overlay.search_selected = 0;
    }

    /// Reconstruit `list_display_items` depuis `state.ex.exercises` et `CHAPTERS`.
    fn build_list_display_items(state: &mut AppState) {
        // Pass 1: count exercises per chapter
        let mut chapter_counts: std::collections::HashMap<Option<usize>, (usize, usize)> =
            std::collections::HashMap::new();
        for (ex_idx, ex) in state.ex.exercises.iter().enumerate() {
            let ch_idx = state
                .progress
                .subject_to_chapter
                .get(ex.subject.as_str())
                .copied();
            let entry = chapter_counts.entry(ch_idx).or_insert((0, 0));
            entry.0 += 1;
            if state.ex.completed.get(ex_idx).copied().unwrap_or(false) {
                entry.1 += 1;
            }
        }

        // Pass 2: build display items
        state.overlay.list_display_items.clear();
        let mut current_chapter: Option<usize> = None;

        for (ex_idx, ex) in state.ex.exercises.iter().enumerate() {
            let ch_idx = state
                .progress
                .subject_to_chapter
                .get(ex.subject.as_str())
                .copied();

            if ch_idx != current_chapter {
                current_chapter = ch_idx;
                let (number, title) = match ch_idx {
                    Some(i) => (CHAPTERS[i].number, CHAPTERS[i].title),
                    None => (0, "Divers"),
                };
                let (count, done) = chapter_counts.get(&ch_idx).copied().unwrap_or((0, 0));
                state
                    .overlay
                    .list_display_items
                    .push(ListDisplayItem::ChapterHeader {
                        chapter_number: number,
                        title,
                        exercise_count: count,
                        done_count: done,
                    });
            }
            state
                .overlay
                .list_display_items
                .push(ListDisplayItem::Exercise {
                    exercise_index: ex_idx,
                });
        }
    }

    /// Trouve le prochain item Exercise dans `list_display_items` en avançant depuis `from` (wrap).
    fn next_exercise_item(items: &[ListDisplayItem], from: usize, forward: bool) -> usize {
        let len = items.len();
        if len == 0 {
            return 0;
        }
        let mut pos = from;
        for _ in 0..len {
            pos = if forward {
                (pos + 1) % len
            } else {
                pos.checked_sub(1).unwrap_or(len - 1)
            };
            if matches!(items[pos], ListDisplayItem::Exercise { .. }) {
                return pos;
            }
        }
        from
    }

    /// Find the first exercise of the next chapter, starting from `from`.
    /// Returns None if no next chapter exists.
    fn find_next_chapter_exercise(items: &[ListDisplayItem], from: usize) -> Option<usize> {
        let mut found_next_header = false;
        for (i, item) in items.iter().enumerate() {
            if i > from && !found_next_header {
                if matches!(item, ListDisplayItem::ChapterHeader { .. }) {
                    found_next_header = true;
                }
            } else if found_next_header && matches!(item, ListDisplayItem::Exercise { .. }) {
                return Some(i);
            }
        }
        None
    }

    /// Find the first exercise of the previous chapter, starting from `from`.
    /// Returns None if no previous chapter exists.
    fn find_prev_chapter_exercise(items: &[ListDisplayItem], from: usize) -> Option<usize> {
        // Find current chapter header (scan backward from `from`)
        let mut current_ch_header = None;
        for i in (0..=from).rev() {
            if matches!(items[i], ListDisplayItem::ChapterHeader { .. }) {
                current_ch_header = Some(i);
                break;
            }
        }
        // Find previous chapter header
        if let Some(ch_pos) = current_ch_header {
            if ch_pos > 0 {
                for i in (0..ch_pos).rev() {
                    if matches!(items[i], ListDisplayItem::ChapterHeader { .. }) {
                        // Jump to first exercise after this header
                        if i + 1 < items.len()
                            && matches!(items[i + 1], ListDisplayItem::Exercise { .. })
                        {
                            return Some(i + 1);
                        }
                    }
                }
            }
        }
        None
    }

    /// Boucle principale piscine avec Ratatui.
    ///
    /// Paramètres :
    /// - `terminal` : terminal Ratatui initialisé
    /// - `conn` : connexion SQLite (pour compile_and_run + record_attempt)
    /// - `session_id` : optional exam session ID (for exam checkpoints)
    pub fn run_piscine(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
    ) -> Result<()> {
        use std::time::Duration;

        // Initialise le cache timer piscine pour le premier frame
        if let Some(start) = self.state.piscine.start {
            let elapsed = start.elapsed().as_secs();
            self.state.timer_cache.piscine_last_elapsed_secs = elapsed;
            self.state.timer_cache.cached_piscine_elapsed_str =
                format!("⏱ {}m{:02}s", elapsed / 60, elapsed % 60);
        }
        if let Some(deadline) = self.state.piscine.deadline {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            self.state.timer_cache.piscine_last_remaining_secs = remaining;
            self.state.timer_cache.cached_piscine_remaining_str = format_remaining_secs(remaining);
        }

        // Prépare le premier exercice
        self.reset_state_and_load_exercise(conn)?;

        let rx = match &self.state.ex.source_path {
            Some(p) => crate::tui::events::spawn_event_reader(p.clone()),
            None => return Ok(()),
        };

        // Boucle TEA
        loop {
            terminal
                .draw(|f| crate::tui::ui_piscine::view(f, &mut self.state))
                .inspect_err(|e| {
                    eprintln!("[clings] draw error: {e}");
                    ratatui::restore();
                })?;

            match rx.recv_timeout(Duration::from_millis(
                crate::constants::RENDER_RECV_TIMEOUT_MS,
            )) {
                Ok(msg) => self.update_piscine(msg, conn, session_id),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Check deadline on timeout
                    if let Some(deadline) = self.state.piscine.deadline {
                        if std::time::Instant::now() >= deadline {
                            self.state.session.should_quit = true;
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if self.state.session.should_quit {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_exercise(hints: Vec<String>, has_vis: bool) -> crate::models::Exercise {
        use crate::models::*;
        Exercise {
            id: "test_01".to_string(),
            subject: "pointers".to_string(),
            lang: Lang::C,
            difficulty: Difficulty::Easy,
            title: "Test Exercise".to_string(),
            description: "desc".to_string(),
            starter_code: String::new(),
            solution_code: String::new(),
            hints,
            validation: ValidationConfig {
                mode: ValidationMode::Output,
                expected_output: Some("ok".to_string()),
                max_duration_ms: None,
                test_code: None,
                expected_tests_pass: None,
            },
            prerequisites: vec![],
            starter_code_stages: vec![],
            files: vec![],
            exercise_type: ExerciseType::Complete,
            key_concept: None,
            common_mistake: None,
            kc_ids: vec![],
            visualizer: if has_vis {
                Visualizer {
                    vis_type: String::new(),
                    steps: vec![VisStep {
                        step_label: "s1".to_string(),
                        label: "l1".to_string(),
                        explanation: "e1".to_string(),
                        stack: vec![],
                        heap: vec![],
                        arrows: vec![],
                        call_frames: vec![],
                    }],
                }
            } else {
                Visualizer::default()
            },
            libsys_module: None,
            libsys_function: None,
            libsys_unlock: None,
            header_code: None,
        }
    }

    #[test]
    fn overlay_state_defaults_inactive() {
        let o = OverlayState::default();
        assert_eq!(o.active, ActiveOverlay::None);
        assert_eq!(o.vis_step, 0);
        assert!(o.search_query.is_empty());
        assert!(o.search_results.is_empty());
    }

    #[test]
    fn header_cache_defaults_empty() {
        let h = HeaderCache::default();
        assert!(h.cached_mini_map.is_empty());
        assert_eq!(h.cached_header_left_len, 0);
    }

    #[test]
    fn piscine_timer_cache_defaults_max() {
        let t = PiscineTimerCache::default();
        assert_eq!(t.piscine_last_elapsed_secs, u64::MAX);
        assert_eq!(t.piscine_last_remaining_secs, u64::MAX);
    }

    #[test]
    fn can_reveal_solution_all_hints_shown() {
        let mut app = App::new();
        let ex = make_exercise(vec!["h1".into(), "h2".into()], false);
        app.state.ex.exercises = vec![ex];
        app.state.ex.completed = vec![false];
        app.state.ex.hint_index = 2; // all shown
        app.state.ex.consecutive_failures = 0;
        assert!(app.can_reveal_solution(3));
    }

    #[test]
    fn can_reveal_solution_enough_failures() {
        let mut app = App::new();
        let ex = make_exercise(vec!["h1".into(), "h2".into()], false);
        app.state.ex.exercises = vec![ex];
        app.state.ex.completed = vec![false];
        app.state.ex.hint_index = 0; // none shown
        app.state.ex.consecutive_failures = 3;
        assert!(app.can_reveal_solution(3));
    }

    #[test]
    fn can_reveal_solution_not_enough() {
        let mut app = App::new();
        let ex = make_exercise(vec!["h1".into(), "h2".into()], false);
        app.state.ex.exercises = vec![ex];
        app.state.ex.completed = vec![false];
        app.state.ex.hint_index = 1; // partial
        app.state.ex.consecutive_failures = 1;
        assert!(!app.can_reveal_solution(3));
    }

    #[test]
    fn can_reveal_solution_no_hints() {
        let mut app = App::new();
        let ex = make_exercise(vec![], false);
        app.state.ex.exercises = vec![ex];
        app.state.ex.completed = vec![false];
        app.state.ex.hint_index = 0;
        app.state.ex.consecutive_failures = 0;
        assert!(app.can_reveal_solution(3));
    }

    #[test]
    fn reveal_next_hint_increments() {
        let mut app = App::new();
        let ex = make_exercise(vec!["h1".into(), "h2".into()], false);
        app.state.ex.exercises = vec![ex];
        app.state.ex.completed = vec![false];
        assert_eq!(app.state.ex.hint_index, 0);

        // Gate : hint 1 nécessite HINT_MIN_ATTEMPTS tentatives — sans tentatives, bloqué
        app.reveal_next_hint();
        assert_eq!(app.state.ex.hint_index, 0, "gate bloque sans tentatives");

        // Simuler HINT_MIN_ATTEMPTS échecs pour débloquer
        app.state.ex.consecutive_failures = crate::constants::HINT_MIN_ATTEMPTS;
        app.reveal_next_hint();
        assert_eq!(app.state.ex.hint_index, 1);
        app.reveal_next_hint();
        assert_eq!(app.state.ex.hint_index, 2);
        // Should not exceed hints length
        app.reveal_next_hint();
        assert_eq!(app.state.ex.hint_index, 2);
    }

    #[test]
    fn toggle_visualizer_overlay_activates() {
        let mut app = App::new();
        let ex = make_exercise(vec![], true);
        app.state.ex.exercises = vec![ex];
        app.state.ex.completed = vec![false];
        assert_eq!(app.state.overlay.active, ActiveOverlay::None);
        app.toggle_visualizer_overlay();
        assert_eq!(app.state.overlay.active, ActiveOverlay::Visualizer);
        assert_eq!(app.state.overlay.vis_step, 0);
    }

    #[test]
    fn toggle_visualizer_overlay_noop_without_steps() {
        let mut app = App::new();
        let ex = make_exercise(vec![], false);
        app.state.ex.exercises = vec![ex];
        app.state.ex.completed = vec![false];
        app.toggle_visualizer_overlay();
        assert_eq!(app.state.overlay.active, ActiveOverlay::None);
    }

    #[test]
    fn format_remaining_secs_zero() {
        assert_eq!(format_remaining_secs(0), "Temps écoulé");
    }

    #[test]
    fn format_remaining_secs_under_minute() {
        assert_eq!(format_remaining_secs(42), "42s restantes");
    }

    #[test]
    fn format_remaining_secs_over_minute() {
        assert_eq!(format_remaining_secs(125), "2m05s restantes");
    }

    // ── Tests de transition d'état ───────────────────────────────────────

    #[test]
    fn open_list_overlay_sets_flag() {
        let mut app = App::new();
        app.state.ex.exercises = vec![make_exercise(vec![], false)];
        app.state.ex.completed = vec![false];
        assert_eq!(app.state.overlay.active, ActiveOverlay::None);
        app.open_list_overlay();
        assert_eq!(app.state.overlay.active, ActiveOverlay::List);
    }

    #[test]
    fn open_list_overlay_positions_cursor_on_current() {
        let mut app = App::new();
        app.state.ex.exercises = vec![make_exercise(vec![], false), make_exercise(vec![], false)];
        app.state.ex.completed = vec![false, false];
        app.state.ex.current_index = 1;
        app.open_list_overlay();
        // list_selected doit pointer sur l'item correspondant à current_index
        let selected = app.state.overlay.list_selected;
        let found = app.state.overlay.list_display_items.iter().position(|item| {
            matches!(item, ListDisplayItem::Exercise { exercise_index } if *exercise_index == 1)
        });
        assert_eq!(Some(selected), found);
    }

    #[test]
    fn open_search_overlay_clears_state() {
        let mut app = App::new();
        app.state.overlay.search_query = "ancien".to_string();
        app.state.overlay.search_subject_filter = true;
        app.open_search_overlay();
        assert_eq!(app.state.overlay.active, ActiveOverlay::Search);
        assert!(app.state.overlay.search_query.is_empty());
        assert!(!app.state.overlay.search_subject_filter);
    }

    #[test]
    fn help_overlay_activates_via_overlay_flag() {
        let mut app = App::new();
        assert_eq!(app.state.overlay.active, ActiveOverlay::None);
        app.state.overlay.active = ActiveOverlay::Help;
        assert_eq!(app.state.overlay.active, ActiveOverlay::Help);
    }

    #[test]
    fn msg_resize_variant_is_handled() {
        // Vérifie que Msg::Resize peut être construit et ne panique pas dans un match.
        let msg = Msg::Resize(80, 24);
        match msg {
            Msg::Resize(w, h) => {
                assert_eq!(w, 80);
                assert_eq!(h, 24);
            }
            _ => panic!("mauvaise variante"),
        }
    }

    #[test]
    fn open_search_overlay_idempotent_when_already_active() {
        let mut app = App::new();
        app.open_search_overlay();
        app.state.overlay.search_query = "test".to_string();
        // Deuxième ouverture doit remettre la query à zéro
        app.open_search_overlay();
        assert!(app.state.overlay.search_query.is_empty());
    }
}
