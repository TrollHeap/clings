//! Shared constants for the clings application.

// === Timing ===
/// Délai maximal d'exécution d'un programme C (secondes).
pub const EXECUTION_TIMEOUT_SECS: u64 = 10;
/// Secondes par minute.
pub const SECS_PER_MINUTE: u64 = 60;
/// Secondes par heure.
pub const SECS_PER_HOUR: u64 = 3600;
/// Secondes par jour.
pub const SECS_PER_DAY: i64 = 86_400;
/// Délai de debounce pour le file watcher (ms).
pub const DEBOUNCE_INTERVAL_MS: u64 = 200;
/// Timeout pour vérifier les entrées clavier (ms).
pub const KEY_CHECK_TIMEOUT_MS: u64 = 50;
/// Timeout SQLite quand la DB est verrouillée (ms).
pub const DB_BUSY_TIMEOUT_MS: i32 = 5000;
/// Durée de pause après la résolution d'un exercice (secondes).
pub const SUCCESS_PAUSE_SECS: u64 = 2;
/// Durée d'affichage des messages status (secondes).
pub const STATUS_MSG_TIMEOUT_SECS: u64 = 3;
/// Intervalle de polling des événements TUI (ms).
pub const EVENT_POLL_MS: u64 = 100;
/// Timeout réception events TUI (ms) — recv_timeout dans la render loop.
pub const RENDER_RECV_TIMEOUT_MS: u64 = 50;
/// Durée de l'examen NSY103 simulé (minutes).
pub const EXAM_NSY103_DURATION_MINS: u64 = 150;
/// Durée de l'examen UTC502 simulé (minutes).
pub const EXAM_UTC502_DURATION_MINS: u64 = 180;

// === Mastery SRS ===
/// Score de maîtrise maximal (cap).
pub const MASTERY_MAX: f64 = 5.0;
/// Score de maîtrise minimal (floor).
pub const MASTERY_MIN: f64 = 0.0;
/// Incrément de score après une réponse correcte.
pub const MASTERY_SUCCESS_DELTA: f64 = 1.0;
/// Décrémentation de score après une réponse incorrecte.
pub const MASTERY_FAILURE_DELTA: f64 = 0.5;
/// Nombre de jours avant que le score décroisse (inactivité).
pub const MASTERY_DECAY_DAYS: i64 = 14;
/// Multiplicateur d'intervalle SRS après chaque succès (2.5x).
pub const SRS_INTERVAL_MULTIPLIER: f64 = 2.5;
/// Intervalle SRS minimal après succès (jours).
pub const SRS_BASE_INTERVAL_DAYS: i64 = 1;
/// Intervalle SRS maximal (jours).
pub const SRS_MAX_INTERVAL_DAYS: i64 = 60;
/// Score seuil de déverrouillage : Difficulté 2 (Easy → Medium).
pub const DIFFICULTY_2_UNLOCK: f64 = 2.0;
/// Score seuil de déverrouillage : Difficulté 3 (Medium → Hard).
pub const DIFFICULTY_3_UNLOCK: f64 = 4.0;
/// Score seuil de déverrouillage : Difficulté 4 (Hard → Advanced).
pub const DIFFICULTY_4_UNLOCK: f64 = 4.5;
/// Score seuil de déverrouillage : Difficulté 5 (Advanced → Expert).
pub const DIFFICULTY_5_UNLOCK: f64 = 5.0;

// === Status bar layout ===
/// Largeur minimale pour les touches de raccourci dans la status bar.
pub const STATUS_BAR_KEY_MIN_WIDTH: u16 = 15;
/// Espacement entre les éléments de la status bar.
pub const STATUS_BAR_SPACING: u16 = 10;

// === UI dimensions ===
/// Largeur du header de chapitre/exercice.
pub const HEADER_WIDTH: usize = 56;
/// Largeur de la barre de progression générale.
pub const PROGRESS_BAR_WIDTH: usize = 30;
/// Largeur de la barre de maîtrise (star count).
pub const MASTERY_BAR_WIDTH: usize = 10;
/// Largeur de la barre de progression en mode piscine.
pub const PISCINE_PROGRESS_BAR_WIDTH: usize = 20;
/// Largeur de wrapping pour le texte (description, etc.).
pub const TEXT_WRAP_WIDTH: usize = 72;
/// Largeur de la colonne sujet dans les statistiques.
pub const STATS_NAME_WIDTH: usize = 22;
/// Largeur de l'affichage exercice dans les scores.
pub const SCORES_EXERCISE_WIDTH: usize = 32;
/// Largeur du nom sujet dans le rapport par chapitre.
pub const PROGRESS_SUBJECT_WIDTH: usize = 20;
/// Largeur totale de la ligne dans le rapport.
pub const PROGRESS_HR_WIDTH: usize = 58;

// === Display thresholds ===
/// Seuil de pourcentage pour afficher en vert (≥ 75%).
pub const PCT_GREEN_THRESHOLD: u32 = 75;
/// Seuil de pourcentage pour afficher en jaune (< 25%).
pub const PCT_YELLOW_THRESHOLD: u32 = 25;
/// Nombre maximal d'exercices dans la minimap.
pub const MINIMAP_MAX_ITEMS: usize = 60;
/// Seuil d'échecs consécutifs avant unlock de la solution.
pub const CONSECUTIVE_FAILURE_THRESHOLD: usize = 3;
/// Nombre minimum de tentatives avant le 1er indice accessible.
pub const HINT_MIN_ATTEMPTS: u8 = 2;
/// Nombre de succès consécutifs sur un sujet avant nudge d'interleaving.
pub const INTERLEAVING_NUDGE_THRESHOLD: u8 = 3;
/// Seuil de piscine/exam (plus bas car progression linéaire).
pub const PISCINE_FAILURE_THRESHOLD: u32 = 2;
/// Score seuil pour afficher la barre maîtrise en vert.
pub const MASTERY_BAR_GREEN_THRESHOLD: f64 = 4.0;
/// Score seuil pour afficher la barre maîtrise en jaune.
pub const MASTERY_BAR_YELLOW_THRESHOLD: f64 = 2.0;
/// Nombre de sujets à afficher en top X dans les statistiques.
pub const STATS_TOP_SUBJECTS_COUNT: usize = 5;

// === Limits ===
/// Taille maximale de stdout autorisée (octets).
pub const MAX_OUTPUT_BYTES: u64 = 1024 * 1024;

// === Compiler ===
/// Binaire gcc à utiliser pour la compilation.
pub const GCC_BINARY: &str = "gcc";
/// Flags gcc par défaut (warnings, standard C11, source GNU).
pub const GCC_FLAGS: &[&str] = &["-Wall", "-Wextra", "-std=c11", "-D_GNU_SOURCE"];
/// Préfixe pour les patterns regex dans expected_output.
pub const REGEX_PREFIX: &str = "REGEX:";

// === Paths & keys ===
/// Répertoire par défaut relatif à $HOME.
pub const CLINGS_DIR: &str = ".clings";
/// Nom du fichier de base de données SQLite.
pub const DB_FILENAME: &str = "progress.db";
/// Nom du fichier C généré (code utilisateur courant).
pub const CURRENT_C_FILENAME: &str = "current.c";
/// Variable d'env pour override du répertoire exercices.
pub const EXERCISES_ENV_VAR: &str = "CLINGS_EXERCISES";
/// Variable d'env pour override du répertoire ~/.clings.
pub const CLINGS_HOME_ENV_VAR: &str = "CLINGS_HOME";

// === Config file ===
/// Nom du fichier de configuration utilisateur.
pub const CONFIG_TOML_FILENAME: &str = "clings.toml";
/// Section TOML pour la configuration SRS.
pub const CONFIG_SECTION_SRS: &str = "srs";
/// Section TOML pour la configuration UI.
pub const CONFIG_SECTION_UI: &str = "ui";
/// Section TOML pour la configuration tmux.
pub const CONFIG_SECTION_TMUX: &str = "tmux";
/// Section TOML pour la configuration sync.
pub const CONFIG_SECTION_SYNC: &str = "sync";

/// Résout le répertoire de données clings.
/// Priorité : `CLINGS_HOME` env var > `$HOME/.clings` > `/tmp/.clings`
pub fn clings_data_dir() -> std::path::PathBuf {
    if let Ok(custom) = std::env::var(CLINGS_HOME_ENV_VAR) {
        return std::path::PathBuf::from(custom);
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE")) // Windows fallback
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join(CLINGS_DIR)
}
/// Clé de checkpoint pour le mode piscine dans la DB.
pub const PISCINE_CHECKPOINT_KEY: &str = "piscine_checkpoint";
/// Clé de checkpoint pour le mode exam dans la DB.
pub const EXAM_CHECKPOINT_KEY: &str = "exam_checkpoint";
/// Clé de la dernière session d'examen visitée.
pub const LAST_EXAM_SESSION_KEY: &str = "last_exam_session";
/// Version courante du schéma SQLite (pour migrations).
pub const DB_USER_VERSION_CURRENT: i32 = 1;

// === tmux ===
/// Largeur du pane tmux (pourcentage écran).
pub const TMUX_PANE_WIDTH_PERCENT: &str = "50";
/// Éditeur par défaut pour tmux.
pub const TMUX_EDITOR: &str = "nvim";

// === Sync ===
/// Nom du fichier snapshot JSON pour la synchronisation.
pub const SYNC_SNAPSHOT_FILENAME: &str = "progress.json";
/// Contenu du .gitignore pour le sync.
pub const SYNC_GITIGNORE_CONTENT: &str = "# clings sync — seul progress.json est versionné\n*.db\n*.db-wal\n*.db-shm\n*.toml\n*.c\n*.h\n";
/// Branche Git par défaut pour la synchronisation.
pub const SYNC_DEFAULT_BRANCH: &str = "main";
/// Timeout pour les opérations Git (secondes).
pub const SYNC_GIT_TIMEOUT_SECS: u64 = 10;

// === ANSI escape sequences ===
/// Séquence ANSI pour effacer l'écran.
pub const ANSI_CLEAR_SCREEN: &str = "\x1b[2J\x1b[H";

// === UI messages ===
/// Message pour inviter l'utilisateur à appuyer sur une touche.
pub const MSG_PRESS_KEY_RETURN: &str = "Appuyez sur une touche pour revenir...";
/// Message affiché après la résolution d'un exercice.
pub const MSG_EXERCISE_SOLVED_ADVANCING: &str = "Exercice résolu ! Avancement dans 2s...";

// === Test harness output tokens ===
/// Token pour le nombre de tests dans la sortie Unity.
pub const TEST_SUMMARY_TESTS: &str = "Tests";
/// Token pour le nombre d'échecs dans la sortie Unity.
pub const TEST_SUMMARY_FAILURES: &str = "Failures";
/// Token pour le nombre de tests ignorés dans la sortie Unity.
pub const TEST_SUMMARY_IGNORED: &str = "Ignored";

// === TUI result messages ===
/// Message d'erreur : compilation échouée.
pub const MSG_COMPILE_ERROR: &str = "✗ ERREUR DE COMPILATION";
/// Message d'erreur : timeout d'exécution.
pub const MSG_TIMEOUT: &str = "✗ TIMEOUT";
/// Message d'erreur : tests unitaires échoués.
pub const MSG_TESTS_FAILED: &str = "✗ TESTS ÉCHOUÉS";
/// Message d'erreur : sortie incorrecte.
pub const MSG_WRONG_OUTPUT: &str = "✗ SORTIE INCORRECTE";

// === Security ===
/// Longueur maximale d'un pattern regex (anti-ReDoS).
pub const MAX_REGEX_PATTERN_LEN: usize = 500;
