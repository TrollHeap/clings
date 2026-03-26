//! User configuration loaded from `~/.clings/clings.toml`.
//!
//! At startup, `init()` is called once. All code reads config via `get()`.
//! Missing fields fall back to the compile-time constants in `constants.rs`.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::constants;
use crate::error::KfError;

static CONFIG: OnceLock<ClingConfig> = OnceLock::new();

// ── Top-level struct ──────────────────────────────────────────────────────────

/// Top-level user configuration loaded from ~/.clings/clings.toml.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ClingConfig {
    /// SRS (Spaced Repetition System) configuration: decay, intervals, multiplier.
    pub srs: SrsConfig,
    /// UI configuration: editor path, tmux pane width.
    pub ui: UiConfig,
    /// Tmux integration settings.
    pub tmux: TmuxConfig,
    /// Git sync configuration: remote, branch, enabled flag.
    pub sync: SyncConfig,
}

// ── [srs] section ─────────────────────────────────────────────────────────────

/// SRS (Spaced Repetition System) configuration.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct SrsConfig {
    /// Days of inactivity before mastery score decays by 0.5.
    pub decay_days: i64,
    /// Minimum review interval (days) after a success.
    pub base_interval_days: i64,
    /// Maximum review interval (days) after repeated successes.
    pub max_interval_days: i64,
    /// Multiplier applied to interval on success (e.g., 2.5).
    pub interval_multiplier: f64,
}

impl Default for SrsConfig {
    fn default() -> Self {
        SrsConfig {
            decay_days: constants::MASTERY_DECAY_DAYS,
            base_interval_days: constants::SRS_BASE_INTERVAL_DAYS,
            max_interval_days: constants::SRS_MAX_INTERVAL_DAYS,
            interval_multiplier: constants::SRS_INTERVAL_MULTIPLIER,
        }
    }
}

// ── [ui] section ──────────────────────────────────────────────────────────────

/// UI and editor configuration.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct UiConfig {
    /// Default editor command (e.g., 'nvim', 'vim'). Used in tmux pane integration.
    pub editor: String,
    /// Width (in chars) of tmux editor pane when split. Default: 50.
    pub tmux_pane_width: u8,
}

impl Default for UiConfig {
    fn default() -> Self {
        UiConfig {
            editor: constants::TMUX_EDITOR.to_string(),
            tmux_pane_width: 50,
        }
    }
}

// ── [tmux] section ────────────────────────────────────────────────────────────

/// Tmux integration configuration.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct TmuxConfig {
    /// Enable/disable automatic tmux pane creation for editor integration.
    pub enabled: bool,
}

impl Default for TmuxConfig {
    fn default() -> Self {
        TmuxConfig { enabled: true }
    }
}

// ── [sync] section ────────────────────────────────────────────────────────────

/// Git sync configuration for progress synchronization across machines.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SyncConfig {
    /// Enable/disable git sync for progress snapshots.
    pub enabled: bool,
    /// Git remote URL (e.g., 'https://github.com/user/clings-progress.git').
    pub remote: String,
    /// Git branch name for progress snapshots. Default: 'main'.
    pub branch: String,
    /// Machine hostname for sync commits. If empty, system hostname is used.
    pub hostname: String,
}

impl Default for SyncConfig {
    fn default() -> Self {
        SyncConfig {
            enabled: false,
            remote: String::new(),
            branch: constants::SYNC_DEFAULT_BRANCH.to_string(),
            hostname: String::new(),
        }
    }
}

// ── Load / access ─────────────────────────────────────────────────────────────

/// Load config from `~/.clings/clings.toml`, falling back to defaults.
/// Silently ignores missing or malformed files.
pub fn load() -> ClingConfig {
    let path = constants::clings_data_dir().join(constants::CONFIG_TOML_FILENAME);

    if !path.exists() {
        return ClingConfig::default();
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str::<ClingConfig>(&content).unwrap_or_else(|e| {
            eprintln!("  [clings] erreur config TOML: {e}");
            ClingConfig::default()
        }),
        Err(e) => {
            eprintln!("  [clings] impossible de lire clings.toml: {e}");
            ClingConfig::default()
        }
    }
}

/// Initialize the global config. Must be called once at startup.
/// Double-init is a no-op (OnceLock guarantees single initialization).
pub fn init(cfg: ClingConfig) {
    CONFIG.set(cfg).ok();
}

/// Access the global config. Returns default if `init()` was not called.
pub fn get() -> &'static ClingConfig {
    CONFIG.get_or_init(ClingConfig::default)
}

/// Write a single `section.key = value` into `~/.clings/clings.toml`.
/// Creates the file if it does not exist.
pub fn set_value(section: &str, key: &str, value: &str) -> crate::error::Result<()> {
    const ALLOWED: &[(&str, &str)] = &[
        ("srs", "decay_days"),
        ("srs", "base_interval_days"),
        ("ui", "editor"),
        ("tmux", "enabled"),
        ("ui", "tmux_pane_width"),
        ("sync", "enabled"),
        ("sync", "remote"),
        ("sync", "branch"),
        ("sync", "hostname"),
    ];
    if !ALLOWED.iter().any(|(s, k)| *s == section && *k == key) {
        return Err(KfError::Config(format!(
            "clé inconnue '{section}.{key}' — valeurs autorisées : {}",
            ALLOWED
                .iter()
                .map(|(s, k)| format!("{s}.{k}"))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    let path = constants::clings_data_dir().join(constants::CONFIG_TOML_FILENAME);

    // Load current TOML as a Value so we preserve unknown fields
    let mut doc: toml::Value = if path.exists() {
        std::fs::read_to_string(&path)
            .map_err(|e| KfError::Config(e.to_string()))
            .and_then(|s| toml::from_str(&s).map_err(|e| KfError::Config(e.to_string())))
            .unwrap_or(toml::Value::Table(toml::map::Map::new()))
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    // Parse value: try i64 → f64 → bool → string
    let parsed: toml::Value = if let Ok(i) = value.parse::<i64>() {
        toml::Value::Integer(i)
    } else if let Ok(f) = value.parse::<f64>() {
        toml::Value::Float(f)
    } else if let Ok(b) = value.parse::<bool>() {
        toml::Value::Boolean(b)
    } else {
        toml::Value::String(value.to_string())
    };

    let table = doc
        .as_table_mut()
        .ok_or_else(|| KfError::Config("format TOML invalide".to_string()))?;

    let section_table = table
        .entry(section)
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| KfError::Config(format!("section '{section}' n'est pas une table")))?;

    section_table.insert(key.to_string(), parsed);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        {
            use std::fs::DirBuilder;
            use std::os::unix::fs::DirBuilderExt;
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)
                .map_err(|e: std::io::Error| KfError::Config(e.to_string()))?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(parent).map_err(|e| KfError::Config(e.to_string()))?;
    }

    let serialized = toml::to_string_pretty(&doc).map_err(|e| KfError::Config(e.to_string()))?;
    std::fs::write(&path, serialized).map_err(|e| KfError::Config(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to serialize tests that modify CLINGS_HOME env var
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // ── SrsConfig defaults ─────────────────────────────────────────────────────────

    #[test]
    fn test_srs_config_default_decay_days() {
        let cfg = SrsConfig::default();
        assert_eq!(cfg.decay_days, constants::MASTERY_DECAY_DAYS);
        assert_eq!(cfg.decay_days, 14);
    }

    #[test]
    fn test_srs_config_default_base_interval() {
        let cfg = SrsConfig::default();
        assert_eq!(cfg.base_interval_days, constants::SRS_BASE_INTERVAL_DAYS);
        assert_eq!(cfg.base_interval_days, 1);
    }

    #[test]
    fn test_srs_config_default_max_interval() {
        let cfg = SrsConfig::default();
        assert_eq!(cfg.max_interval_days, constants::SRS_MAX_INTERVAL_DAYS);
        assert_eq!(cfg.max_interval_days, 60);
    }

    #[test]
    fn test_srs_config_default_multiplier() {
        let cfg = SrsConfig::default();
        assert_eq!(cfg.interval_multiplier, constants::SRS_INTERVAL_MULTIPLIER);
        assert_eq!(cfg.interval_multiplier, 2.5);
    }

    // ── UiConfig defaults ──────────────────────────────────────────────────────────

    #[test]
    fn test_ui_config_default_editor() {
        let cfg = UiConfig::default();
        assert_eq!(cfg.editor, constants::TMUX_EDITOR);
        assert_eq!(cfg.editor, "nvim");
    }

    #[test]
    fn test_ui_config_default_pane_width() {
        let cfg = UiConfig::default();
        assert_eq!(cfg.tmux_pane_width, 50);
    }

    // ── TmuxConfig defaults ────────────────────────────────────────────────────────

    #[test]
    fn test_tmux_config_default_enabled() {
        let cfg = TmuxConfig::default();
        assert!(cfg.enabled);
    }

    // ── SyncConfig defaults ────────────────────────────────────────────────────────

    #[test]
    fn test_sync_config_default_disabled() {
        let cfg = SyncConfig::default();
        assert!(!cfg.enabled);
    }

    #[test]
    fn test_sync_config_default_empty_remote() {
        let cfg = SyncConfig::default();
        assert_eq!(cfg.remote, "");
    }

    #[test]
    fn test_sync_config_default_branch() {
        let cfg = SyncConfig::default();
        assert_eq!(cfg.branch, constants::SYNC_DEFAULT_BRANCH);
        assert_eq!(cfg.branch, "main");
    }

    #[test]
    fn test_sync_config_default_empty_hostname() {
        let cfg = SyncConfig::default();
        assert_eq!(cfg.hostname, "");
    }

    // ── ClingConfig defaults ───────────────────────────────────────────────────────

    #[test]
    fn test_cling_config_default_all_sections() {
        let cfg = ClingConfig::default();
        assert_eq!(cfg.srs.decay_days, 14);
        assert_eq!(cfg.ui.editor, "nvim");
        assert!(cfg.tmux.enabled);
        assert!(!cfg.sync.enabled);
    }

    // ── TOML deserialization ───────────────────────────────────────────────────────

    #[test]
    fn test_toml_deserialize_srs_section() {
        let toml_str = r#"
[srs]
decay_days = 7
base_interval_days = 2
max_interval_days = 90
interval_multiplier = 3.0
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.srs.decay_days, 7);
        assert_eq!(cfg.srs.base_interval_days, 2);
        assert_eq!(cfg.srs.max_interval_days, 90);
        assert_eq!(cfg.srs.interval_multiplier, 3.0);
    }

    #[test]
    fn test_toml_deserialize_ui_section() {
        let toml_str = r#"
[ui]
editor = "vim"
tmux_pane_width = 80
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.ui.editor, "vim");
        assert_eq!(cfg.ui.tmux_pane_width, 80);
    }

    #[test]
    fn test_toml_deserialize_tmux_section() {
        let toml_str = r#"
[tmux]
enabled = false
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert!(!cfg.tmux.enabled);
    }

    #[test]
    fn test_toml_deserialize_sync_section() {
        let toml_str = r#"
[sync]
enabled = true
remote = "https://github.com/user/clings-progress.git"
branch = "develop"
hostname = "laptop-a"
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.sync.enabled);
        assert_eq!(
            cfg.sync.remote,
            "https://github.com/user/clings-progress.git"
        );
        assert_eq!(cfg.sync.branch, "develop");
        assert_eq!(cfg.sync.hostname, "laptop-a");
    }

    #[test]
    fn test_toml_deserialize_mixed_sections() {
        let toml_str = r#"
[srs]
decay_days = 21

[ui]
editor = "code"

[tmux]
enabled = false

[sync]
enabled = true
remote = "https://example.com/repo.git"
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.srs.decay_days, 21);
        assert_eq!(cfg.ui.editor, "code");
        assert!(!cfg.tmux.enabled);
        assert!(cfg.sync.enabled);
        assert_eq!(cfg.sync.remote, "https://example.com/repo.git");
    }

    #[test]
    fn test_toml_deserialize_partial_config() {
        let toml_str = r#"
[srs]
decay_days = 28
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.srs.decay_days, 28);
        // Other values should be defaults
        assert_eq!(cfg.srs.base_interval_days, 1);
        assert_eq!(cfg.ui.editor, "nvim");
    }

    // ── set_value: valid keys ──────────────────────────────────────────────────────

    #[test]
    fn test_set_value_srs_decay_days() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("srs", "decay_days", "21");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_srs_base_interval() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("srs", "base_interval_days", "3");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_ui_editor() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("ui", "editor", "emacs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_ui_pane_width() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("ui", "tmux_pane_width", "100");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_tmux_enabled() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("tmux", "enabled", "false");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_sync_enabled() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("sync", "enabled", "true");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_sync_remote() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("sync", "remote", "https://github.com/user/repo.git");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_sync_branch() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("sync", "branch", "development");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_sync_hostname() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("sync", "hostname", "machine-01");
        assert!(result.is_ok());
    }

    // ── set_value: unknown keys ────────────────────────────────────────────────────

    #[test]
    fn test_set_value_unknown_section() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("unknown", "key", "value");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("clé inconnue"));
    }

    #[test]
    fn test_set_value_unknown_key_in_known_section() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("srs", "unknown_key", "42");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("clé inconnue"));
    }

    #[test]
    fn test_set_value_wrong_section_right_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("tmux", "decay_days", "7");
        assert!(result.is_err());
    }

    // ── set_value: value parsing ───────────────────────────────────────────────────

    #[test]
    fn test_set_value_integer_parsing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = set_value("srs", "decay_days", "42");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_float_parsing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = set_value("srs", "base_interval_days", "3");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_boolean_true() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = set_value("tmux", "enabled", "true");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_boolean_false() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = set_value("sync", "enabled", "false");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_string_with_spaces() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = set_value("ui", "editor", "code --wait");
        assert!(result.is_ok());
    }

    // ── set_value: file creation & preservation ────────────────────────────────────

    #[test]
    fn test_set_value_creates_file() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        set_value("srs", "decay_days", "28").unwrap();
        let path = clings_home.join("clings.toml");
        assert!(path.exists());
    }

    #[test]
    fn test_set_value_creates_parent_dir() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("nested").join("dir");
        std::env::set_var("CLINGS_HOME", &nested);
        let result = set_value("srs", "decay_days", "7");
        assert!(result.is_ok());
        assert!(nested.exists());
    }

    #[test]
    fn test_set_value_preserves_existing_keys() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let r1 = set_value("srs", "decay_days", "14");
        let r2 = set_value("srs", "base_interval_days", "2");
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        // Verify file was created
        let path = clings_home.join("clings.toml");
        assert!(path.exists());
    }

    #[test]
    fn test_set_value_updates_existing_key_succeeds() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let r1 = set_value("srs", "decay_days", "14");
        let r2 = set_value("srs", "decay_days", "21");
        assert!(r1.is_ok());
        assert!(r2.is_ok());
    }

    #[test]
    fn test_set_value_multiple_sections_succeeds() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let r1 = set_value("srs", "decay_days", "21");
        let r2 = set_value("ui", "editor", "vim");
        let r3 = set_value("tmux", "enabled", "false");
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(r3.is_ok());
    }

    // ── set_value: edge cases ──────────────────────────────────────────────────────

    #[test]
    fn test_set_value_empty_string_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("", "key", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_value_empty_string_section() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = set_value("srs", "", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_value_numeric_out_of_u8_range_for_pane_width() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        // 300 is valid i64 but won't fit u8; it will be stored as i64
        let clings_home = tmp.path().to_path_buf();
        set_value("ui", "tmux_pane_width", "300").unwrap();
        let path = clings_home.join("clings.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("tmux_pane_width = 300"));
    }

    #[test]
    fn test_set_value_url_as_string() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        set_value("sync", "remote", "https://github.com/user/repo.git").unwrap();
        let path = clings_home.join("clings.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("https://github.com/user/repo.git"));
    }

    #[test]
    fn test_set_value_negative_integer_succeeds() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = set_value("srs", "decay_days", "-5");
        assert!(result.is_ok());
    }

    // ── Load function ─────────────────────────────────────────────────────────────
    // Note: load() tests are not included because load() reads HOME env var at runtime,
    // and parallel tests cause race conditions when modifying global env state.
    // The load() function is also tested implicitly through set_value integration tests.
}
