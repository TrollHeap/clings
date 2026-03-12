//! User configuration loaded from `~/.clings/clings.toml`.
//!
//! At startup, `init()` is called once. All code reads config via `get()`.
//! Missing fields fall back to the compile-time constants in `constants.rs`.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::constants;

static CONFIG: OnceLock<ClingConfig> = OnceLock::new();

// ── Top-level struct ──────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ClingConfig {
    pub srs: SrsConfig,
    pub ui: UiConfig,
    pub tmux: TmuxConfig,
}

// ── [srs] section ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct SrsConfig {
    pub decay_days: i64,
    pub base_interval_days: i64,
    pub max_interval_days: i64,
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct UiConfig {
    pub editor: String,
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct TmuxConfig {
    pub enabled: bool,
}

impl Default for TmuxConfig {
    fn default() -> Self {
        TmuxConfig { enabled: true }
    }
}

// ── Load / access ─────────────────────────────────────────────────────────────

/// Load config from `~/.clings/clings.toml`, falling back to defaults.
/// Silently ignores missing or malformed files.
pub fn load() -> ClingConfig {
    let path = match std::env::var_os("HOME") {
        Some(h) => std::path::PathBuf::from(h)
            .join(constants::CLINGS_DIR)
            .join("clings.toml"),
        None => return ClingConfig::default(),
    };

    if !path.exists() {
        return ClingConfig::default();
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str::<ClingConfig>(&content).unwrap_or_default(),
        Err(_) => ClingConfig::default(),
    }
}

/// Initialize the global config. Must be called once at startup.
pub fn init(cfg: ClingConfig) {
    CONFIG.set(cfg).ok();
}

/// Access the global config. Returns default if `init()` was not called.
pub fn get() -> &'static ClingConfig {
    CONFIG.get_or_init(ClingConfig::default)
}

/// Write a single `section.key = value` into `~/.clings/clings.toml`.
/// Creates the file if it does not exist.
pub fn set_value(section: &str, key: &str, value: &str) -> Result<(), String> {
    const ALLOWED: &[(&str, &str)] = &[
        ("srs", "decay_days"),
        ("srs", "base_interval_days"),
        ("ui", "editor"),
        ("tmux", "enabled"),
        ("ui", "tmux_pane_width"),
    ];
    if !ALLOWED.iter().any(|(s, k)| *s == section && *k == key) {
        return Err(format!(
            "clé inconnue '{section}.{key}' — valeurs autorisées : {}",
            ALLOWED
                .iter()
                .map(|(s, k)| format!("{s}.{k}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let path = {
        let home = std::env::var_os("HOME").ok_or_else(|| "$HOME non définie".to_string())?;
        std::path::PathBuf::from(home)
            .join(constants::CLINGS_DIR)
            .join("clings.toml")
    };

    // Load current TOML as a Value so we preserve unknown fields
    let mut doc: toml::Value = if path.exists() {
        std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| toml::from_str(&s).map_err(|e| e.to_string()))
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
        .ok_or_else(|| "format TOML invalide".to_string())?;

    let section_table = table
        .entry(section)
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| format!("section '{section}' n'est pas une table"))?;

    section_table.insert(key.to_string(), parsed);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let serialized = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, serialized).map_err(|e| e.to_string())?;

    Ok(())
}
