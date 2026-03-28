//! Application state and event handling (TEA/Elm architecture).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use ratatui::widgets::ListState;

use crate::chapters::CHAPTERS;
use crate::error::Result;
use crate::models::Exercise;
use crate::runner::RunResult;
use crate::search;

pub use crate::tui::ui_keymap::key_to_cmd;
pub use crate::tui::ui_messages::{ActiveOverlay, Command, ListDisplayItem, Msg};

/// État des overlays (help, list, search, solution, visualizer, libsys, nav_confirm).
#[derive(Default, Debug)]
pub struct OverlayState {
    /// Overlay exclusif actif. Les modaux `nav_confirm_active` et `success_overlay`
    /// se superposent par-dessus et restent des booléens indépendants.
    pub active: ActiveOverlay,
    /// Exercice sélectionné dans la liste.
    pub list_selected: usize,
    /// Items à afficher dans la liste (headers de chapitre + exercises).
    pub list_display_items: Vec<ListDisplayItem>,
    /// Requête de recherche actuelle.
    pub search_query: String,
    /// Résultats de recherche (indices dans exercises[]).
    pub search_results: Vec<usize>,
    /// Exercice sélectionné dans les résultats.
    pub search_selected: usize,
    /// Filtre par sujet : `true` = filtre activé.
    pub search_subject_filter: bool,
    /// Flag pour savoir si `g` est pressé (commande 2-chars attendue).
    pub search_g_pending: bool,
    /// Étape actuelle du visualiseur.
    pub vis_step: usize,
    /// Overlay succès — affiché brief après une réussite (modal, se superpose à `active`).
    pub success_overlay: bool,
    /// Modal de confirmation avant changement d'exercice (évite reset accidentel).
    pub nav_confirm_active: bool,
    /// Direction du changement : `true` = next, `false` = prev.
    pub nav_confirm_next: bool,
    /// Modal de confirmation avant de quitter la session.
    pub quit_confirm_active: bool,
    /// ListState persistant pour l'overlay liste [l].
    pub list_list_state: ListState,
    /// ListState persistant pour l'overlay recherche [/].
    pub search_list_state: ListState,
    /// Données du portfolio libsys (cache pour éviter I/O par frame).
    pub libsys_portfolio: Vec<crate::libsys::ModuleStatus>,
}

/// Cache du header — invalidé sur changement d'exercice ou mise à jour mastery.
/// Réduit les allocations string par frame en cachant les chaînes pré-formatées.
#[derive(Default, Debug)]
pub struct HeaderCache {
    /// String pré-formatée de la minimap d'achèvement.
    pub cached_mini_map: String,
    /// Compteur exercices actuels/totaux pré-formaté.
    pub cached_exercise_counter: String,
    /// Affichage du score de maîtrise pré-formaté.
    pub cached_mastery_display: String,
    /// Type d'exercice (complete, fix_bug, fill_blank, refactor, libsys).
    pub cached_exercise_type: String,
    /// Longueur de la partie gauche du header (pour layout).
    pub cached_header_left_len: usize,
    /// Longueur de la minimap (pour layout).
    pub cached_mini_map_len: usize,
    /// Dernier index d'exercice en cache (détecte changement).
    last_cached_index: usize,
    /// Dernier nombre total d'exercices en cache (détecte changement).
    last_cached_total: usize,
    /// Dernier score de maîtrise en cache (détecte changement).
    last_cached_mastery: f64,
}

impl HeaderCache {
    /// Invalide le cache si index/total/mastery ont changé et recompute les strings.
    fn invalidate(
        &mut self,
        idx: usize,
        total: usize,
        mastery: f64,
        exercises: &[Exercise],
        completed: &[bool],
    ) {
        if self.last_cached_index != idx || self.last_cached_total != total {
            self.cached_exercise_counter = format!("[{}/{}] ", idx + 1, total);
            self.last_cached_index = idx;
            self.last_cached_total = total;
        }
        if (self.last_cached_mastery - mastery).abs() > 0.01 {
            self.cached_mastery_display = format!("mastery: {:.1}  ", mastery);
            self.last_cached_mastery = mastery;
        }
        self.cached_mini_map = crate::tui::common::mini_map(completed, idx);
        self.cached_mini_map_len = self.cached_mini_map.chars().count();
        self.cached_exercise_type = exercises
            .get(idx)
            .map(|e| e.exercise_type.to_string())
            .unwrap_or_default();
        let title = exercises.get(idx).map(|e| e.title.as_str()).unwrap_or("");
        self.cached_header_left_len =
            self.cached_exercise_counter.chars().count() + title.chars().count();
    }
}

/// Cache du timer piscine/exam — mis à jour dans Tick quand la seconde change.
/// Évite les allocations string à chaque rendu.
#[derive(Debug)]
pub struct PiscineTimerCache {
    /// Temps écoulé pré-formaté.
    pub cached_piscine_elapsed_str: String,
    /// Dernier temps écoulé en cache (détecte changement).
    pub piscine_last_elapsed_secs: u64,
    /// Temps restant pré-formaté.
    pub cached_piscine_remaining_str: String,
    /// Dernier temps restant en cache (détecte changement).
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

/// Contexte exercice — progression, résultats, indices.
#[derive(Default, Debug)]
pub struct ExerciseCtx {
    /// Tous les exercices chargés (filtrés par chapitre/mode).
    pub exercises: Vec<Exercise>,
    /// Bool par exercice : `true` si mastery >= seuil ou résolu en piscine.
    pub completed: Vec<bool>,
    /// Index courant dans `exercises[]`.
    pub current_index: usize,
    /// Résultat du dernier compile+run : None avant la 1ère tentative.
    pub run_result: Option<RunResult>,
    /// Chemin absolu du fichier ~/.clings/current.c.
    pub source_path: Option<PathBuf>,
    /// Stage courant du code de départ (0–4) ou None si pas de stages.
    pub current_stage: Option<u8>,
    /// Index du dernier indice révélé (0 = aucun).
    pub hint_index: usize,
    /// Nombre d'échecs consécutifs sur l'exercice courant.
    pub consecutive_failures: u8,
    /// `true` si la tentative courante est enregistrée en DB (évite doublons).
    pub already_recorded: bool,
    /// Offset vertical de scroll de la description en watch mode.
    pub description_scroll: u16,
    /// Nombre de succès consécutifs sur le sujet courant (nudge interleaving).
    pub consecutive_successes_on_subject: u8,
    /// Dernier sujet avec succès (pour vérifier changement de sujet).
    pub last_success_subject: String,
}

/// Contexte piscine/examen — timer, état de progression linéaire.
#[derive(Default, Debug)]
pub struct PiscineCtx {
    /// Deadline piscine (Instant du démarrage + durée limite).
    pub deadline: Option<Instant>,
    /// Timestamp du démarrage du mode piscine.
    pub start: Option<Instant>,
    /// Durée totale piscine en secondes (NSY103=9000, UTC502=10800).
    pub timer_total: u64,
    /// Nombre d'échecs accumulés en piscine.
    pub fail_count: u32,
}

/// Contexte session runtime — flags de contrôle, messages, éditeur.
#[derive(Default, Debug)]
pub struct SessionCtx {
    /// `true` si l'utilisateur a appuyé sur [q] ou fermé le terminal.
    pub should_quit: bool,
    /// `true` si compile est programmé (attend next Tick).
    pub compile_pending: bool,
    /// `true` pour ignorer le prochain FileChanged (ex. après write_starter_code).
    pub skip_file_changed: bool,
    /// Message status courant (ex. "Fichier sauvegardé").
    pub status_msg: Option<String>,
    /// Timestamp du message status (pour expiration après 3s).
    pub status_msg_at: Option<Instant>,
    /// Nom du pane tmux (ex. "pane-1:2") si intégration active.
    pub editor_pane: Option<String>,
}

/// Contexte progression — mastery, révisions, ordre curriculum.
#[derive(Default, Debug)]
pub struct ProgressCtx {
    /// Subject → days_until_review (négatif = due). Calculé au démarrage.
    pub review_map: HashMap<String, Option<i64>>,
    /// Subject → mastery score courant (depuis DB). Calculé au démarrage.
    pub mastery_map: HashMap<String, f64>,
    /// Ordre des sujets (pour sidebar navigation).
    pub subject_order: Vec<String>,
    /// Subject → chapter number (pour résumé par chapitre).
    pub subject_to_chapter: HashMap<String, usize>,
    /// Nombre de sujets en révision (cached, invalidé quand review_map change).
    pub cached_due_count: Option<usize>,
}

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

    /// Gère les touches de l'overlay liste.
    /// Retourne `true` si Enter a été pressé (jump-to-exercise).
    fn dispatch_list_overlay_key(
        state: &mut AppState,
        key: ratatui::crossterm::event::KeyEvent,
    ) -> bool {
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
    fn dispatch_search_overlay_key(
        state: &mut AppState,
        key: ratatui::crossterm::event::KeyEvent,
    ) -> bool {
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
            // Vim : l / → = step suivant ; h / ← = step précédent ; j/k aussi intuitifs
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Char('j') => {
                let total = state
                    .ex
                    .exercises
                    .get(state.ex.current_index)
                    .expect("current_index in bounds")
                    .visualizer
                    .steps
                    .len();
                state.overlay.vis_step = (state.overlay.vis_step + 1).min(total.saturating_sub(1));
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('k') => {
                state.overlay.vis_step = state.overlay.vis_step.saturating_sub(1);
            }
            _ => {
                state.overlay.active = ActiveOverlay::None;
            }
        }
    }

    /// Gère les touches de l'overlay solution.
    /// Retourne `true` si l'overlay était actif (l'appelant doit `return`).
    fn handle_solution_overlay(
        state: &mut AppState,
        key: ratatui::crossterm::event::KeyEvent,
    ) -> bool {
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
        if let Err(e) = self.reset_state_and_load_exercise(conn) {
            eprintln!("[clings] erreur chargement exercice: {e}");
        }
        if let Some(sid) = session_id {
            let idx = self.state.ex.current_index;
            self.save_checkpoint(conn, Some(sid), idx);
        }
    }

    /// Handle success overlay key press (any key closes, Enter navigates to next).
    fn handle_success_overlay_key(
        &mut self,
        key: ratatui::crossterm::event::KeyEvent,
        conn: &rusqlite::Connection,
        session_id: Option<&str>,
    ) {
        use ratatui::crossterm::event::KeyCode;
        self.state.overlay.success_overlay = false;
        if matches!(key.code, KeyCode::Enter) && !self.navigate_next(conn, session_id) {
            self.state.session.should_quit = true;
        }
    }

    /// Dispatch overlay keys shared between watch and piscine.
    /// Returns `true` if the key was handled by an overlay (caller should `return`).
    /// If an overlay navigation triggers a jump, calls `reset_state_and_load_exercise`.
    fn handle_overlay_dispatch(
        &mut self,
        key: ratatui::crossterm::event::KeyEvent,
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

    /// Shared hint reveal handler `[h]`.
    fn reveal_next_hint(&mut self) {
        use crate::constants::HINT_MIN_ATTEMPTS;
        let hints_len = self
            .state
            .ex
            .exercises
            .get(self.state.ex.current_index)
            .expect("current_index in bounds")
            .hints
            .len();
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
    fn toggle_visualizer_overlay(&mut self) {
        if let Some(exercise) = self.state.ex.exercises.get(self.state.ex.current_index) {
            if !exercise.visualizer.steps.is_empty() {
                self.state.overlay.active = ActiveOverlay::Visualizer;
                self.state.overlay.vis_step = 0;
            }
        }
    }

    /// Shared list overlay open `[l]`.
    fn open_list_overlay(&mut self) {
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
    fn open_libsys_overlay(&mut self) {
        let path = crate::libsys::libsys_path();
        self.state.overlay.libsys_portfolio =
            crate::libsys::portfolio_status(&path).unwrap_or_default();
        self.state.overlay.active = ActiveOverlay::Libsys;
    }

    /// Shared search overlay open `[/]`.
    fn open_search_overlay(&mut self) {
        self.state.overlay.active = ActiveOverlay::Search;
        self.state.overlay.search_subject_filter = false;
        self.state.overlay.search_query.clear();
        Self::rebuild_search(&mut self.state);
    }

    /// Navigate to next exercise, optionally saving checkpoint.
    /// Returns `true` if navigation happened, `false` if at end.
    fn navigate_next(&mut self, conn: &rusqlite::Connection, session_id: Option<&str>) -> bool {
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
            if let Some(sid) = session_id {
                self.save_checkpoint(conn, Some(sid), self.state.ex.current_index);
            }
            false
        }
    }

    /// Navigate to previous exercise, optionally saving checkpoint.
    fn navigate_prev(&mut self, conn: &rusqlite::Connection, session_id: Option<&str>) {
        if self.state.ex.current_index > 0 {
            self.state.ex.current_index -= 1;
            if let Err(e) = self.reset_state_and_load_exercise(conn) {
                eprintln!("[clings] erreur chargement exercice: {e}");
            }
            if let Some(sid) = session_id {
                let idx = self.state.ex.current_index;
                self.save_checkpoint(conn, Some(sid), idx);
            }
        }
    }

    /// Check if solution can be revealed (all hints shown OR enough failures).
    fn can_reveal_solution(&self, failure_threshold: usize) -> bool {
        let exercise = self
            .state
            .ex
            .exercises
            .get(self.state.ex.current_index)
            .expect("current_index in bounds");
        let all_shown =
            exercise.hints.is_empty() || self.state.ex.hint_index >= exercise.hints.len();
        all_shown || self.state.ex.consecutive_failures as usize >= failure_threshold
    }

    /// Check if piscine solution can be revealed (fail_count-based threshold).
    fn can_reveal_solution_piscine(&self) -> bool {
        let exercise = self
            .state
            .ex
            .exercises
            .get(self.state.ex.current_index)
            .expect("current_index in bounds");
        let all_shown =
            exercise.hints.is_empty() || self.state.ex.hint_index >= exercise.hints.len();
        all_shown || self.state.piscine.fail_count >= crate::constants::PISCINE_FAILURE_THRESHOLD
    }

    /// Handle compilation and test of current exercise (triggered by 'r' key).
    /// Compiles the code, runs it, records attempt, and navigates on success.
    fn handle_compile(&mut self, conn: &rusqlite::Connection) {
        use crate::constants::CONSECUTIVE_FAILURE_THRESHOLD;

        let Some(path) = self.state.ex.source_path.as_deref() else {
            return;
        };
        self.state.session.compile_pending = true;
        let exercise = self
            .state
            .ex
            .exercises
            .get(self.state.ex.current_index)
            .expect("current_index in bounds");
        let result = crate::runner::compile_and_run(path, exercise);
        self.state.session.compile_pending = false;
        let success = result.success;
        self.state.ex.run_result = Some(result);

        if success {
            self.state.ex.consecutive_failures = 0;
            if !self.state.ex.already_recorded {
                self.state.ex.already_recorded = true;
                // Clone avant tout borrow mutable
                let subject = exercise.subject.clone();
                let exercise_id = exercise.id.clone();
                if let Err(e) = crate::progress::record_attempt(conn, &subject, &exercise_id, true)
                {
                    eprintln!("[clings] erreur enregistrement tentative: {e}");
                }
                // Export libsys si c'est un exercice library_export
                if exercise.exercise_type == crate::models::ExerciseType::LibraryExport {
                    if let (Some(module), Some(function), Some(h_decl)) = (
                        exercise.libsys_module.clone(),
                        exercise.libsys_function.clone(),
                        exercise.header_code.clone(),
                    ) {
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
                                self.state.session.status_msg =
                                    Some(format!("✓ {fn_name} ajouté à libsys !"));
                                self.state.session.status_msg_at = Some(std::time::Instant::now());
                            }
                            Err(e) => eprintln!("[clings] erreur export libsys: {e}"),
                        }
                    }
                }
                Self::invalidate_header_cache(&mut self.state);
                // Interleaving nudge : suggérer de changer de sujet après N succès consécutifs
                if self.state.ex.last_success_subject == subject {
                    self.state.ex.consecutive_successes_on_subject = self
                        .state
                        .ex
                        .consecutive_successes_on_subject
                        .saturating_add(1);
                } else {
                    self.state.ex.consecutive_successes_on_subject = 1;
                    self.state.ex.last_success_subject = subject.clone();
                }
                if self.state.ex.consecutive_successes_on_subject
                    >= crate::constants::INTERLEAVING_NUDGE_THRESHOLD
                {
                    self.state.session.status_msg = Some(format!(
                        "{} succès sur «{}» — explorer un autre sujet booste la mémoire !",
                        crate::constants::INTERLEAVING_NUDGE_THRESHOLD,
                        subject
                    ));
                    self.state.session.status_msg_at = Some(std::time::Instant::now());
                    self.state.ex.consecutive_successes_on_subject = 0;
                }
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
                    .ex
                    .exercises
                    .get(self.state.ex.current_index)
                    .expect("current_index in bounds")
                    .hints
                    .len();
                if hints_len > 0 {
                    self.state.ex.hint_index = 1;
                }
            }
            let exercise = self
                .state
                .ex
                .exercises
                .get(self.state.ex.current_index)
                .expect("current_index in bounds");
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

        match msg {
            Msg::Key(key) => {
                if self.handle_overlay_dispatch(key, conn, None) {
                    return;
                }
                let Some(cmd) = key_to_cmd(&key, false) else {
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

    /// Dispatch Piscine messages → état
    pub fn update_piscine(
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
                let Some(cmd) = key_to_cmd(&key, true) else {
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
                        let Some(path) = self.state.ex.source_path.as_deref() else {
                            return;
                        };
                        self.state.session.compile_pending = true;
                        let exercise = self
                            .state
                            .ex
                            .exercises
                            .get(self.state.ex.current_index)
                            .expect("current_index in bounds");
                        let result = crate::runner::compile_and_run(path, exercise);
                        self.state.session.compile_pending = false;
                        let success = result.success;
                        let compile_error = result.compile_error;
                        self.state.ex.run_result = Some(result);

                        if success {
                            self.state.piscine.fail_count = 0;
                            if !self.state.ex.already_recorded {
                                self.state.ex.already_recorded = true;
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
                            self.state.ex.completed[self.state.ex.current_index] = true;
                            self.state.overlay.success_overlay = true;
                        } else {
                            if !compile_error {
                                self.state.piscine.fail_count =
                                    self.state.piscine.fail_count.saturating_add(1);
                                if self.state.piscine.fail_count
                                    >= crate::constants::PISCINE_FAILURE_THRESHOLD
                                {
                                    if let Some(cm) = &exercise.common_mistake {
                                        self.state.session.status_msg =
                                            Some(format!("⚠ Piège : {}", cm));
                                        self.state.session.status_msg_at = Some(Instant::now());
                                    }
                                }
                            }
                            let exercise = self
                                .state
                                .ex
                                .exercises
                                .get(self.state.ex.current_index)
                                .expect("current_index in bounds");
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
            }
            Msg::FileChanged => self.handle_file_changed(),
            Msg::Resize(_w, _h) => {
                // Ratatui recalcule le layout à chaque draw — pas d'action requise.
            }
            Msg::Tick => {
                self.handle_tick_status_clear();
                // Mise à jour du cache timer elapsed (1 allocation/seconde max)
                if let Some(start) = self.state.piscine.start {
                    let elapsed = start.elapsed().as_secs();
                    if elapsed != self.state.timer_cache.piscine_last_elapsed_secs {
                        self.state.timer_cache.piscine_last_elapsed_secs = elapsed;
                        self.state.timer_cache.cached_piscine_elapsed_str =
                            format!("⏱ {}m{:02}s", elapsed / 60, elapsed % 60);
                    }
                }
                // Mise à jour du cache timer restant (1 allocation/seconde max)
                if let Some(deadline) = self.state.piscine.deadline {
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
                        let idx = self.state.ex.current_index;
                        self.save_checkpoint(conn, session_id, idx);
                        self.state.session.should_quit = true;
                    }
                }
            }
        }
    }

    /// Mise à jour commune FileChanged — enregistre le message de status.
    fn handle_file_changed(&mut self) {
        if self.state.session.skip_file_changed {
            self.state.session.skip_file_changed = false;
            return;
        }
        self.state.session.status_msg = Some("fichier sauvegardé — [r] pour compiler".to_string());
        self.state.session.status_msg_at = Some(Instant::now());
    }

    /// Mise à jour commune Tick — expire le message de status après timeout.
    fn handle_tick_status_clear(&mut self) {
        if let Some(at) = self.state.session.status_msg_at {
            if at.elapsed()
                > std::time::Duration::from_secs(crate::constants::STATUS_MSG_TIMEOUT_SECS)
            {
                self.state.session.status_msg = None;
                self.state.session.status_msg_at = None;
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
