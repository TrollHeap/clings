//! Application state and event handling (TEA/Elm architecture).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::error::Result;
use crate::models::Exercise;
use crate::runner::RunResult;

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
    // Status message (file saved, etc.)
    pub status_msg: Option<String>,
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
            status_msg: None,
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
            self.state.source_path.as_ref().unwrap(),
        );
        self.state.editor_pane = pane;

        Ok(())
    }

    /// Dispatch Watch messages → état
    pub fn update_watch(&mut self, msg: Msg, conn: &rusqlite::Connection) {
        use crate::constants::{CONSECUTIVE_FAILURE_THRESHOLD, SUCCESS_PAUSE_SECS};
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};

        match msg {
            Msg::Key(key) => {
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
                    KeyCode::Char('j')
                    | KeyCode::Char('J')
                    | KeyCode::Char('n')
                    | KeyCode::Char('N') => {
                        let next = self.state.current_index + 1;
                        if next < self.state.exercises.len() {
                            self.state.current_index = next;
                            let _ = self.load_current_exercise(conn);
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Char('K') => {
                        if self.state.current_index > 0 {
                            self.state.current_index -= 1;
                            let _ = self.load_current_exercise(conn);
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
                                    let _ = crate::progress::record_attempt(
                                        conn,
                                        &exercise.subject.clone(),
                                        &exercise.id.clone(),
                                        true,
                                    );
                                }
                                // Advance after short delay
                                std::thread::sleep(std::time::Duration::from_secs(
                                    SUCCESS_PAUSE_SECS,
                                ));
                                let next = self.state.current_index + 1;
                                self.state.completed[self.state.current_index] = true;
                                if next < self.state.exercises.len() {
                                    self.state.current_index = next;
                                    let _ = self.load_current_exercise(conn);
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
                                let _ = crate::progress::record_attempt(
                                    conn,
                                    &self.state.exercises[self.state.current_index]
                                        .subject
                                        .clone(),
                                    &self.state.exercises[self.state.current_index].id.clone(),
                                    false,
                                );
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
            }
            Msg::Tick => {
                // Clear status message after a while (optionnel)
            }
            Msg::Quit => {
                self.state.should_quit = true;
            }
        }
    }
}
