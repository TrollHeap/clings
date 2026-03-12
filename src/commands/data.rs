//! Commandes de gestion des données — export, import, reset, config, new.

use std::io;
use std::io::Write;

use colored::Colorize;

use crate::error::{KfError, Result};
use crate::{authoring, config, exercises, progress};

pub fn cmd_export(output: Option<&std::path::Path>) -> Result<()> {
    let conn = progress::open_db()?;
    let subjects = progress::get_all_subjects(&conn)?;
    let count = subjects.len();
    let json = progress::export_progress(&conn)?;
    match output {
        Some(path) => {
            std::fs::write(path, &json)?;
            println!(
                "  {} {count} sujet(s) exporté(s) vers {}",
                "✓".bold().green(),
                path.display()
            );
        }
        None => {
            print!("{json}");
            println!("  {} {count} sujet(s) affiché(s).", "✓".bold().green());
        }
    }
    Ok(())
}

pub fn cmd_import(input: &std::path::Path, overwrite: bool) -> Result<()> {
    let json = std::fs::read_to_string(input)?;
    let mut conn = progress::open_db()?;
    let (count, warnings) = progress::import_progress(&mut conn, &json, overwrite)?;
    for w in &warnings {
        eprintln!("  {} {}", "⚠".yellow(), w);
    }
    if overwrite {
        println!(
            "  {} {count} sujet(s) importé(s) (mode remplacement).",
            "✓".bold().green()
        );
    } else {
        println!(
            "  {} {count} sujet(s) importé(s) (mode fusion).",
            "✓".bold().green()
        );
    }
    Ok(())
}

pub fn cmd_reset(subject: Option<&str>) -> Result<()> {
    if let Some(name) = subject {
        print!(
            "  {} Supprimer la progression de '{}'. Taper 'yes' pour confirmer : ",
            "Attention !".bold().red(),
            name
        );
        let _ = io::stdout().flush(); // best-effort flush — non-critique
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim() == "yes" {
            let conn = progress::open_db()?;
            progress::reset_subject(&conn, name)?;
            println!("  {} Progression de '{}' réinitialisée.", "✓".green(), name);
        } else {
            println!("  {}", "Annulé.".dimmed());
        }
    } else {
        print!(
            "  {} Ceci supprimera TOUTE la progression. Tapez 'yes' pour confirmer : ",
            "Attention !".bold().red()
        );
        let _ = io::stdout().flush(); // best-effort flush — non-critique
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim() == "yes" {
            let conn = progress::open_db()?;
            progress::reset_progress(&conn)?;
            println!("  {}", "Progression réinitialisée.".green());
        } else {
            println!("  {}", "Annulé.".dimmed());
        }
    }
    Ok(())
}

pub fn cmd_config(key: &str, value: &str) -> Result<()> {
    let (section, field) = key.split_once('.').ok_or_else(|| {
        KfError::Config(format!(
            "Format de clé invalide : '{key}' — attendu 'section.champ' (ex: srs.decay_days)"
        ))
    })?;
    config::set_value(section, field, value).map_err(KfError::Config)?;
    println!(
        "  {} {key} = {value}",
        "Config mise à jour :".bold().green()
    );
    Ok(())
}

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
        dir.join(format!("{}.json", exercise.id))
    };

    let json = serde_json::to_string_pretty(&exercise)
        .map_err(|e| KfError::Config(format!("sérialisation JSON : {e}")))?;
    std::fs::write(&target, &json)?;

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
        if json.contains(placeholder) {
            println!("    {} {}", "•".dimmed(), placeholder.yellow());
        }
    }
    if json.contains("__EXPECTED_OUTPUT__") {
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
