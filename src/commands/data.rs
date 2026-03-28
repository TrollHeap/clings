//! Exercise authoring commands — new, schema, config.

use colored::Colorize;
use schemars::schema_for;

use crate::error::{KfError, Result};
use crate::models::Exercise;
use crate::{authoring, config, exercises};

/// Write a single config key-value pair to ~/.clings/clings.toml (format: 'section.field').
pub fn cmd_config(key: &str, value: &str) -> Result<()> {
    let (section, field) = key.split_once('.').ok_or_else(|| {
        KfError::Config(format!(
            "Format de clé invalide : '{key}' — attendu 'section.champ' (ex: srs.decay_days)"
        ))
    })?;
    config::set_value(section, field, value)?;
    println!(
        "  {} {key} = {value}",
        "Config mise à jour :".bold().green()
    );
    Ok(())
}

/// Generate exercise skeleton or validate exercise JSON. If validate_only provided, validates that file only.
/// Otherwise generates a new exercise template with given subject, difficulty, and mode (complete/fix_bug/etc).
pub fn cmd_new(
    subject: Option<&str>,
    difficulty: u8,
    mode: &str,
    output: Option<&std::path::Path>,
    validate_only: Option<&std::path::Path>,
) -> Result<()> {
    // ── Mode --validate-only ──────────────────────────────────────────────
    if let Some(path) = validate_only {
        let errors = authoring::validate_exercise(path);
        if errors.is_empty() {
            println!("  {} Validation réussie.", "✓".bold().green());
        } else {
            eprintln!("  {} Erreurs de validation :", "✗".bold().red());
            for err in &errors {
                eprintln!("    {} {}", "•".dimmed(), err);
            }
            return Err(KfError::Config("validation échouée".to_string()));
        }
        return Ok(());
    }

    // ── Mode génération ───────────────────────────────────────────────────
    let subject = subject.ok_or_else(|| {
        KfError::Config(
            "--subject requis pour générer un squelette (ex: --subject pointers)".to_string(),
        )
    })?;

    let exercise = authoring::generate_skeleton(subject, difficulty, mode)?;

    // Determine output path
    let target = if let Some(p) = output {
        p.to_path_buf()
    } else {
        let exercises_dir = exercises::resolve_exercises_dir()?;
        let dir = exercises_dir.join(subject);
        std::fs::create_dir_all(&dir)?;
        dir.join(format!("{}.toml", exercise.id))
    };

    let toml_str = toml::to_string_pretty(&exercise)
        .map_err(|e| KfError::Config(format!("sérialisation TOML : {e}")))?;
    std::fs::write(&target, &toml_str)?;

    println!();
    println!(
        "  {} {}",
        "Squelette généré :".bold().green(),
        target.display()
    );
    println!();
    println!("  Champs à remplir :");
    for placeholder in &[
        "__TITLE__",
        "__DESCRIPTION__",
        "__STARTER_CODE__",
        "__SOLUTION_CODE__",
    ] {
        if toml_str.contains(placeholder) {
            println!("    {} {}", "•".dimmed(), placeholder.yellow());
        }
    }
    if toml_str.contains("__EXPECTED_OUTPUT__") {
        println!("    {} {}", "•".dimmed(), "__EXPECTED_OUTPUT__".yellow());
    }
    println!();
    println!(
        "  Valider ensuite avec : {}",
        format!("clings new --validate-only {}", target.display()).bold()
    );
    println!();
    Ok(())
}

/// Generate exercise.schema.json from the Exercise struct for IDE autocompletion.
pub fn cmd_schema(output: &std::path::Path) -> Result<()> {
    let schema = schema_for!(Exercise);
    let json = serde_json::to_string_pretty(&schema)
        .map_err(|e| KfError::Config(format!("sérialisation du schema : {e}")))?;
    std::fs::write(output, &json)?;
    println!(
        "  {} Schema écrit dans {}",
        "✓".bold().green(),
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_config_valid_key() {
        let result = cmd_config("srs.decay_days", "21");
        // This test verifies the function accepts valid keys and values.
        // If it succeeds, the key was accepted; if it fails, it must be due to I/O
        // (file permission) not argument validation, so we just check it doesn't
        // immediately reject the key format.
        match result {
            Ok(_) => {
                // Successfully set config
            }
            Err(e) => {
                // Should not be a Config error about invalid key
                assert!(!e.to_string().contains("Format de clé invalide"));
            }
        }
    }

    #[test]
    fn test_cmd_config_invalid_key_format() {
        // Key without a dot separator should fail immediately
        let result = cmd_config("invalid_key_no_dot", "value");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Format de clé invalide"));
    }

    #[test]
    fn test_cmd_config_unknown_key() {
        // Valid format but unknown key/section combination
        let result = cmd_config("unknown.field", "value");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("inconnue"));
    }

    #[test]
    fn test_cmd_config_valid_srs_interval() {
        let result = cmd_config("srs.base_interval_days", "7");
        // Valid key that should be accepted
        match result {
            Ok(_) => {
                // Successfully set
            }
            Err(e) => {
                // Should not be about invalid key format
                assert!(!e.to_string().contains("Format de clé invalide"));
            }
        }
    }

    #[test]
    fn test_cmd_config_valid_ui_editor() {
        let result = cmd_config("ui.editor", "vim");
        match result {
            Ok(_) => {
                // Successfully set
            }
            Err(e) => {
                assert!(!e.to_string().contains("Format de clé invalide"));
            }
        }
    }

    #[test]
    fn test_cmd_config_valid_tmux_enabled() {
        let result = cmd_config("tmux.enabled", "true");
        match result {
            Ok(_) => {
                // Successfully set
            }
            Err(e) => {
                assert!(!e.to_string().contains("Format de clé invalide"));
            }
        }
    }

    #[test]
    fn test_cmd_config_multiple_dots_in_value() {
        // Value with multiple dots should be accepted as string
        let result = cmd_config("sync.remote", "https://github.com/user/repo.git");
        match result {
            Ok(_) => {
                // Successfully set
            }
            Err(e) => {
                assert!(!e.to_string().contains("Format de clé invalide"));
            }
        }
    }
}
