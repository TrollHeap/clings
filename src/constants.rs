//! Shared constants for the clings application.

// === Timing ===
pub const EXECUTION_TIMEOUT_SECS: u64 = 10;
pub const POLL_INTERVAL_MS: u64 = 50;
pub const DEBOUNCE_INTERVAL_MS: u64 = 200;
pub const KEY_CHECK_TIMEOUT_MS: u64 = 50;
pub const DB_BUSY_TIMEOUT_MS: i32 = 5000;
pub const SUCCESS_PAUSE_SECS: u64 = 2;

// === Mastery thresholds (from mastery.rs) ===
pub const MASTERY_MAX: f64 = 5.0;
pub const MASTERY_MIN: f64 = 0.0;
pub const MASTERY_SUCCESS_DELTA: f64 = 1.0;
pub const MASTERY_FAILURE_DELTA: f64 = 0.5;
pub const MASTERY_DECAY_DAYS: i64 = 14;
pub const MASTERY_DECAY_RATE: f64 = 0.1;
pub const SRS_INTERVAL_MULTIPLIER: f64 = 2.5;
pub const SRS_BASE_INTERVAL_DAYS: i64 = 1;
pub const SRS_MAX_INTERVAL_DAYS: i64 = 60;
pub const DIFFICULTY_2_UNLOCK: f64 = 2.0;
pub const DIFFICULTY_3_UNLOCK: f64 = 4.0;
pub const DIFFICULTY_4_UNLOCK: f64 = 4.5;
pub const DIFFICULTY_5_UNLOCK: f64 = 5.0;

// === UI dimensions ===
pub const HEADER_WIDTH: usize = 56;
pub const PROGRESS_BAR_WIDTH: usize = 30;
pub const TEXT_WRAP_WIDTH: usize = 72;

// === Display thresholds ===
pub const MINIMAP_MAX_ITEMS: usize = 60;
pub const CONSECUTIVE_FAILURE_THRESHOLD: usize = 3;

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

// === tmux ===
pub const TMUX_PANE_WIDTH_PERCENT: &str = "50";
pub const TMUX_EDITOR: &str = "nvim";
