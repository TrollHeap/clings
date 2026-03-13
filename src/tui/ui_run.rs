//! TUI interactive mode for running a single exercise (`clings run <id>`).
//!
//! This provides a simplified version of watch mode but focused on one exercise,
//! with file watching, compilation, and visualization support.

use colored::Colorize;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use crate::error::Result;
use crate::models::Exercise;
use crate::watcher::{watch_file_interactive, WatchAction};
use crate::{progress, runner};

/// Run a single exercise in interactive mode with file watching and TUI visualization.
pub fn run_exercise(exercise: &Exercise, mastery_score: Option<f64>) -> Result<()> {
    let conn = progress::open_db()?;
    let source_path = runner::write_starter_code(exercise, mastery_score)?;

    // Display exercise header
    println!();
    println!(
        "  {} [{}] {}",
        "▶".bold().green(),
        exercise.id.yellow(),
        exercise.title.bold().green()
    );
    println!(
        "  {} {}",
        "Difficulté:".dimmed(),
        format_difficulty_stars(exercise.difficulty)
    );
    println!();

    // Display description
    println!("  Description :");
    for line in exercise.description.lines() {
        if line.is_empty() {
            println!();
        } else {
            println!("    {}", line);
        }
    }
    println!();

    // Display instructions
    println!("  Instructions :");
    println!("    • Éditez le fichier : {}", source_path.display());
    println!("    • Appuyez sur [r] après chaque sauvegarde pour compiler et vérifier");
    println!("    • Appuyez sur [h] pour voir un indice");
    println!("    • Appuyez sur [v] pour voir le visualiser mémoire (si disponible)");
    println!("    • Appuyez sur [q] pour quitter");
    println!();
    println!("  Keybinds :");
    println!("    [r] Compiler et vérifier        [h] Indice");
    if !exercise.visualizer.steps.is_empty() {
        println!("    [v] Visualiser la mémoire");
    }
    println!("    [q] Quitter");
    println!();

    let exercise_clone = exercise.clone();
    let source_for_change = source_path.clone();
    let mut vis_active = false;
    let mut vis_step: usize = 0;

    let action = watch_file_interactive(
        &source_path,
        || {
            let result = runner::compile_and_run(&source_for_change, &exercise_clone);

            // Display compilation result
            println!();
            if result.compile_error {
                println!("  {} Erreurs de compilation :", "✗".bold().red());
                println!();
                for line in result.stderr.lines() {
                    println!("    {}", line.red());
                }
            } else if result.success {
                println!("  {} Exercice résolu !", "✓".bold().green());
                if let Err(e) = progress::record_attempt(
                    &conn,
                    &exercise_clone.subject,
                    &exercise_clone.id,
                    true,
                ) {
                    eprintln!("[clings] erreur enregistrement tentative: {e}");
                }
            } else {
                println!("  {} Sortie incorrecte.", "✗".bold().red());
                println!();
                println!("  Sortie obtenue :");
                for line in result.stdout.lines() {
                    println!("    {}", line.red());
                }
                if let Err(e) = progress::record_attempt(
                    &conn,
                    &exercise_clone.subject,
                    &exercise_clone.id,
                    false,
                ) {
                    eprintln!("[clings] erreur enregistrement tentative: {e}");
                }
            }

            if result.success {
                return WatchAction::Advance;
            }

            println!();
            println!("  {}", "En attente de la prochaine sauvegarde...".dimmed());
            WatchAction::Continue
        },
        |key_event| {
            if key_event.kind != KeyEventKind::Press {
                return None;
            }

            if vis_active {
                match key_event.code {
                    KeyCode::Right => {
                        vis_step = (vis_step + 1).min(exercise_clone.visualizer.steps.len() - 1);
                        None
                    }
                    KeyCode::Left => {
                        vis_step = vis_step.saturating_sub(1);
                        None
                    }
                    _ => {
                        vis_active = false;
                        None
                    }
                }
            } else {
                match key_event.code {
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        println!();
                        println!("  {} — {}", "Indices".bold().cyan(), exercise_clone.title);
                        println!();
                        for (i, hint) in exercise_clone.hints.iter().enumerate() {
                            println!(
                                "  {} Indice {} :\n",
                                (i + 1).to_string().bold(),
                                (i + 1).to_string().bold()
                            );
                            for line in hint.lines() {
                                println!("      {}", line.dimmed());
                            }
                            println!();
                        }
                        None
                    }
                    KeyCode::Char('v') | KeyCode::Char('V')
                        if !exercise_clone.visualizer.steps.is_empty() =>
                    {
                        vis_step = 0;
                        vis_active = true;
                        None
                    }
                    KeyCode::Char('q') | KeyCode::Char('Q') => Some(WatchAction::Quit),
                    KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(WatchAction::Quit)
                    }
                    KeyCode::Char('z') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(WatchAction::Quit)
                    }
                    _ => None,
                }
            }
        },
    )?;

    if matches!(action, WatchAction::Advance) {
        println!();
        println!("  {} Terminé !", "✓".bold().green());
        println!();
    }

    Ok(())
}

fn format_difficulty_stars(diff: crate::models::Difficulty) -> String {
    use crate::models::Difficulty;
    match diff {
        Difficulty::Easy => "★☆☆☆☆ Easy".green().to_string(),
        Difficulty::Medium => "★★☆☆☆ Medium".yellow().to_string(),
        Difficulty::Hard => "★★★☆☆ Hard".red().to_string(),
        Difficulty::Advanced => "★★★★☆ Advanced".magenta().to_string(),
        Difficulty::Expert => "★★★★★ Expert".cyan().to_string(),
    }
}
