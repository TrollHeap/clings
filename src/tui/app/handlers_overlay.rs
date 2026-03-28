//! Overlay event handlers shared between watch and piscine modes.
//!
//! Handles: list, search, solution, help, visualizer, libsys, nav_confirm, quit_confirm, success overlays.

use ratatui::crossterm::event::KeyEvent;

use crate::tui::ui_messages::{ActiveOverlay, ListDisplayItem};

use super::{App, AppState};

impl App {
    /// Dispatch overlay keys shared between watch and piscine.
    /// Returns `true` if the key was handled by an overlay (caller should `return`).
    /// If an overlay navigation triggers a jump, calls `reset_state_and_load_exercise`.
    pub(crate) fn handle_overlay_dispatch(
        &mut self,
        key: KeyEvent,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
    ) -> bool {
        if self.state.overlay.quit_confirm_active {
            use ratatui::crossterm::event::KeyCode;
            self.state.overlay.quit_confirm_active = false;
            if matches!(
                key.code,
                KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Enter
            ) {
                self.state.session.should_quit = true;
            }
            return true;
        }
        if self.state.overlay.nav_confirm_active {
            use ratatui::crossterm::event::KeyCode;
            self.state.overlay.nav_confirm_active = false;
            if matches!(
                key.code,
                KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Enter
            ) {
                if self.state.overlay.nav_confirm_next {
                    if !self.navigate_next(conn, session_id) {
                        self.state.session.should_quit = true;
                    }
                } else {
                    self.navigate_prev(conn, session_id);
                }
            }
            return true;
        }
        if self.state.overlay.success_overlay {
            self.handle_success_overlay_key(key, conn, session_id);
            return true;
        }
        match self.state.overlay.active {
            ActiveOverlay::List => {
                if Self::dispatch_list_overlay_key(&mut self.state, key) {
                    self.load_exercise_and_checkpoint(conn, session_id);
                }
                return true;
            }
            ActiveOverlay::Search => {
                if Self::dispatch_search_overlay_key(&mut self.state, key) {
                    self.load_exercise_and_checkpoint(conn, session_id);
                }
                return true;
            }
            ActiveOverlay::Solution => {
                Self::handle_solution_overlay(&mut self.state, key);
                return true;
            }
            ActiveOverlay::Help => {
                self.state.overlay.active = ActiveOverlay::None;
                return true;
            }
            ActiveOverlay::Visualizer => {
                Self::handle_vis_key(&mut self.state, key);
                return true;
            }
            ActiveOverlay::Libsys => {
                self.state.overlay.active = ActiveOverlay::None;
                return true;
            }
            ActiveOverlay::None => {}
        }
        false
    }

    /// Gère les touches de l'overlay de liste.
    /// Retourne `true` si Enter a été pressé avec un exercice sélectionné
    /// (l'appelant doit alors charger l'exercice et/ou sauvegarder le checkpoint).
    fn dispatch_list_overlay_key(state: &mut AppState, key: KeyEvent) -> bool {
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
                state.overlay.active = ActiveOverlay::None;
            }
            return false;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('l') | KeyCode::Char('L') => {
                state.overlay.active = ActiveOverlay::None;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                state.overlay.active = ActiveOverlay::None;
            }
            KeyCode::Enter => {
                if let Some(ListDisplayItem::Exercise { exercise_index }) = state
                    .overlay
                    .list_display_items
                    .get(state.overlay.list_selected)
                {
                    state.ex.current_index = *exercise_index;
                    state.overlay.active = ActiveOverlay::None;
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
    fn dispatch_search_overlay_key(state: &mut AppState, key: KeyEvent) -> bool {
        use ratatui::crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                state.overlay.active = ActiveOverlay::None;
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
                    state.ex.current_index = idx;
                    state.overlay.active = ActiveOverlay::None;
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
                state.overlay.search_g_pending = false;
                Self::rebuild_search(state);
            }
            KeyCode::Char(c) => {
                if c.is_alphanumeric() || " _-".contains(c) {
                    state.overlay.search_query.push(c);
                    state.overlay.search_g_pending = false;
                    Self::rebuild_search(state);
                }
            }
            _ => {}
        }
        false
    }

    /// Retourne `true` si l'overlay était actif (l'appelant doit `return`).
    fn handle_solution_overlay(state: &mut AppState, key: KeyEvent) -> bool {
        if state.overlay.active != ActiveOverlay::Solution {
            return false;
        }
        if matches!(
            key.code,
            ratatui::crossterm::event::KeyCode::Esc
                | ratatui::crossterm::event::KeyCode::Char('s')
                | ratatui::crossterm::event::KeyCode::Char('S')
        ) {
            state.overlay.active = ActiveOverlay::None;
        }
        true
    }

    /// Handle visualizer key press (arrow keys to step, any other key to close).
    fn handle_vis_key(state: &mut AppState, key: KeyEvent) {
        use ratatui::crossterm::event::KeyCode;
        match key.code {
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                if let Some(ex) = state.ex.exercises.get(state.ex.current_index) {
                    if state.overlay.vis_step < ex.visualizer.steps.len().saturating_sub(1) {
                        state.overlay.vis_step += 1;
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => {
                state.overlay.vis_step = state.overlay.vis_step.saturating_sub(1);
            }
            _ => {
                state.overlay.active = ActiveOverlay::None;
            }
        }
    }

    /// Handle success overlay key press (any key closes, Enter navigates to next).
    fn handle_success_overlay_key(
        &mut self,
        key: KeyEvent,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
    ) {
        use ratatui::crossterm::event::KeyCode;
        self.state.overlay.success_overlay = false;
        if matches!(key.code, KeyCode::Enter) && !self.navigate_next(conn, session_id) {
            self.state.session.should_quit = true;
        }
    }

    /// Sauvegarde le checkpoint piscine et exam (si session_id présent).
    /// Logue les erreurs sans les propager — contexte event loop.
    pub(crate) fn save_checkpoint(
        &self,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
        idx: usize,
    ) {
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
    pub(crate) fn load_exercise_and_checkpoint(
        &mut self,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
    ) {
        if let Err(e) = self.reset_state_and_load_exercise(conn) {
            eprintln!("[clings] erreur chargement exercice: {e}");
        }
        if let Some(sid) = session_id {
            let idx = self.state.ex.current_index;
            self.save_checkpoint(conn, Some(sid), idx);
        }
    }

    /// Shared hint reveal handler `[h]`.
    pub(crate) fn reveal_next_hint(&mut self) {
        use crate::constants::HINT_MIN_ATTEMPTS;
        let hints_len = self
            .state
            .current_ex()
            .map(|ex| ex.hints.len())
            .unwrap_or(0);
        if hints_len == 0 {
            return;
        }
        // Gate : le 1er indice nécessite HINT_MIN_ATTEMPTS tentatives préalables.
        if self.state.ex.hint_index == 0 && self.state.ex.consecutive_failures < HINT_MIN_ATTEMPTS {
            let remaining = HINT_MIN_ATTEMPTS - self.state.ex.consecutive_failures;
            self.state.session.status_msg = Some(format!(
                "Essayez encore ({} tentative{} avant le 1er indice)",
                remaining,
                if remaining > 1 { "s" } else { "" }
            ));
            self.state.session.status_msg_at = Some(std::time::Instant::now());
            return;
        }
        self.state.ex.hint_index = (self.state.ex.hint_index + 1).min(hints_len);
    }

    /// Shared visualizer toggle `[v]`.
    pub(crate) fn toggle_visualizer_overlay(&mut self) {
        if let Some(exercise) = self.state.ex.exercises.get(self.state.ex.current_index) {
            if !exercise.visualizer.steps.is_empty() {
                self.state.overlay.active = ActiveOverlay::Visualizer;
                self.state.overlay.vis_step = 0;
            }
        }
    }

    /// Shared list overlay open `[l]`.
    pub(crate) fn open_list_overlay(&mut self) {
        Self::build_list_display_items(&mut self.state);
        self.state.overlay.active = ActiveOverlay::List;
        let ci = self.state.ex.current_index;
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

    /// Portfolio libsys overlay `[b]`.
    pub(crate) fn open_libsys_overlay(&mut self) {
        let path = crate::libsys::libsys_path();
        // NOTE: If portfolio_status fails (missing repo, git error, etc.), silently use default empty portfolio.
        // This is acceptable in TUI context where errors cannot be easily propagated; overlay displays
        // empty state and user can reinvoke or fix the libsys repo.
        self.state.overlay.libsys_portfolio =
            crate::libsys::portfolio_status(&path).unwrap_or_default();
        self.state.overlay.active = ActiveOverlay::Libsys;
    }

    /// Shared search overlay open `[/]`.
    pub(crate) fn open_search_overlay(&mut self) {
        self.state.overlay.active = ActiveOverlay::Search;
        self.state.overlay.search_subject_filter = false;
        self.state.overlay.search_query.clear();
        Self::rebuild_search(&mut self.state);
    }

    /// Navigate to next exercise, optionally saving checkpoint.
    /// Returns `true` if navigation happened, `false` if at end.
    pub(crate) fn navigate_next(
        &mut self,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
    ) -> bool {
        let next = self.state.ex.current_index + 1;
        if next < self.state.ex.exercises.len() {
            self.state.ex.current_index = next;
            if let Err(e) = self.reset_state_and_load_exercise(conn) {
                eprintln!("[clings] erreur chargement exercice: {e}");
            }
            if let Some(sid) = session_id {
                self.save_checkpoint(conn, Some(sid), next);
            }
            true
        } else {
            false
        }
    }

    /// Navigate to previous exercise, optionally saving checkpoint.
    pub(crate) fn navigate_prev(&mut self, conn: &rusqlite::Connection, session_id: Option<&str>) {
        if self.state.ex.current_index > 0 {
            self.state.ex.current_index -= 1;
            if let Err(e) = self.reset_state_and_load_exercise(conn) {
                eprintln!("[clings] erreur chargement exercice: {e}");
            }
            if let Some(sid) = session_id {
                self.save_checkpoint(conn, Some(sid), self.state.ex.current_index);
            }
        }
    }

    /// Determines if solution can be revealed based on hint progress or failures.
    pub(crate) fn can_reveal_solution(&self, failure_threshold: usize) -> bool {
        let Some(exercise) = self.state.current_ex() else {
            return false;
        };
        let all_shown =
            exercise.hints.is_empty() || self.state.ex.hint_index >= exercise.hints.len();
        all_shown || self.state.ex.consecutive_failures as usize >= failure_threshold
    }

    /// Determines if solution can be revealed in piscine mode.
    /// Piscine: via all hints shown OR based on piscine fail count.
    pub(crate) fn can_reveal_solution_piscine(&self) -> bool {
        let Some(exercise) = self.state.current_ex() else {
            return false;
        };
        let all_shown =
            exercise.hints.is_empty() || self.state.ex.hint_index >= exercise.hints.len();
        all_shown || self.state.piscine.fail_count >= crate::constants::PISCINE_FAILURE_THRESHOLD
    }
}
