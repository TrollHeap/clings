//! Piscine mode event handlers.
//!
//! Handles compilation, navigation, and mode-specific logic for piscine/exam sessions.

use crate::tui::ui_messages::{ActiveOverlay, Command, Msg};

use super::App;

impl App {
    /// Dispatch Piscine messages → état (main update function for piscine loop).
    pub(crate) fn update_piscine(
        &mut self,
        msg: Msg,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
    ) {
        match msg {
            Msg::Key(key) => {
                if self.handle_overlay_dispatch(key, conn, session_id) {
                    return;
                }
                let Some(cmd) = crate::tui::ui_keymap::key_to_cmd(&key, true) else {
                    return;
                };
                match cmd {
                    Command::Quit => {
                        let idx = self.state.ex.current_index;
                        self.save_checkpoint(conn, session_id, idx);
                        if key.modifiers.is_empty() {
                            self.state.overlay.quit_confirm_active = true;
                        } else {
                            // Ctrl+C / Ctrl+Z → quitter immédiatement sans modale
                            self.state.session.should_quit = true;
                        }
                    }
                    Command::ShowHint => self.reveal_next_hint(),
                    Command::ToggleSolution => {
                        if self.can_reveal_solution_piscine() {
                            self.state.overlay.active =
                                if self.state.overlay.active == ActiveOverlay::Solution {
                                    ActiveOverlay::None
                                } else {
                                    ActiveOverlay::Solution
                                };
                        }
                    }
                    Command::OpenVisualizer => self.toggle_visualizer_overlay(),
                    Command::OpenLibsys => self.open_libsys_overlay(),
                    Command::OpenList => self.open_list_overlay(),
                    Command::OpenSearch => self.open_search_overlay(),
                    Command::ScrollDown => {
                        self.state.ex.description_scroll =
                            self.state.ex.description_scroll.saturating_add(3);
                    }
                    Command::ScrollUp => {
                        self.state.ex.description_scroll =
                            self.state.ex.description_scroll.saturating_sub(3);
                    }
                    Command::NavNext => {
                        if !self.navigate_next(conn, session_id) {
                            self.state.session.should_quit = true;
                        }
                    }
                    Command::NavPrev => {
                        self.navigate_prev(conn, session_id);
                    }
                    Command::ShowHelp => {} // non disponible en piscine (filtré par key_to_cmd)
                    Command::CompileRun => {
                        self.handle_compile_piscine(conn, session_id);
                    }
                }
            }
            Msg::FileChanged => {
                // Piscine: no auto-compile on file change
            }
            Msg::Tick => {
                self.update_piscine_timer();
            }
            Msg::Resize(_w, _h) => {
                // Ratatui recalcule le layout à chaque draw — pas d'action requise.
            }
        }
    }

    /// Compile and run in piscine mode (different from watch).
    /// Piscine: no consecutive_failures gate for hints, checkpoint on each attempt.
    fn handle_compile_piscine(&mut self, conn: &rusqlite::Connection, _session_id: Option<&str>) {
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
        let compile_error = result.compile_error;
        self.state.ex.run_result = Some(result);

        if success {
            self.state.piscine.fail_count = 0;
            if !self.state.ex.already_recorded {
                self.state.ex.already_recorded = true;
                if let Err(e) =
                    crate::progress::record_attempt(conn, &exercise.subject, &exercise.id, true)
                {
                    eprintln!("[clings] erreur enregistrement tentative: {e}");
                }
                Self::invalidate_header_cache(&mut self.state);
            }
            self.state.ex.completed[self.state.ex.current_index] = true;
            self.state.overlay.success_overlay = true;
        } else {
            if !compile_error {
                self.state.piscine.fail_count = self.state.piscine.fail_count.saturating_add(1);
                if self.state.piscine.fail_count >= crate::constants::PISCINE_FAILURE_THRESHOLD {
                    if let Some(cm) = &exercise.common_mistake {
                        self.state.session.status_msg = Some(format!("⚠ Piège : {}", cm));
                        self.state.session.status_msg_at = Some(std::time::Instant::now());
                    }
                }
            }
            if let Err(e) =
                crate::progress::record_attempt(conn, &exercise.subject, &exercise.id, false)
            {
                eprintln!("[clings] erreur enregistrement tentative: {e}");
            }
            Self::invalidate_header_cache(&mut self.state);
        }
    }

    /// Update piscine timer cache on each Tick (invalidate string caches when second changes).
    fn update_piscine_timer(&mut self) {
        if let Some(start) = self.state.piscine.start {
            let elapsed = start.elapsed().as_secs();
            if elapsed != self.state.timer_cache.piscine_last_elapsed_secs {
                self.state.timer_cache.piscine_last_elapsed_secs = elapsed;
                self.state.timer_cache.cached_piscine_elapsed_str =
                    format!("⏱ {}m{:02}s", elapsed / 60, elapsed % 60);
            }
        }
        if let Some(deadline) = self.state.piscine.deadline {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if remaining != self.state.timer_cache.piscine_last_remaining_secs {
                self.state.timer_cache.piscine_last_remaining_secs = remaining;
                self.state.timer_cache.cached_piscine_remaining_str =
                    format_remaining_secs(remaining);
            }
        }
    }
}

/// Formate un nombre de secondes restantes en chaîne lisible.
/// Partagé entre `run_piscine()` (init) et le Tick handler.
pub fn format_remaining_secs(remaining: u64) -> String {
    if remaining == 0 {
        "Temps écoulé".to_string()
    } else if remaining >= 60 {
        format!("{}m{:02}s restantes", remaining / 60, remaining % 60)
    } else {
        format!("{}s restantes", remaining)
    }
}
