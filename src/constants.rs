//! Shared constants for the clings application.

// === Timing ===
pub const EXECUTION_TIMEOUT_SECS: u64 = 10;
pub const SECS_PER_DAY: i64 = 86_400;
pub const DEBOUNCE_INTERVAL_MS: u64 = 200;
pub const KEY_CHECK_TIMEOUT_MS: u64 = 50;
pub const DB_BUSY_TIMEOUT_MS: i32 = 5000;
pub const SUCCESS_PAUSE_SECS: u64 = 2;
pub const STATUS_MSG_TIMEOUT_SECS: u64 = 3;
pub const EVENT_POLL_MS: u64 = 100;

// === Mastery thresholds (from mastery.rs) ===
pub const MASTERY_MAX: f64 = 5.0;
pub const MASTERY_MIN: f64 = 0.0;
pub const MASTERY_SUCCESS_DELTA: f64 = 1.0;
pub const MASTERY_FAILURE_DELTA: f64 = 0.5;
pub const MASTERY_DECAY_DAYS: i64 = 14;
pub const SRS_INTERVAL_MULTIPLIER: f64 = 2.5;
pub const SRS_BASE_INTERVAL_DAYS: i64 = 1;
pub const SRS_MAX_INTERVAL_DAYS: i64 = 60;
pub const DIFFICULTY_2_UNLOCK: f64 = 2.0;
pub const DIFFICULTY_3_UNLOCK: f64 = 4.0;
pub const DIFFICULTY_4_UNLOCK: f64 = 4.5;
pub const DIFFICULTY_5_UNLOCK: f64 = 5.0;

// === Status bar layout ===
pub const STATUS_BAR_KEY_MIN_WIDTH: u16 = 15;
pub const STATUS_BAR_SPACING: u16 = 10;

// === UI dimensions ===
pub const HEADER_WIDTH: usize = 56;
pub const PROGRESS_BAR_WIDTH: usize = 30;
pub const MASTERY_BAR_WIDTH: usize = 10;
pub const PISCINE_PROGRESS_BAR_WIDTH: usize = 20;
pub const TEXT_WRAP_WIDTH: usize = 72;
pub const STATS_NAME_WIDTH: usize = 22;
pub const SCORES_EXERCISE_WIDTH: usize = 32;
pub const PROGRESS_SUBJECT_WIDTH: usize = 20;
pub const PROGRESS_HR_WIDTH: usize = 58;

// === Display thresholds ===
pub const PCT_GREEN_THRESHOLD: u32 = 75;
pub const PCT_YELLOW_THRESHOLD: u32 = 25;
pub const MINIMAP_MAX_ITEMS: usize = 60;
pub const CONSECUTIVE_FAILURE_THRESHOLD: usize = 3;
/// Seuil de piscine/exam : plus bas car la progression est linéaire sans navigation libre.
pub const PISCINE_FAILURE_THRESHOLD: u32 = 2;
pub const MASTERY_BAR_GREEN_THRESHOLD: f64 = 4.0;
pub const MASTERY_BAR_YELLOW_THRESHOLD: f64 = 2.0;
pub const STATS_TOP_SUBJECTS_COUNT: usize = 5;

// === Limits ===
pub const MAX_OUTPUT_BYTES: u64 = 1024 * 1024;

// === Compiler ===
pub const GCC_BINARY: &str = "gcc";
pub const GCC_FLAGS: &[&str] = &["-Wall", "-Wextra", "-std=c11", "-D_GNU_SOURCE"];
pub const REGEX_PREFIX: &str = "REGEX:";

// === Paths & keys ===
pub const CLINGS_DIR: &str = ".clings";
pub const DB_FILENAME: &str = "progress.db";
pub const CURRENT_C_FILENAME: &str = "current.c";
pub const EXERCISES_ENV_VAR: &str = "CLINGS_EXERCISES";
pub const PISCINE_CHECKPOINT_KEY: &str = "piscine_checkpoint";
pub const EXAM_CHECKPOINT_KEY: &str = "exam_checkpoint";
pub const LAST_EXAM_SESSION_KEY: &str = "last_exam_session";
pub const DB_USER_VERSION_CURRENT: i32 = 1;

// === tmux ===
pub const TMUX_PANE_WIDTH_PERCENT: &str = "50";
pub const TMUX_EDITOR: &str = "nvim";

// === ANSI escape sequences ===
pub const ANSI_CLEAR_SCREEN: &str = "\x1b[2J\x1b[H";

// === UI messages ===
pub const MSG_PRESS_KEY_RETURN: &str = "Appuyez sur une touche pour revenir...";
pub const MSG_EXERCISE_SOLVED_ADVANCING: &str = "Exercice résolu ! Avancement dans 2s...";

// === Test harness output tokens ===
pub const TEST_SUMMARY_TESTS: &str = "Tests";
pub const TEST_SUMMARY_FAILURES: &str = "Failures";
pub const TEST_SUMMARY_IGNORED: &str = "Ignored";

// === TUI result messages ===
pub const MSG_COMPILE_ERROR: &str = "✗ ERREUR DE COMPILATION";
pub const MSG_TIMEOUT: &str = "✗ TIMEOUT";
pub const MSG_TESTS_FAILED: &str = "✗ TESTS ÉCHOUÉS";
pub const MSG_WRONG_OUTPUT: &str = "✗ SORTIE INCORRECTE";

// === Security ===
/// Longueur maximale d'un pattern regex dans les exercices (anti-ReDoS).
pub const MAX_REGEX_PATTERN_LEN: usize = 500;
