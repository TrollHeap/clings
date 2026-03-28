//! Load and persist user configuration to ~/.clings/clings.toml.

use crate::constants;
use crate::error::KfError;

/// Charge la configuration depuis ~/.clings/clings.toml avec fallback aux défauts.
/// Ignore silencieusement les fichiers manquants ou mal formés.
pub fn load() -> super::ClingConfig {
    let path = constants::clings_data_dir().join(constants::CONFIG_TOML_FILENAME);

    if !path.exists() {
        return super::ClingConfig::default();
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str::<super::ClingConfig>(&content).unwrap_or_else(|e| {
            eprintln!("  [clings] erreur config TOML: {e}");
            super::ClingConfig::default()
        }),
        Err(e) => {
            eprintln!("  [clings] impossible de lire clings.toml: {e}");
            super::ClingConfig::default()
        }
    }
}

/// Écrit une seule clé `section.key = value` dans ~/.clings/clings.toml.
/// Crée le fichier s'il n'existe pas.
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
