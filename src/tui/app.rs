//! Application state and event handling (TEA/Elm architecture).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::error::Result;
use crate::models::Exercise;
use crate::runner::RunResult;
use crate::search;

/// Messages traités par App::update_watch()
#[derive(Debug)]
pub enum Msg {
    Key(ratatui::crossterm::event::KeyEvent),
    FileChanged,
    Tick,
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
    pub solution_active: bool,
    pub compile_pending: bool,
    pub vis_active: bool,
    pub vis_step: usize,
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
    // Fuzzy search overlay
    pub search_active: bool,
    pub search_query: String,
    pub search_results: Vec<usize>,
    pub search_selected: usize,
    pub search_subject_filter: bool,
    // Subject order cache (filled once on init)
    pub subject_order: Vec<String>,
    // Help overlay
    pub help_active: bool,
    // Description panel scroll offset
    pub description_scroll: u16,
    // Search: pending 'g' for gg → first
    pub search_g_pending: bool,
    // Header cache — invalider sur changement d'exercice ou mise à jour mastery
    pub cached_mini_map: String,
    pub cached_exercise_counter: String, // "[N/total] "
    pub cached_mastery_display: String,  // "mastery: X.X  "
    pub cached_exercise_type: String,    // ExerciseType::to_string() — invariant par exercice
    pub cached_header_left_len: usize,   // Width of "[N/total] " + title in chars
    pub cached_mini_map_len: usize,      // Display width of cached_mini_map (chars count)
    // Piscine timer cache — mis à jour dans Tick quand la seconde change
    pub cached_piscine_elapsed_str: String,
    pub piscine_last_elapsed_secs: u64,
    pub cached_piscine_remaining_str: String,
    pub piscine_last_remaining_secs: u64,
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
            solution_active: false,
            compile_pending: false,
            vis_active: false,
            vis_step: 0,
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
            search_active: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            search_subject_filter: false,
            subject_order: Vec::new(),
            help_active: false,
            description_scroll: 0,
            search_g_pending: false,
            cached_mini_map: String::new(),
            cached_exercise_counter: String::new(),
            cached_mastery_display: String::new(),
            cached_exercise_type: String::new(),
            cached_header_left_len: 0,
            cached_mini_map_len: 0,
            cached_piscine_elapsed_str: String::new(),
            piscine_last_elapsed_secs: u64::MAX,
            cached_piscine_remaining_str: String::new(),
            piscine_last_remaining_secs: u64::MAX,
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
        state.cached_exercise_counter = format!("[{}/{}] ", idx + 1, total);
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
        state.cached_mastery_display = format!("mastery: {:.1}  ", mastery);
        state.cached_mini_map = crate::tui::common::mini_map(&state.completed, idx);
        state.cached_mini_map_len = state.cached_mini_map.chars().count();
        state.cached_exercise_type = state
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
        state.cached_header_left_len =
            state.cached_exercise_counter.chars().count() + title.chars().count();
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
        let (source_path, stage) = crate::runner::prepare_exercise_source(conn, exercise)?;

        // Restart watcher if we change exercise
        self.state.source_path = Some(source_path);
        self.state.current_stage = stage;
        self.state.run_result = None;
        self.state.hint_index = 0;
        self.state.solution_active = false;
        self.state.compile_pending = false;
        self.state.vis_active = false;
        self.state.vis_step = 0;
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
        let subject_filter = if state.search_subject_filter {
            state
                .exercises
                .get(state.current_index)
                .map(|ex| ex.subject.as_str())
        } else {
            None
        };
        if state.search_query.is_empty() && subject_filter.is_none() {
            state.search_results = (0..state.exercises.len()).collect();
        } else if state.search_query.is_empty() {
            state.search_results = state
                .exercises
                .iter()
                .enumerate()
                .filter(|(_, ex)| subject_filter.is_none_or(|s| ex.subject == s))
                .map(|(i, _)| i)
                .collect();
        } else {
            state.search_results =
                search::search_exercises(&state.exercises, &state.search_query, subject_filter)
                    .into_iter()
                    .map(|(idx, _score)| idx)
                    .collect();
        }
        state.search_selected = 0;
    }

    /// Gère les touches de l'overlay de recherche.
    /// Retourne `true` si Enter a été pressé avec un résultat sélectionné
    /// (l'appelant doit alors charger l'exercice et/ou sauvegarder le checkpoint).
    fn handle_search_key(state: &mut AppState, key: ratatui::crossterm::event::KeyEvent) -> bool {
        use ratatui::crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                state.search_active = false;
                state.search_query.clear();
                state.search_results.clear();
            }
            KeyCode::Tab => {
                state.search_subject_filter = !state.search_subject_filter;
                Self::rebuild_search(state);
            }
            KeyCode::Enter => {
                if !state.search_results.is_empty() {
                    let idx = state.search_results[state.search_selected];
                    state.current_index = idx;
                    state.search_active = false;
                    state.search_query.clear();
                    state.search_results.clear();
                    return true;
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                let len = state.search_results.len();
                if len > 0 {
                    state.search_selected = (state.search_selected + 1) % len;
                }
                state.search_g_pending = false;
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                let len = state.search_results.len();
                if len > 0 {
                    state.search_selected = state.search_selected.checked_sub(1).unwrap_or(len - 1);
                }
                state.search_g_pending = false;
            }
            KeyCode::Char('G') => {
                let len = state.search_results.len();
                if len > 0 {
                    state.search_selected = len - 1;
                }
                state.search_g_pending = false;
            }
            KeyCode::Char('g') => {
                if state.search_g_pending {
                    state.search_selected = 0;
                    state.search_g_pending = false;
                } else {
                    state.search_g_pending = true;
                }
            }
            KeyCode::Backspace => {
                state.search_query.pop();
                Self::rebuild_search(state);
                state.search_g_pending = false;
            }
            KeyCode::Char(c) => {
                state.search_g_pending = false;
                state.search_query.push(c);
                Self::rebuild_search(state);
            }
            _ => {
                state.search_g_pending = false;
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
                state.vis_step = (state.vis_step + 1).min(total.saturating_sub(1));
            }
            KeyCode::Left => {
                state.vis_step = state.vis_step.saturating_sub(1);
            }
            _ => {
                state.vis_active = false;
            }
        }
    }

    /// Gère les touches de l'overlay solution.
    /// Retourne `true` si l'overlay était actif (l'appelant doit `return`).
    fn handle_solution_overlay(
        state: &mut AppState,
        key: ratatui::crossterm::event::KeyEvent,
    ) -> bool {
        if !state.solution_active {
            return false;
        }
        if matches!(
            key.code,
            ratatui::crossterm::event::KeyCode::Esc
                | ratatui::crossterm::event::KeyCode::Char('s')
                | ratatui::crossterm::event::KeyCode::Char('S')
        ) {
            state.solution_active = false;
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

    /// Dispatch Watch messages → état
    pub fn update_watch(&mut self, msg: Msg, conn: &rusqlite::Connection) {
        use crate::constants::{CONSECUTIVE_FAILURE_THRESHOLD, SUCCESS_PAUSE_SECS};
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};

        match msg {
            Msg::Key(key) => {
                // Search overlay — capture all keys when active
                if self.state.search_active {
                    if Self::handle_search_key(&mut self.state, key) {
                        if let Err(e) = self.load_current_exercise(conn) {
                            eprintln!("[clings] erreur chargement exercice: {e}");
                        }
                    }
                    return;
                }

                // Solution overlay — Esc or [s] closes
                if Self::handle_solution_overlay(&mut self.state, key) {
                    return;
                }

                // Help overlay — any key closes
                if self.state.help_active {
                    self.state.help_active = false;
                    return;
                }

                // Visualizer mode — arrow keys / any key to close
                if self.state.vis_active {
                    Self::handle_vis_key(&mut self.state, key);
                    return;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        self.state.should_quit = true;
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        let hints_len = self.state.exercises[self.state.current_index].hints.len();
                        self.state.hint_index = (self.state.hint_index + 1).min(hints_len);
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        let exercise = &self.state.exercises[self.state.current_index];
                        let all_shown = exercise.hints.is_empty()
                            || self.state.hint_index >= exercise.hints.len();
                        let enough_failures = self.state.consecutive_failures as usize
                            >= CONSECUTIVE_FAILURE_THRESHOLD;
                        if all_shown || enough_failures {
                            self.state.solution_active = !self.state.solution_active;
                        }
                    }
                    KeyCode::Char('v') | KeyCode::Char('V') => {
                        if !self.state.exercises[self.state.current_index]
                            .visualizer
                            .steps
                            .is_empty()
                        {
                            self.state.vis_active = true;
                            self.state.vis_step = 0;
                        }
                    }
                    KeyCode::Char('/') => {
                        self.state.search_active = true;
                        self.state.search_subject_filter = false;
                        self.state.search_query.clear();
                        Self::rebuild_search(&mut self.state);
                    }
                    KeyCode::Char('?') => {
                        self.state.help_active = true;
                    }
                    KeyCode::Char('j')
                    | KeyCode::Char('J')
                    | KeyCode::Char('n')
                    | KeyCode::Char('N') => {
                        let next = self.state.current_index + 1;
                        if next < self.state.exercises.len() {
                            self.state.current_index = next;
                            if let Err(e) = self.load_current_exercise(conn) {
                                eprintln!("[clings] erreur chargement exercice: {e}");
                            }
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Char('K') => {
                        if self.state.current_index > 0 {
                            self.state.current_index -= 1;
                            if let Err(e) = self.load_current_exercise(conn) {
                                eprintln!("[clings] erreur chargement exercice: {e}");
                            }
                        }
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        if let Some(path) = self.state.source_path.as_deref() {
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
                                // Advance after short delay
                                std::thread::sleep(std::time::Duration::from_secs(
                                    SUCCESS_PAUSE_SECS,
                                ));
                                let next = self.state.current_index + 1;
                                self.state.completed[self.state.current_index] = true;
                                if next < self.state.exercises.len() {
                                    self.state.current_index = next;
                                    if let Err(e) = self.load_current_exercise(conn) {
                                        eprintln!("[clings] erreur chargement exercice: {e}");
                                    }
                                } else {
                                    self.state.should_quit = true;
                                }
                            } else {
                                self.state.consecutive_failures =
                                    self.state.consecutive_failures.saturating_add(1);
                                if (self.state.consecutive_failures as usize)
                                    >= CONSECUTIVE_FAILURE_THRESHOLD
                                {
                                    // Reveal first hint automatically if none shown yet
                                    if self.state.hint_index == 0 {
                                        let hints_len = self.state.exercises
                                            [self.state.current_index]
                                            .hints
                                            .len();
                                        if hints_len > 0 {
                                            self.state.hint_index = 1;
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
            self.state.piscine_last_elapsed_secs = elapsed;
            self.state.cached_piscine_elapsed_str =
                format!("⏱ {}m{:02}s", elapsed / 60, elapsed % 60);
        }
        if let Some(deadline) = self.state.piscine_deadline {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            self.state.piscine_last_remaining_secs = remaining;
            self.state.cached_piscine_remaining_str = format_remaining_secs(remaining);
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
        use crate::constants::PISCINE_FAILURE_THRESHOLD;
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};

        match msg {
            Msg::Key(key) => {
                // Search overlay — capture all keys when active
                if self.state.search_active {
                    if Self::handle_search_key(&mut self.state, key) {
                        if let Err(e) = self.load_current_exercise(conn) {
                            eprintln!("[clings] erreur chargement exercice: {e}");
                        }
                        let cidx = self.state.current_index;
                        self.save_checkpoint(conn, session_id, cidx);
                    }
                    return;
                }

                // Solution overlay — Esc or [s] closes
                if Self::handle_solution_overlay(&mut self.state, key) {
                    return;
                }

                // Visualizer mode — arrow keys / any key to close
                if self.state.vis_active {
                    Self::handle_vis_key(&mut self.state, key);
                    return;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        let idx = self.state.current_index;
                        self.save_checkpoint(conn, session_id, idx);
                        self.state.should_quit = true;
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        let hints_len = self.state.exercises[self.state.current_index].hints.len();
                        self.state.hint_index = (self.state.hint_index + 1).min(hints_len);
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        let exercise = &self.state.exercises[self.state.current_index];
                        let all_shown = exercise.hints.is_empty()
                            || self.state.hint_index >= exercise.hints.len();
                        let enough_failures =
                            self.state.piscine_fail_count >= PISCINE_FAILURE_THRESHOLD;
                        if all_shown || enough_failures {
                            self.state.solution_active = !self.state.solution_active;
                        }
                    }
                    KeyCode::Char('v') | KeyCode::Char('V') => {
                        if !self.state.exercises[self.state.current_index]
                            .visualizer
                            .steps
                            .is_empty()
                        {
                            self.state.vis_active = true;
                            self.state.vis_step = 0;
                        }
                    }
                    KeyCode::Char('n')
                    | KeyCode::Char('N')
                    | KeyCode::Char('j')
                    | KeyCode::Char('J') => {
                        let next = self.state.current_index + 1;
                        if next < self.state.exercises.len() {
                            self.state.current_index = next;
                            if let Err(e) = self.load_current_exercise(conn) {
                                eprintln!("[clings] erreur chargement exercice: {e}");
                            }
                            let idx = self.state.current_index;
                            self.save_checkpoint(conn, session_id, idx);
                        } else {
                            // Reached end
                            let idx = self.state.current_index;
                            self.save_checkpoint(conn, session_id, idx);
                            self.state.should_quit = true;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Char('K') => {
                        if self.state.current_index > 0 {
                            self.state.current_index -= 1;
                            if let Err(e) = self.load_current_exercise(conn) {
                                eprintln!("[clings] erreur chargement exercice: {e}");
                            }
                            let idx = self.state.current_index;
                            self.save_checkpoint(conn, session_id, idx);
                        }
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        if let Some(path) = self.state.source_path.as_deref() {
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
                                // Advance after short delay
                                std::thread::sleep(std::time::Duration::from_secs(
                                    crate::constants::SUCCESS_PAUSE_SECS,
                                ));
                                self.state.completed[self.state.current_index] = true;
                                let next = self.state.current_index + 1;
                                if next < self.state.exercises.len() {
                                    self.state.current_index = next;
                                    if let Err(e) = self.load_current_exercise(conn) {
                                        eprintln!("[clings] erreur chargement exercice: {e}");
                                    }
                                    let idx = self.state.current_index;
                                    self.save_checkpoint(conn, session_id, idx);
                                } else {
                                    // Reached end
                                    let idx = self.state.current_index;
                                    self.save_checkpoint(conn, session_id, idx);
                                    self.state.should_quit = true;
                                }
                            } else {
                                if !compile_error {
                                    self.state.piscine_fail_count =
                                        self.state.piscine_fail_count.saturating_add(1);
                                    if self.state.piscine_fail_count >= PISCINE_FAILURE_THRESHOLD {
                                        if let Some(cm) = &exercise.common_mistake {
                                            self.state.status_msg =
                                                Some(format!("⚠ Piège : {}", cm));
                                            self.state.status_msg_at = Some(Instant::now());
                                        }
                                    }
                                }
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
                    }
                    KeyCode::Char('/') => {
                        self.state.search_active = true;
                        Self::rebuild_search(&mut self.state);
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
                    if elapsed != self.state.piscine_last_elapsed_secs {
                        self.state.piscine_last_elapsed_secs = elapsed;
                        self.state.cached_piscine_elapsed_str =
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
                    if remaining != self.state.piscine_last_remaining_secs {
                        self.state.piscine_last_remaining_secs = remaining;
                        self.state.cached_piscine_remaining_str = format_remaining_secs(remaining);
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
