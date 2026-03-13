//! Application state and event handling (TEA/Elm architecture).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::error::Result;
use crate::models::Exercise;
use crate::runner::RunResult;
use crate::search;

/// Mode d'affichage de l'application
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppMode {
    Watch {
        chapter: Option<u8>,
    },
    Piscine {
        chapter: Option<u8>,
        timed: Option<u64>,
    },
}

/// Messages traités par App::update_watch()
#[derive(Debug)]
#[allow(dead_code)]
pub enum Msg {
    Key(ratatui::crossterm::event::KeyEvent),
    FileChanged(PathBuf),
    Tick,
    Quit,
}

/// État centralisé — mode watch
#[allow(dead_code)]
pub struct AppState {
    pub mode: AppMode,
    pub should_quit: bool,
    // Watch data
    pub exercises: Vec<Exercise>,
    pub completed: Vec<bool>,
    pub current_index: usize,
    pub run_result: Option<RunResult>,
    pub source_path: Option<PathBuf>,
    pub current_stage: Option<u8>,
    pub editor_pane: Option<String>,
    pub hint_shown: bool,
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
    // Search: pending 'g' for gg → first
    pub search_g_pending: bool,
}

impl AppState {
    pub fn new(mode: AppMode) -> Self {
        Self {
            mode,
            should_quit: false,
            exercises: Vec::new(),
            completed: Vec::new(),
            current_index: 0,
            run_result: None,
            source_path: None,
            current_stage: None,
            editor_pane: None,
            hint_shown: false,
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
            search_g_pending: false,
        }
    }
}

pub struct App {
    pub state: AppState,
}

impl App {
    pub fn new(mode: AppMode) -> Self {
        Self {
            state: AppState::new(mode),
        }
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
        self.state.hint_shown = false;
        self.state.vis_active = false;
        self.state.vis_step = 0;
        self.state.already_recorded = false;
        self.state.consecutive_failures = 0;
        self.state.status_msg = None;

        // Open/update neovim pane in tmux
        let pane = crate::tmux::update_editor_pane(
            self.state.editor_pane.as_deref(),
            self.state
                .source_path
                .as_ref()
                .expect("source_path toujours Some après load_current_exercise"),
        );
        self.state.editor_pane = pane;

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
            let base = state.exercises.as_ptr();
            state.search_results =
                search::search_exercises(&state.exercises, &state.search_query, subject_filter)
                    .into_iter()
                    .map(|(ex, _score)| {
                        // SAFETY: `search_exercises` returns references into `state.exercises`
                        // which is a contiguous Vec — both pointers share the same allocation.
                        unsafe { (ex as *const crate::models::Exercise).offset_from(base) as usize }
                    })
                    .collect();
        }
        state.search_selected = 0;
    }

    /// Dispatch Watch messages → état
    pub fn update_watch(&mut self, msg: Msg, conn: &rusqlite::Connection) {
        use crate::constants::{CONSECUTIVE_FAILURE_THRESHOLD, SUCCESS_PAUSE_SECS};
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};

        match msg {
            Msg::Key(key) => {
                // Search overlay — capture all keys when active
                if self.state.search_active {
                    match key.code {
                        KeyCode::Esc => {
                            self.state.search_active = false;
                            self.state.search_query.clear();
                            self.state.search_results.clear();
                        }
                        KeyCode::Tab => {
                            self.state.search_subject_filter = !self.state.search_subject_filter;
                            Self::rebuild_search(&mut self.state);
                        }
                        KeyCode::Enter => {
                            if !self.state.search_results.is_empty() {
                                let idx = self.state.search_results[self.state.search_selected];
                                self.state.current_index = idx;
                                self.state.search_active = false;
                                self.state.search_query.clear();
                                self.state.search_results.clear();
                                if let Err(e) = self.load_current_exercise(conn) {
                                    eprintln!("[clings] erreur chargement exercice: {e}");
                                }
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                            let len = self.state.search_results.len();
                            if len > 0 {
                                self.state.search_selected = (self.state.search_selected + 1) % len;
                            }
                            self.state.search_g_pending = false;
                        }
                        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                            let len = self.state.search_results.len();
                            if len > 0 {
                                self.state.search_selected =
                                    self.state.search_selected.checked_sub(1).unwrap_or(len - 1);
                            }
                            self.state.search_g_pending = false;
                        }
                        KeyCode::Char('G') => {
                            let len = self.state.search_results.len();
                            if len > 0 {
                                self.state.search_selected = len - 1;
                            }
                            self.state.search_g_pending = false;
                        }
                        KeyCode::Char('g') => {
                            if self.state.search_g_pending {
                                self.state.search_selected = 0;
                                self.state.search_g_pending = false;
                            } else {
                                self.state.search_g_pending = true;
                            }
                        }
                        KeyCode::Backspace => {
                            self.state.search_query.pop();
                            Self::rebuild_search(&mut self.state);
                            self.state.search_g_pending = false;
                        }
                        KeyCode::Char(c) => {
                            self.state.search_g_pending = false;
                            self.state.search_query.push(c);
                            Self::rebuild_search(&mut self.state);
                        }
                        _ => {
                            self.state.search_g_pending = false;
                        }
                    }
                    return;
                }

                // Help overlay — any key closes
                if self.state.help_active {
                    self.state.help_active = false;
                    return;
                }

                // Visualizer mode — arrow keys / any key to close
                if self.state.vis_active {
                    match key.code {
                        KeyCode::Right => {
                            let total = self.state.exercises[self.state.current_index]
                                .visualizer
                                .steps
                                .len();
                            self.state.vis_step =
                                (self.state.vis_step + 1).min(total.saturating_sub(1));
                        }
                        KeyCode::Left => {
                            self.state.vis_step = self.state.vis_step.saturating_sub(1);
                        }
                        _ => {
                            self.state.vis_active = false;
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        self.state.should_quit = true;
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        self.state.hint_shown = true;
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
                        if let Some(path) = &self.state.source_path.clone() {
                            let exercise = &self.state.exercises[self.state.current_index];
                            let result = crate::runner::compile_and_run(path, exercise);
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
                                    self.state.hint_shown = true;
                                }
                                let subj = self.state.exercises[self.state.current_index]
                                    .subject
                                    .clone();
                                let id = self.state.exercises[self.state.current_index].id.clone();
                                if let Err(e) =
                                    crate::progress::record_attempt(conn, &subj, &id, false)
                                {
                                    eprintln!("[clings] erreur enregistrement tentative: {e}");
                                }
                            }
                        }
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
            Msg::FileChanged(_) => {
                self.state.status_msg = Some("fichier sauvegardé — [r] pour compiler".to_string());
                self.state.status_msg_at = Some(Instant::now());
            }
            Msg::Tick => {
                if let Some(at) = self.state.status_msg_at {
                    if at.elapsed() > std::time::Duration::from_secs(3) {
                        self.state.status_msg = None;
                        self.state.status_msg_at = None;
                    }
                }
            }
            Msg::Quit => {
                self.state.should_quit = true;
            }
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
                    match key.code {
                        KeyCode::Esc => {
                            self.state.search_active = false;
                            self.state.search_query.clear();
                            self.state.search_results.clear();
                        }
                        KeyCode::Tab => {
                            self.state.search_subject_filter = !self.state.search_subject_filter;
                            Self::rebuild_search(&mut self.state);
                        }
                        KeyCode::Enter => {
                            if !self.state.search_results.is_empty() {
                                let idx = self.state.search_results[self.state.search_selected];
                                self.state.current_index = idx;
                                self.state.search_active = false;
                                self.state.search_query.clear();
                                self.state.search_results.clear();
                                if let Err(e) = self.load_current_exercise(conn) {
                                    eprintln!("[clings] erreur chargement exercice: {e}");
                                }
                                let cidx = self.state.current_index;
                                let _ = crate::progress::save_piscine_checkpoint(conn, cidx);
                                if let Some(sid) = session_id {
                                    let _ = crate::progress::save_exam_checkpoint(conn, sid, cidx);
                                }
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                            let len = self.state.search_results.len();
                            if len > 0 {
                                self.state.search_selected = (self.state.search_selected + 1) % len;
                            }
                            self.state.search_g_pending = false;
                        }
                        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                            let len = self.state.search_results.len();
                            if len > 0 {
                                self.state.search_selected =
                                    self.state.search_selected.checked_sub(1).unwrap_or(len - 1);
                            }
                            self.state.search_g_pending = false;
                        }
                        KeyCode::Char('G') => {
                            let len = self.state.search_results.len();
                            if len > 0 {
                                self.state.search_selected = len - 1;
                            }
                            self.state.search_g_pending = false;
                        }
                        KeyCode::Char('g') => {
                            if self.state.search_g_pending {
                                self.state.search_selected = 0;
                                self.state.search_g_pending = false;
                            } else {
                                self.state.search_g_pending = true;
                            }
                        }
                        KeyCode::Backspace => {
                            self.state.search_query.pop();
                            Self::rebuild_search(&mut self.state);
                            self.state.search_g_pending = false;
                        }
                        KeyCode::Char(c) => {
                            self.state.search_g_pending = false;
                            self.state.search_query.push(c);
                            Self::rebuild_search(&mut self.state);
                        }
                        _ => {
                            self.state.search_g_pending = false;
                        }
                    }
                    return;
                }

                // Visualizer mode — arrow keys / any key to close
                if self.state.vis_active {
                    match key.code {
                        KeyCode::Right => {
                            let total = self.state.exercises[self.state.current_index]
                                .visualizer
                                .steps
                                .len();
                            self.state.vis_step =
                                (self.state.vis_step + 1).min(total.saturating_sub(1));
                        }
                        KeyCode::Left => {
                            self.state.vis_step = self.state.vis_step.saturating_sub(1);
                        }
                        _ => {
                            self.state.vis_active = false;
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        let idx = self.state.current_index;
                        let _ = crate::progress::save_piscine_checkpoint(conn, idx);
                        if let Some(sid) = session_id {
                            let _ = crate::progress::save_exam_checkpoint(conn, sid, idx);
                        }
                        self.state.should_quit = true;
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        self.state.hint_shown = true;
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
                            let _ = crate::progress::save_piscine_checkpoint(conn, idx);
                            if let Some(sid) = session_id {
                                let _ = crate::progress::save_exam_checkpoint(conn, sid, idx);
                            }
                        } else {
                            // Reached end
                            let idx = self.state.current_index;
                            let _ = crate::progress::save_piscine_checkpoint(conn, idx);
                            if let Some(sid) = session_id {
                                let _ = crate::progress::save_exam_checkpoint(conn, sid, idx);
                            }
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
                            let _ = crate::progress::save_piscine_checkpoint(conn, idx);
                            if let Some(sid) = session_id {
                                let _ = crate::progress::save_exam_checkpoint(conn, sid, idx);
                            }
                        }
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        if let Some(path) = &self.state.source_path.clone() {
                            let exercise = &self.state.exercises[self.state.current_index];
                            let result = crate::runner::compile_and_run(path, exercise);
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
                                    let _ = crate::progress::save_piscine_checkpoint(conn, idx);
                                    if let Some(sid) = session_id {
                                        let _ =
                                            crate::progress::save_exam_checkpoint(conn, sid, idx);
                                    }
                                } else {
                                    // Reached end
                                    let idx = self.state.current_index;
                                    let _ = crate::progress::save_piscine_checkpoint(conn, idx);
                                    if let Some(sid) = session_id {
                                        let _ =
                                            crate::progress::save_exam_checkpoint(conn, sid, idx);
                                    }
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
                            }
                        }
                    }
                    KeyCode::Char('/') => {
                        self.state.search_active = true;
                        Self::rebuild_search(&mut self.state);
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let idx = self.state.current_index;
                        let _ = crate::progress::save_piscine_checkpoint(conn, idx);
                        if let Some(sid) = session_id {
                            let _ = crate::progress::save_exam_checkpoint(conn, sid, idx);
                        }
                        self.state.should_quit = true;
                    }
                    KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let idx = self.state.current_index;
                        let _ = crate::progress::save_piscine_checkpoint(conn, idx);
                        if let Some(sid) = session_id {
                            let _ = crate::progress::save_exam_checkpoint(conn, sid, idx);
                        }
                        self.state.should_quit = true;
                    }
                    _ => {}
                }
            }
            Msg::FileChanged(_) => {
                self.state.status_msg = Some("fichier sauvegardé — [r] pour compiler".to_string());
                self.state.status_msg_at = Some(Instant::now());
            }
            Msg::Tick => {
                if let Some(at) = self.state.status_msg_at {
                    if at.elapsed() > std::time::Duration::from_secs(3) {
                        self.state.status_msg = None;
                        self.state.status_msg_at = None;
                    }
                }
                // Check deadline on tick
                if let Some(deadline) = self.state.piscine_deadline {
                    if std::time::Instant::now() >= deadline {
                        let idx = self.state.current_index;
                        let _ = crate::progress::save_piscine_checkpoint(conn, idx);
                        if let Some(sid) = session_id {
                            let _ = crate::progress::save_exam_checkpoint(conn, sid, idx);
                        }
                        self.state.should_quit = true;
                    }
                }
            }
            Msg::Quit => {
                let idx = self.state.current_index;
                let _ = crate::progress::save_piscine_checkpoint(conn, idx);
                if let Some(sid) = session_id {
                    let _ = crate::progress::save_exam_checkpoint(conn, sid, idx);
                }
                self.state.should_quit = true;
            }
        }
    }
}
