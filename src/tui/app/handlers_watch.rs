//! Watch mode event handlers.
//!
//! Handles compilation, navigation, hint/solution/visualizer toggles specific to watch mode.

use crate::tui::ui_messages::{ActiveOverlay, Command, Msg};

use super::App;

impl App {
    /// Dispatch watch mode messages → état (main update function for watch loop).
    pub(crate) fn update_watch(&mut self, msg: Msg, conn: &rusqlite::Connection) {
        use crate::constants::CONSECUTIVE_FAILURE_THRESHOLD;

        match msg {
            Msg::Key(key) => {
                if self.handle_overlay_dispatch(key, conn, None) {
                    return;
                }
                let Some(cmd) = crate::tui::ui_keymap::key_to_cmd(&key, false) else {
                    return;
                };
                match cmd {
                    Command::Quit => {
                        if key.modifiers.is_empty() {
                            self.state.overlay.quit_confirm_active = true;
                        } else {
                            // Ctrl+C / Ctrl+Z → quitter immédiatement sans modale
                            self.state.session.should_quit = true;
                        }
                    }
                    Command::CompileRun => self.handle_compile(conn),
                    Command::ShowHint => self.reveal_next_hint(),
                    Command::ToggleSolution => {
                        if self.can_reveal_solution(CONSECUTIVE_FAILURE_THRESHOLD) {
                            self.state.overlay.active =
                                if self.state.overlay.active == ActiveOverlay::Solution {
                                    ActiveOverlay::None
                                } else {
                                    ActiveOverlay::Solution
                                };
                        }
                    }
                    Command::OpenVisualizer => self.toggle_visualizer_overlay(),
                    Command::OpenList => self.open_list_overlay(),
                    Command::OpenSearch => self.open_search_overlay(),
                    Command::OpenLibsys => self.open_libsys_overlay(),
                    Command::ShowHelp => self.state.overlay.active = ActiveOverlay::Help,
                    Command::NavNext => {
                        self.state.overlay.nav_confirm_active = true;
                        self.state.overlay.nav_confirm_next = true;
                    }
                    Command::NavPrev => {
                        self.state.overlay.nav_confirm_active = true;
                        self.state.overlay.nav_confirm_next = false;
                    }
                    Command::ScrollDown => {
                        self.state.ex.description_scroll =
                            self.state.ex.description_scroll.saturating_add(3);
                    }
                    Command::ScrollUp => {
                        self.state.ex.description_scroll =
                            self.state.ex.description_scroll.saturating_sub(3);
                    }
                }
            }
            Msg::FileChanged => self.handle_file_changed(),
            Msg::Tick => self.handle_tick_status_clear(),
            Msg::Resize(_w, _h) => {
                // Ratatui recalcule le layout à chaque draw — pas d'action requise.
            }
        }
    }

    /// Compile and run the current exercise, record attempt, and update state.
    /// Trois étapes implicites :
    /// 1. Exécute le runner (compile + run)
    /// 2. Enregistre la tentative en DB
    /// 3. Exporte libsys si applicable
    fn handle_compile(&mut self, conn: &rusqlite::Connection) {
        use crate::constants::CONSECUTIVE_FAILURE_THRESHOLD;

        let Some(path) = self.state.ex.source_path.as_deref() else {
            return;
        };
        self.state.session.compile_pending = true;
        let Some(exercise) = self.state.ex.exercises.get(self.state.ex.current_index) else {
            self.state.session.compile_pending = false;
            return;
        };
        let result = crate::runner::compile_and_run(path, exercise);
        self.state.session.compile_pending = false;
        let success = result.success;
        self.state.ex.run_result = Some(result);

        if success {
            self.state.ex.consecutive_failures = 0;
            if !self.state.ex.already_recorded {
                self.state.ex.already_recorded = true;
                let subject = exercise.subject.clone();
                let exercise_id = exercise.id.clone();
                // Clone libsys fields before releasing exercise borrow
                let libsys_info = (
                    exercise.exercise_type,
                    exercise.libsys_module.clone(),
                    exercise.libsys_function.clone(),
                    exercise.header_code.clone(),
                );
                if let Err(e) = crate::progress::record_attempt(conn, &subject, &exercise_id, true)
                {
                    eprintln!("[clings] erreur enregistrement tentative: {e}");
                }
                let path_owned = path.to_path_buf();
                self.try_export_libsys(&path_owned, libsys_info);
                Self::invalidate_header_cache(&mut self.state);
                self.update_interleaving_nudge(subject);
            }
            self.state.ex.completed[self.state.ex.current_index] = true;
            self.state.overlay.success_overlay = true;
        } else {
            self.state.ex.consecutive_failures =
                self.state.ex.consecutive_failures.saturating_add(1);
            if (self.state.ex.consecutive_failures as usize) >= CONSECUTIVE_FAILURE_THRESHOLD
                && self.state.ex.hint_index == 0
            {
                let hints_len = self
                    .state
                    .current_ex()
                    .map(|ex| ex.hints.len())
                    .unwrap_or(0);
                if hints_len > 0 {
                    self.state.ex.hint_index = 1;
                }
            }
            if let Some(exercise) = self.state.ex.exercises.get(self.state.ex.current_index) {
                if let Err(e) =
                    crate::progress::record_attempt(conn, &exercise.subject, &exercise.id, false)
                {
                    eprintln!("[clings] erreur enregistrement tentative: {e}");
                }
            }
            Self::invalidate_header_cache(&mut self.state);
        }
    }

    /// Export libsys si l'exercice est de type LibraryExport et que tous les champs sont présents.
    fn try_export_libsys(
        &mut self,
        path: &std::path::Path,
        libsys_info: (
            crate::models::ExerciseType,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    ) {
        let (exercise_type, libsys_module, libsys_function, header_code) = libsys_info;
        if exercise_type != crate::models::ExerciseType::LibraryExport {
            return;
        }
        let (Some(module), Some(function), Some(h_decl)) =
            (libsys_module, libsys_function, header_code)
        else {
            return;
        };
        let fn_name = function.clone();
        let c_code = std::fs::read_to_string(path).unwrap_or_default();
        let libsys_export = crate::libsys::LibsysExport {
            module,
            function,
            c_code,
            h_decl,
        };
        let libsys_path = crate::libsys::libsys_path();
        match crate::libsys::export(&libsys_path, &libsys_export) {
            Ok(()) => {
                self.state.session.status_msg = Some(format!("✓ {fn_name} ajouté à libsys !"));
                self.state.session.status_msg_at = Some(std::time::Instant::now());
            }
            Err(e) => {
                self.state.session.status_msg = Some(format!("✗ Erreur libsys: {}", e));
                self.state.session.status_msg_at = Some(std::time::Instant::now());
            }
        }
    }

    /// Check if subject changed, update interleaving nudge if same subject.
    fn update_interleaving_nudge(&mut self, subject: String) {
        let same_subject = subject == self.state.ex.last_success_subject;
        if same_subject {
            self.state.ex.consecutive_successes_on_subject = self
                .state
                .ex
                .consecutive_successes_on_subject
                .saturating_add(1);
            if self.state.ex.consecutive_successes_on_subject >= 2 {
                // Suggest switching subject (TBD: display nudge in UI)
                self.state.ex.consecutive_successes_on_subject = 0;
            }
        } else {
            self.state.ex.last_success_subject = subject;
            self.state.ex.consecutive_successes_on_subject = 1;
        }
    }

    /// Handle file changed notification (ignore if skip_file_changed is set).
    pub(crate) fn handle_file_changed(&mut self) {
        if self.state.session.skip_file_changed {
            self.state.session.skip_file_changed = false;
            // Skip this file change notification (debounce).
        }
        // Auto-compile could happen here, but currently disabled.
    }

    /// Handle tick — clear status message if expired (3 seconds).
    pub(crate) fn handle_tick_status_clear(&mut self) {
        use std::time::Duration;

        if let Some(msg_at) = self.state.session.status_msg_at {
            if msg_at.elapsed() >= Duration::from_secs(3) {
                self.state.session.status_msg = None;
                self.state.session.status_msg_at = None;
            }
        }
    }
}
