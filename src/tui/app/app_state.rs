//! TEA/Elm state types — all structs and their basic impls for AppState and related contexts.
//!
//! Extracted from app.rs to reduce its size. The event-loop logic (`impl App`) remains in app.rs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use ratatui::widgets::ListState;

use crate::models::Exercise;
use crate::runner::RunResult;
use crate::tui::ui_messages::ActiveOverlay;

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
    pub(crate) fn invalidate(
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

// AppState, impl AppState, and App struct are defined in mod.rs.
// ListDisplayItem is used for OverlayState::list_display_items.
use crate::tui::ui_messages::ListDisplayItem;
