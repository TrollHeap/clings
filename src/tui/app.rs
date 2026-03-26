//! Application state and event handling (TEA/Elm architecture).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::chapters::CHAPTERS;
use crate::error::Result;
use crate::models::Exercise;
use crate::runner::RunResult;
use crate::search;

/// Item dans la liste d'affichage de l'overlay `[l]` — header de chapitre ou exercice.
#[derive(Debug, Clone)]
pub enum ListDisplayItem {
    ChapterHeader {
        chapter_number: u8,
        title: &'static str,
        exercise_count: usize,
        done_count: usize,
    },
    Exercise {
        exercise_index: usize,
    },
}

/// Messages traités par App::update_watch()
#[derive(Debug)]
pub enum Msg {
    Key(ratatui::crossterm::event::KeyEvent),
    FileChanged,
    Tick,
}

/// État des overlays (help, list, search, solution, visualizer).
#[derive(Default)]
pub struct OverlayState {
    pub help_active: bool,
    pub list_active: bool,
    pub list_selected: usize,
    pub list_display_items: Vec<ListDisplayItem>,
    pub search_active: bool,
    pub search_query: String,
    pub search_results: Vec<usize>,
    pub search_selected: usize,
    pub search_subject_filter: bool,
    pub search_g_pending: bool,
    pub solution_active: bool,
    pub vis_active: bool,
    pub vis_step: usize,
    pub success_overlay: bool,
}

/// Cache du header — invalidé sur changement d'exercice ou mise à jour mastery.
#[derive(Default)]
pub struct HeaderCache {
    pub cached_mini_map: String,
    pub cached_exercise_counter: String,
    pub cached_mastery_display: String,
    pub cached_exercise_type: String,
    pub cached_header_left_len: usize,
    pub cached_mini_map_len: usize,
}

/// Cache du timer piscine — mis à jour dans Tick quand la seconde change.
pub struct PiscineTimerCache {
    pub cached_piscine_elapsed_str: String,
    pub piscine_last_elapsed_secs: u64,
    pub cached_piscine_remaining_str: String,
    pub piscine_last_remaining_secs: u64,
}

impl Default for PiscineTimerCache {
    fn default() -> Self {
        Self {
            cached_piscine_elapsed_str: String::new(),
            piscine_last_elapsed_secs: u64::MAX,
            cached_piscine_remaining_str: String::new(),
            piscine_last_remaining_secs: u64::MAX,
        }
    }
}

/// État centralisé — mode watch
pub struct AppState {
    pub should_quit: bool,
    // Watch data
    pub exercises: Vec<Exercise>,
    pub completed: Vec<bool>,
    pub current_index: usize,
    pub run_result: Option<RunResult>,
    pub source_path: Option<PathBuf>,
    pub current_stage: Option<u8>,
    pub editor_pane: Option<String>,
    pub hint_index: usize,
    pub compile_pending: bool,
    pub consecutive_failures: u8,
    pub already_recorded: bool,
    pub review_map: HashMap<String, Option<i64>>,
    pub mastery_map: HashMap<String, f64>,
    pub piscine_deadline: Option<Instant>,
    pub piscine_start: Option<Instant>,
    pub piscine_timer_total: u64,
    pub piscine_fail_count: u32,
    // Status message (file saved, etc.)
    pub status_msg: Option<String>,
    pub status_msg_at: Option<Instant>,
    pub skip_file_changed: bool,
    // Subject order cache (filled once on init)
    pub subject_order: Vec<String>,
    // Description panel scroll offset
    pub description_scroll: u16,
    // Sub-structs
    pub overlay: OverlayState,
    pub header_cache: HeaderCache,
    pub timer_cache: PiscineTimerCache,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            exercises: Vec::new(),
            completed: Vec::new(),
            current_index: 0,
            run_result: None,
            source_path: None,
            current_stage: None,
            editor_pane: None,
            hint_index: 0,
            compile_pending: false,
            consecutive_failures: 0,
            already_recorded: false,
            review_map: HashMap::new(),
            mastery_map: HashMap::new(),
            piscine_deadline: None,
            piscine_start: None,
            piscine_timer_total: 0,
            piscine_fail_count: 0,
            status_msg: None,
            status_msg_at: None,
            skip_file_changed: false,
            subject_order: Vec::new(),
            description_scroll: 0,
            overlay: OverlayState::default(),
            header_cache: HeaderCache::default(),
            timer_cache: PiscineTimerCache::default(),
        }
    }

    /// Nombre de sujets dont la révision est due (days_until_due ≤ 0).
    pub fn due_count(&self) -> usize {
        self.review_map
            .values()
            .filter(|v| v.map(|d| d <= 0).unwrap_or(false))
            .count()
    }
}

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
    fn invalidate_header_cache(state: &mut AppState) {
        let idx = state.current_index;
        let total = state.exercises.len();
        state.header_cache.cached_exercise_counter = format!("[{}/{}] ", idx + 1, total);
        let mastery = state
            .mastery_map
            .get(
                state
                    .exercises
                    .get(idx)
                    .map(|e| e.subject.as_str())
                    .unwrap_or(""),
            )
            .copied()
            .unwrap_or(0.0);
        state.header_cache.cached_mastery_display = format!("mastery: {:.1}  ", mastery);
        state.header_cache.cached_mini_map = crate::tui::common::mini_map(&state.completed, idx);
        state.header_cache.cached_mini_map_len = state.header_cache.cached_mini_map.chars().count();
        state.header_cache.cached_exercise_type = state
            .exercises
            .get(idx)
            .map(|e| e.exercise_type.to_string())
            .unwrap_or_default();

        // Cache the left header width: "[N/total] " + title chars count
        let title = state
            .exercises
            .get(idx)
            .map(|e| e.title.as_str())
            .unwrap_or("");
        state.header_cache.cached_header_left_len =
            state.header_cache.cached_exercise_counter.chars().count() + title.chars().count();
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
        self.load_current_exercise(conn)?;

        let rx = match &self.state.source_path {
            Some(p) => crate::tui::events::spawn_event_reader(p.clone()),
            None => return Ok(()),
        };

        // Boucle TEA
        loop {
            terminal.draw(|f| crate::tui::ui_watch::view(f, &self.state))?;

            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(msg) => self.update_watch(msg, conn),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if self.state.should_quit {
                break;
            }
        }
        Ok(())
    }

    /// Prépare l'exercice courant : source_path + stage.
    pub fn load_current_exercise(&mut self, conn: &rusqlite::Connection) -> Result<()> {
        let idx = self.state.current_index;
        if idx >= self.state.exercises.len() {
            self.state.should_quit = true;
            return Ok(());
        }
        let exercise = &self.state.exercises[idx];
        self.state.skip_file_changed = true;
        let (source_path, stage) = crate::runner::prepare_exercise_source(conn, exercise)?;

        // Restart watcher if we change exercise
        self.state.source_path = Some(source_path);
        self.state.current_stage = stage;
        self.state.run_result = None;
        self.state.hint_index = 0;
        self.state.overlay.solution_active = false;
        self.state.compile_pending = false;
        self.state.overlay.vis_active = false;
        self.state.overlay.vis_step = 0;
        self.state.already_recorded = false;
        self.state.consecutive_failures = 0;
        self.state.status_msg = None;
        self.state.description_scroll = 0;

        // Open/update neovim pane in tmux
        if let Some(ref path) = self.state.source_path {
            let pane = crate::tmux::update_editor_pane(self.state.editor_pane.as_deref(), path);
            self.state.editor_pane = pane;
        }

        Self::invalidate_header_cache(&mut self.state);
        Ok(())
    }

    /// Recompute search results from current query (indices into state.exercises).
    fn rebuild_search(state: &mut AppState) {
        let subject_filter = if state.overlay.search_subject_filter {
            state
                .exercises
                .get(state.current_index)
                .map(|ex| ex.subject.as_str())
        } else {
            None
        };
        if state.overlay.search_query.is_empty() && subject_filter.is_none() {
            state.overlay.search_results = (0..state.exercises.len()).collect();
        } else if state.overlay.search_query.is_empty() {
            state.overlay.search_results = state
                .exercises
                .iter()
                .enumerate()
                .filter(|(_, ex)| subject_filter.is_none_or(|s| ex.subject == s))
                .map(|(i, _)| i)
                .collect();
        } else {
            state.overlay.search_results = search::search_exercises(
                &state.exercises,
                &state.overlay.search_query,
                subject_filter,
            )
            .into_iter()
            .map(|(idx, _score)| idx)
            .collect();
        }
        state.overlay.search_selected = 0;
    }

    /// Reconstruit `list_display_items` depuis `state.exercises` et `CHAPTERS`.
    fn build_list_display_items(state: &mut AppState) {
        let subject_to_chapter: std::collections::HashMap<&str, usize> = CHAPTERS
            .iter()
            .enumerate()
            .flat_map(|(i, ch)| ch.subjects.iter().map(move |&s| (s, i)))
            .collect();

        // Pass 1: count exercises per chapter
        let mut chapter_counts: std::collections::HashMap<Option<usize>, (usize, usize)> =
            std::collections::HashMap::new();
        for (ex_idx, ex) in state.exercises.iter().enumerate() {
            let ch_idx = subject_to_chapter.get(ex.subject.as_str()).copied();
            let entry = chapter_counts.entry(ch_idx).or_insert((0, 0));
            entry.0 += 1;
            if state.completed.get(ex_idx).copied().unwrap_or(false) {
                entry.1 += 1;
            }
        }

        // Pass 2: build display items
        state.overlay.list_display_items.clear();
        let mut current_chapter: Option<usize> = None;

        for (ex_idx, ex) in state.exercises.iter().enumerate() {
            let ch_idx = subject_to_chapter.get(ex.subject.as_str()).copied();

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

    /// Gère les touches de l'overlay liste.
    /// Retourne `true` si Enter a été pressé (jump-to-exercise).
    fn handle_list_key(state: &mut AppState, key: ratatui::crossterm::event::KeyEvent) -> bool {
        use ratatui::crossterm::event::KeyCode;
        let len = state.overlay.list_display_items.len();
        if len == 0 {
            if matches!(
                key.code,
                KeyCode::Esc
                    | KeyCode::Char('l')
                    | KeyCode::Char('L')
                    | KeyCode::Char('q')
                    | KeyCode::Char('Q')
            ) {
                state.overlay.list_active = false;
            }
            return false;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('l') | KeyCode::Char('L') => {
                state.overlay.list_active = false;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                state.overlay.list_active = false;
            }
            KeyCode::Enter => {
                if let Some(ListDisplayItem::Exercise { exercise_index }) = state
                    .overlay
                    .list_display_items
                    .get(state.overlay.list_selected)
                {
                    state.current_index = *exercise_index;
                    state.overlay.list_active = false;
                    return true;
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                state.overlay.list_selected = Self::next_exercise_item(
                    &state.overlay.list_display_items,
                    state.overlay.list_selected,
                    true,
                );
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                state.overlay.list_selected = Self::next_exercise_item(
                    &state.overlay.list_display_items,
                    state.overlay.list_selected,
                    false,
                );
            }
            KeyCode::Tab => {
                let items = &state.overlay.list_display_items;
                if let Some(pos) =
                    Self::find_next_chapter_exercise(items, state.overlay.list_selected)
                {
                    state.overlay.list_selected = pos;
                    return false;
                }
            }
            KeyCode::BackTab => {
                let items = &state.overlay.list_display_items;
                if let Some(pos) =
                    Self::find_prev_chapter_exercise(items, state.overlay.list_selected)
                {
                    state.overlay.list_selected = pos;
                    return false;
                }
            }
            KeyCode::Char('G') => {
                // Last exercise item
                for (i, item) in state.overlay.list_display_items.iter().enumerate().rev() {
                    if matches!(item, ListDisplayItem::Exercise { .. }) {
                        state.overlay.list_selected = i;
                        break;
                    }
                }
            }
            KeyCode::Char('g') => {
                // First exercise item
                for (i, item) in state.overlay.list_display_items.iter().enumerate() {
                    if matches!(item, ListDisplayItem::Exercise { .. }) {
                        state.overlay.list_selected = i;
                        break;
                    }
                }
            }
            _ => {}
        }
        false
    }

    /// Gère les touches de l'overlay de recherche.
    /// Retourne `true` si Enter a été pressé avec un résultat sélectionné
    /// (l'appelant doit alors charger l'exercice et/ou sauvegarder le checkpoint).
    fn handle_search_key(state: &mut AppState, key: ratatui::crossterm::event::KeyEvent) -> bool {
        use ratatui::crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                state.overlay.search_active = false;
                state.overlay.search_query.clear();
                state.overlay.search_results.clear();
            }
            KeyCode::Tab => {
                state.overlay.search_subject_filter = !state.overlay.search_subject_filter;
                Self::rebuild_search(state);
            }
            KeyCode::Enter => {
                if !state.overlay.search_results.is_empty() {
                    let idx = state.overlay.search_results[state.overlay.search_selected];
                    state.current_index = idx;
                    state.overlay.search_active = false;
                    state.overlay.search_query.clear();
                    state.overlay.search_results.clear();
                    return true;
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                let len = state.overlay.search_results.len();
                if len > 0 {
                    state.overlay.search_selected = (state.overlay.search_selected + 1) % len;
                }
                state.overlay.search_g_pending = false;
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                let len = state.overlay.search_results.len();
                if len > 0 {
                    state.overlay.search_selected = state
                        .overlay
                        .search_selected
                        .checked_sub(1)
                        .unwrap_or(len - 1);
                }
                state.overlay.search_g_pending = false;
            }
            KeyCode::Char('G') => {
                let len = state.overlay.search_results.len();
                if len > 0 {
                    state.overlay.search_selected = len - 1;
                }
                state.overlay.search_g_pending = false;
            }
            KeyCode::Char('g') => {
                if state.overlay.search_g_pending {
                    state.overlay.search_selected = 0;
                    state.overlay.search_g_pending = false;
                } else {
                    state.overlay.search_g_pending = true;
                }
            }
            KeyCode::Backspace => {
                state.overlay.search_query.pop();
                Self::rebuild_search(state);
                state.overlay.search_g_pending = false;
            }
            KeyCode::Char(c) => {
                state.overlay.search_g_pending = false;
                state.overlay.search_query.push(c);
                Self::rebuild_search(state);
            }
            _ => {
                state.overlay.search_g_pending = false;
            }
        }
        false
    }

    /// Gère les touches de l'overlay visualiseur.
    fn handle_vis_key(state: &mut AppState, key: ratatui::crossterm::event::KeyEvent) {
        use ratatui::crossterm::event::KeyCode;
        match key.code {
            KeyCode::Right => {
                let total = state.exercises[state.current_index].visualizer.steps.len();
                state.overlay.vis_step = (state.overlay.vis_step + 1).min(total.saturating_sub(1));
            }
            KeyCode::Left => {
                state.overlay.vis_step = state.overlay.vis_step.saturating_sub(1);
            }
            _ => {
                state.overlay.vis_active = false;
            }
        }
    }

    /// Gère les touches de l'overlay solution.
    /// Retourne `true` si l'overlay était actif (l'appelant doit `return`).
    fn handle_solution_overlay(
        state: &mut AppState,
        key: ratatui::crossterm::event::KeyEvent,
    ) -> bool {
        if !state.overlay.solution_active {
            return false;
        }
        if matches!(
            key.code,
            ratatui::crossterm::event::KeyCode::Esc
                | ratatui::crossterm::event::KeyCode::Char('s')
                | ratatui::crossterm::event::KeyCode::Char('S')
        ) {
            state.overlay.solution_active = false;
        }
        true
    }

    /// Sauvegarde le checkpoint piscine et exam (si session_id présent).
    /// Logue les erreurs sans les propager — contexte event loop.
    fn save_checkpoint(&self, conn: &rusqlite::Connection, session_id: Option<&str>, idx: usize) {
        if let Err(e) = crate::progress::save_piscine_checkpoint(conn, idx) {
            eprintln!("[clings] erreur sauvegarde checkpoint piscine: {e}");
        }
        if let Some(sid) = session_id {
            if let Err(e) = crate::progress::save_exam_checkpoint(conn, sid, idx) {
                eprintln!("[clings] erreur sauvegarde checkpoint exam: {e}");
            }
        }
    }

    /// Load exercise and save checkpoint if needed (shared between overlays).
    fn load_exercise_and_checkpoint(
        &mut self,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
    ) {
        if let Err(e) = self.load_current_exercise(conn) {
            eprintln!("[clings] erreur chargement exercice: {e}");
        }
        if let Some(sid) = session_id {
            let idx = self.state.current_index;
            self.save_checkpoint(conn, Some(sid), idx);
        }
    }

    /// Dispatch overlay keys shared between watch and piscine.
    /// Returns `true` if the key was handled by an overlay (caller should `return`).
    /// If an overlay navigation triggers a jump, calls `load_current_exercise`.
    fn handle_overlay_dispatch(
        &mut self,
        key: ratatui::crossterm::event::KeyEvent,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
    ) -> bool {
        if self.state.overlay.success_overlay {
            use ratatui::crossterm::event::KeyCode;
            self.state.overlay.success_overlay = false;
            if matches!(key.code, KeyCode::Enter) {
                if !self.navigate_next(conn, session_id) {
                    self.state.should_quit = true;
                }
            }
            return true;
        }
        if self.state.overlay.list_active {
            if Self::handle_list_key(&mut self.state, key) {
                self.load_exercise_and_checkpoint(conn, session_id);
            }
            return true;
        }
        if self.state.overlay.search_active {
            if Self::handle_search_key(&mut self.state, key) {
                self.load_exercise_and_checkpoint(conn, session_id);
            }
            return true;
        }
        if Self::handle_solution_overlay(&mut self.state, key) {
            return true;
        }
        if self.state.overlay.help_active {
            self.state.overlay.help_active = false;
            return true;
        }
        if self.state.overlay.vis_active {
            Self::handle_vis_key(&mut self.state, key);
            return true;
        }
        false
    }

    /// Shared hint reveal handler `[h]`.
    fn handle_hint_reveal(&mut self) {
        let hints_len = self.state.exercises[self.state.current_index].hints.len();
        self.state.hint_index = (self.state.hint_index + 1).min(hints_len);
    }

    /// Shared visualizer toggle `[v]`.
    fn handle_vis_toggle(&mut self) {
        if !self.state.exercises[self.state.current_index]
            .visualizer
            .steps
            .is_empty()
        {
            self.state.overlay.vis_active = true;
            self.state.overlay.vis_step = 0;
        }
    }

    /// Shared list overlay open `[l]`.
    fn open_list_overlay(&mut self) {
        Self::build_list_display_items(&mut self.state);
        self.state.overlay.list_active = true;
        let ci = self.state.current_index;
        self.state.overlay.list_selected = self
            .state
            .overlay
            .list_display_items
            .iter()
            .position(|item| {
                matches!(item, ListDisplayItem::Exercise { exercise_index } if *exercise_index == ci)
            })
            .unwrap_or(0);
    }

    /// Shared search overlay open `[/]`.
    fn open_search_overlay(&mut self) {
        self.state.overlay.search_active = true;
        self.state.overlay.search_subject_filter = false;
        self.state.overlay.search_query.clear();
        Self::rebuild_search(&mut self.state);
    }

    /// Navigate to next exercise, optionally saving checkpoint.
    /// Returns `true` if navigation happened, `false` if at end.
    fn navigate_next(&mut self, conn: &rusqlite::Connection, session_id: Option<&str>) -> bool {
        let next = self.state.current_index + 1;
        if next < self.state.exercises.len() {
            self.state.current_index = next;
            if let Err(e) = self.load_current_exercise(conn) {
                eprintln!("[clings] erreur chargement exercice: {e}");
            }
            if let Some(sid) = session_id {
                self.save_checkpoint(conn, Some(sid), next);
            }
            true
        } else {
            if let Some(sid) = session_id {
                self.save_checkpoint(conn, Some(sid), self.state.current_index);
            }
            false
        }
    }

    /// Navigate to previous exercise, optionally saving checkpoint.
    fn navigate_prev(&mut self, conn: &rusqlite::Connection, session_id: Option<&str>) {
        if self.state.current_index > 0 {
            self.state.current_index -= 1;
            if let Err(e) = self.load_current_exercise(conn) {
                eprintln!("[clings] erreur chargement exercice: {e}");
            }
            if let Some(sid) = session_id {
                let idx = self.state.current_index;
                self.save_checkpoint(conn, Some(sid), idx);
            }
        }
    }

    /// Check if solution can be revealed (all hints shown OR enough failures).
    fn can_reveal_solution(&self, failure_threshold: usize) -> bool {
        let exercise = &self.state.exercises[self.state.current_index];
        let all_shown = exercise.hints.is_empty() || self.state.hint_index >= exercise.hints.len();
        all_shown || self.state.consecutive_failures as usize >= failure_threshold
    }

    /// Check if piscine solution can be revealed (fail_count-based threshold).
    fn can_reveal_solution_piscine(&self) -> bool {
        let exercise = &self.state.exercises[self.state.current_index];
        let all_shown = exercise.hints.is_empty() || self.state.hint_index >= exercise.hints.len();
        all_shown || self.state.piscine_fail_count >= crate::constants::PISCINE_FAILURE_THRESHOLD
    }

    /// Handle compilation and test of current exercise (triggered by 'r' key).
    /// Compiles the code, runs it, records attempt, and navigates on success.
    fn handle_compile(&mut self, conn: &rusqlite::Connection) {
        use crate::constants::CONSECUTIVE_FAILURE_THRESHOLD;

        let Some(path) = self.state.source_path.as_deref() else {
            return;
        };
        self.state.compile_pending = true;
        let exercise = &self.state.exercises[self.state.current_index];
        let result = crate::runner::compile_and_run(path, exercise);
        self.state.compile_pending = false;
        let success = result.success;
        self.state.run_result = Some(result);

        if success {
            self.state.consecutive_failures = 0;
            if !self.state.already_recorded {
                self.state.already_recorded = true;
                if let Err(e) =
                    crate::progress::record_attempt(conn, &exercise.subject, &exercise.id, true)
                {
                    eprintln!("[clings] erreur enregistrement tentative: {e}");
                }
                Self::invalidate_header_cache(&mut self.state);
            }
            self.state.completed[self.state.current_index] = true;
            self.state.overlay.success_overlay = true;
        } else {
            self.state.consecutive_failures = self.state.consecutive_failures.saturating_add(1);
            if (self.state.consecutive_failures as usize) >= CONSECUTIVE_FAILURE_THRESHOLD
                && self.state.hint_index == 0
            {
                let hints_len = self.state.exercises[self.state.current_index].hints.len();
                if hints_len > 0 {
                    self.state.hint_index = 1;
                }
            }
            let exercise = &self.state.exercises[self.state.current_index];
            if let Err(e) =
                crate::progress::record_attempt(conn, &exercise.subject, &exercise.id, false)
            {
                eprintln!("[clings] erreur enregistrement tentative: {e}");
            }
            Self::invalidate_header_cache(&mut self.state);
        }
    }

    /// Dispatch Watch messages → état
    pub fn update_watch(&mut self, msg: Msg, conn: &rusqlite::Connection) {
        use crate::constants::CONSECUTIVE_FAILURE_THRESHOLD;
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};

        match msg {
            Msg::Key(key) => {
                if self.handle_overlay_dispatch(key, conn, None) {
                    return;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        self.state.should_quit = true;
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => self.handle_hint_reveal(),
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        if self.can_reveal_solution(CONSECUTIVE_FAILURE_THRESHOLD) {
                            self.state.overlay.solution_active =
                                !self.state.overlay.solution_active;
                        }
                    }
                    KeyCode::Char('v') | KeyCode::Char('V') => self.handle_vis_toggle(),
                    KeyCode::Char('l') | KeyCode::Char('L') => self.open_list_overlay(),
                    KeyCode::Char('/') => self.open_search_overlay(),
                    KeyCode::Char('?') => {
                        self.state.overlay.help_active = true;
                    }
                    KeyCode::Char('j')
                    | KeyCode::Char('J')
                    | KeyCode::Char('n')
                    | KeyCode::Char('N') => {
                        self.navigate_next(conn, None);
                    }
                    KeyCode::Char('k') | KeyCode::Char('K') => {
                        self.navigate_prev(conn, None);
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        self.handle_compile(conn);
                    }
                    KeyCode::PageDown => {
                        self.state.description_scroll =
                            self.state.description_scroll.saturating_add(3);
                    }
                    KeyCode::PageUp => {
                        self.state.description_scroll =
                            self.state.description_scroll.saturating_sub(3);
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.state.should_quit = true;
                    }
                    KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.state.should_quit = true;
                    }
                    _ => {}
                }
            }
            Msg::FileChanged => self.handle_file_changed(),
            Msg::Tick => self.handle_tick_status_clear(),
        }
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
        if let Some(start) = self.state.piscine_start {
            let elapsed = start.elapsed().as_secs();
            self.state.timer_cache.piscine_last_elapsed_secs = elapsed;
            self.state.timer_cache.cached_piscine_elapsed_str =
                format!("⏱ {}m{:02}s", elapsed / 60, elapsed % 60);
        }
        if let Some(deadline) = self.state.piscine_deadline {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            self.state.timer_cache.piscine_last_remaining_secs = remaining;
            self.state.timer_cache.cached_piscine_remaining_str = format_remaining_secs(remaining);
        }

        // Prépare le premier exercice
        self.load_current_exercise(conn)?;

        let rx = match &self.state.source_path {
            Some(p) => crate::tui::events::spawn_event_reader(p.clone()),
            None => return Ok(()),
        };

        // Boucle TEA
        loop {
            terminal.draw(|f| crate::tui::ui_piscine::view(f, &self.state))?;

            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(msg) => self.update_piscine(msg, conn, session_id),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Check deadline on timeout
                    if let Some(deadline) = self.state.piscine_deadline {
                        if std::time::Instant::now() >= deadline {
                            self.state.should_quit = true;
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if self.state.should_quit {
                break;
            }
        }
        Ok(())
    }

    /// Dispatch Piscine messages → état
    pub fn update_piscine(
        &mut self,
        msg: Msg,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
    ) {
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};

        match msg {
            Msg::Key(key) => {
                if self.handle_overlay_dispatch(key, conn, session_id) {
                    return;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        let idx = self.state.current_index;
                        self.save_checkpoint(conn, session_id, idx);
                        self.state.should_quit = true;
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => self.handle_hint_reveal(),
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        if self.can_reveal_solution_piscine() {
                            self.state.overlay.solution_active =
                                !self.state.overlay.solution_active;
                        }
                    }
                    KeyCode::Char('v') | KeyCode::Char('V') => self.handle_vis_toggle(),
                    KeyCode::Char('n')
                    | KeyCode::Char('N')
                    | KeyCode::Char('j')
                    | KeyCode::Char('J') => {
                        if !self.navigate_next(conn, session_id) {
                            self.state.should_quit = true;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Char('K') => {
                        self.navigate_prev(conn, session_id);
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        let Some(path) = self.state.source_path.as_deref() else {
                            return;
                        };
                        self.state.compile_pending = true;
                        let exercise = &self.state.exercises[self.state.current_index];
                        let result = crate::runner::compile_and_run(path, exercise);
                        self.state.compile_pending = false;
                        let success = result.success;
                        let compile_error = result.compile_error;
                        self.state.run_result = Some(result);

                        if success {
                            self.state.piscine_fail_count = 0;
                            if !self.state.already_recorded {
                                self.state.already_recorded = true;
                                if let Err(e) = crate::progress::record_attempt(
                                    conn,
                                    &exercise.subject,
                                    &exercise.id,
                                    true,
                                ) {
                                    eprintln!("[clings] erreur enregistrement tentative: {e}");
                                }
                                Self::invalidate_header_cache(&mut self.state);
                            }
                            self.state.completed[self.state.current_index] = true;
                            self.state.overlay.success_overlay = true;
                        } else {
                            if !compile_error {
                                self.state.piscine_fail_count =
                                    self.state.piscine_fail_count.saturating_add(1);
                                if self.state.piscine_fail_count
                                    >= crate::constants::PISCINE_FAILURE_THRESHOLD
                                {
                                    if let Some(cm) = &exercise.common_mistake {
                                        self.state.status_msg = Some(format!("⚠ Piège : {}", cm));
                                        self.state.status_msg_at = Some(Instant::now());
                                    }
                                }
                            }
                            let exercise = &self.state.exercises[self.state.current_index];
                            if let Err(e) = crate::progress::record_attempt(
                                conn,
                                &exercise.subject,
                                &exercise.id,
                                false,
                            ) {
                                eprintln!("[clings] erreur enregistrement tentative: {e}");
                            }
                            Self::invalidate_header_cache(&mut self.state);
                        }
                    }
                    KeyCode::Char('l') | KeyCode::Char('L') => self.open_list_overlay(),
                    KeyCode::Char('/') => self.open_search_overlay(),
                    KeyCode::PageDown => {
                        self.state.description_scroll =
                            self.state.description_scroll.saturating_add(3);
                    }
                    KeyCode::PageUp => {
                        self.state.description_scroll =
                            self.state.description_scroll.saturating_sub(3);
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let idx = self.state.current_index;
                        self.save_checkpoint(conn, session_id, idx);
                        self.state.should_quit = true;
                    }
                    KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let idx = self.state.current_index;
                        self.save_checkpoint(conn, session_id, idx);
                        self.state.should_quit = true;
                    }
                    _ => {}
                }
            }
            Msg::FileChanged => self.handle_file_changed(),
            Msg::Tick => {
                self.handle_tick_status_clear();
                // Mise à jour du cache timer elapsed (1 allocation/seconde max)
                if let Some(start) = self.state.piscine_start {
                    let elapsed = start.elapsed().as_secs();
                    if elapsed != self.state.timer_cache.piscine_last_elapsed_secs {
                        self.state.timer_cache.piscine_last_elapsed_secs = elapsed;
                        self.state.timer_cache.cached_piscine_elapsed_str =
                            format!("⏱ {}m{:02}s", elapsed / 60, elapsed % 60);
                    }
                }
                // Mise à jour du cache timer restant (1 allocation/seconde max)
                if let Some(deadline) = self.state.piscine_deadline {
                    let now = std::time::Instant::now();
                    let remaining = deadline
                        .checked_duration_since(now)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    if remaining != self.state.timer_cache.piscine_last_remaining_secs {
                        self.state.timer_cache.piscine_last_remaining_secs = remaining;
                        self.state.timer_cache.cached_piscine_remaining_str =
                            format_remaining_secs(remaining);
                    }
                    // Check deadline on tick (réutilise `now` — évite un second syscall)
                    if now >= deadline {
                        let idx = self.state.current_index;
                        self.save_checkpoint(conn, session_id, idx);
                        self.state.should_quit = true;
                    }
                }
            }
        }
    }

    /// Mise à jour commune FileChanged — enregistre le message de status.
    fn handle_file_changed(&mut self) {
        if self.state.skip_file_changed {
            self.state.skip_file_changed = false;
            return;
        }
        self.state.status_msg = Some("fichier sauvegardé — [r] pour compiler".to_string());
        self.state.status_msg_at = Some(Instant::now());
    }

    /// Mise à jour commune Tick — expire le message de status après timeout.
    fn handle_tick_status_clear(&mut self) {
        if let Some(at) = self.state.status_msg_at {
            if at.elapsed()
                > std::time::Duration::from_secs(crate::constants::STATUS_MSG_TIMEOUT_SECS)
            {
                self.state.status_msg = None;
                self.state.status_msg_at = None;
            }
        }
    }
}

/// Formate un nombre de secondes restantes en chaîne lisible.
/// Partagé entre `run_piscine()` (init) et le Tick handler.
fn format_remaining_secs(remaining: u64) -> String {
    if remaining == 0 {
        "Temps écoulé".to_string()
    } else if remaining >= 60 {
        format!("{}m{:02}s restantes", remaining / 60, remaining % 60)
    } else {
        format!("{}s restantes", remaining)
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
                    }],
                }
            } else {
                Visualizer::default()
            },
        }
    }

    #[test]
    fn overlay_state_defaults_inactive() {
        let o = OverlayState::default();
        assert!(!o.help_active);
        assert!(!o.list_active);
        assert!(!o.search_active);
        assert!(!o.solution_active);
        assert!(!o.vis_active);
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
        app.state.exercises = vec![ex];
        app.state.completed = vec![false];
        app.state.hint_index = 2; // all shown
        app.state.consecutive_failures = 0;
        assert!(app.can_reveal_solution(3));
    }

    #[test]
    fn can_reveal_solution_enough_failures() {
        let mut app = App::new();
        let ex = make_exercise(vec!["h1".into(), "h2".into()], false);
        app.state.exercises = vec![ex];
        app.state.completed = vec![false];
        app.state.hint_index = 0; // none shown
        app.state.consecutive_failures = 3;
        assert!(app.can_reveal_solution(3));
    }

    #[test]
    fn can_reveal_solution_not_enough() {
        let mut app = App::new();
        let ex = make_exercise(vec!["h1".into(), "h2".into()], false);
        app.state.exercises = vec![ex];
        app.state.completed = vec![false];
        app.state.hint_index = 1; // partial
        app.state.consecutive_failures = 1;
        assert!(!app.can_reveal_solution(3));
    }

    #[test]
    fn can_reveal_solution_no_hints() {
        let mut app = App::new();
        let ex = make_exercise(vec![], false);
        app.state.exercises = vec![ex];
        app.state.completed = vec![false];
        app.state.hint_index = 0;
        app.state.consecutive_failures = 0;
        assert!(app.can_reveal_solution(3));
    }

    #[test]
    fn handle_hint_reveal_increments() {
        let mut app = App::new();
        let ex = make_exercise(vec!["h1".into(), "h2".into()], false);
        app.state.exercises = vec![ex];
        app.state.completed = vec![false];
        assert_eq!(app.state.hint_index, 0);
        app.handle_hint_reveal();
        assert_eq!(app.state.hint_index, 1);
        app.handle_hint_reveal();
        assert_eq!(app.state.hint_index, 2);
        // Should not exceed hints length
        app.handle_hint_reveal();
        assert_eq!(app.state.hint_index, 2);
    }

    #[test]
    fn handle_vis_toggle_activates() {
        let mut app = App::new();
        let ex = make_exercise(vec![], true);
        app.state.exercises = vec![ex];
        app.state.completed = vec![false];
        assert!(!app.state.overlay.vis_active);
        app.handle_vis_toggle();
        assert!(app.state.overlay.vis_active);
        assert_eq!(app.state.overlay.vis_step, 0);
    }

    #[test]
    fn handle_vis_toggle_noop_without_steps() {
        let mut app = App::new();
        let ex = make_exercise(vec![], false);
        app.state.exercises = vec![ex];
        app.state.completed = vec![false];
        app.handle_vis_toggle();
        assert!(!app.state.overlay.vis_active);
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
}
