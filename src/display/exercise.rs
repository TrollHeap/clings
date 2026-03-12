//! TUI display for a single exercise — title, description, hints, and run results.

use colored::Colorize;

use crate::constants::TEXT_WRAP_WIDTH;
use crate::models::{Exercise, ValidationMode};
use crate::runner::RunResult;

/// Per-exercise SRS and scaffolding hints for the watch mode header.
#[allow(dead_code)]
pub struct WatchMeta {
    pub stage: Option<u8>,
    pub next_review_days: Option<i64>,
    pub unmet_prereqs: Vec<String>,
}

use super::{difficulty_stars, gcc_re, hr, show_banner, wrap_text};

/// Render the description, key_concept, common_mistake, and validation warning
/// for an exercise — shared between watch mode and single-run mode.
fn render_exercise_body(exercise: &Exercise) {
    for line in exercise.description.lines() {
        if line.chars().count() > TEXT_WRAP_WIDTH {
            for wrapped in wrap_text(line, TEXT_WRAP_WIDTH) {
                println!("  {wrapped}");
            }
        } else {
            println!("  {line}");
        }
    }
    println!();

    if let Some(kc) = &exercise.key_concept {
        println!("  {} {}", "💡 Concept clé :".bold().cyan(), kc);
    }
    if let Some(cm) = &exercise.common_mistake {
        println!("  {} {}", "⚠  Piège:".bold().yellow(), cm);
    }
    if !exercise.files.is_empty() {
        let names: Vec<&str> = exercise.files.iter().map(|f| f.name.as_str()).collect();
        println!(
            "  {} {}",
            "📎 Fichiers fournis :".bold().cyan(),
            names.join(", ").dimmed()
        );
    }

    println!();
}

/// Display exercise info in watch mode (rustlings-style).
#[allow(dead_code)]
pub fn show_exercise_watch(
    exercise: &Exercise,
    index: usize,
    total: usize,
    completed: &[bool],
    chapter_ctx: Option<&crate::chapters::ChapterContext>,
    meta: &WatchMeta,
) {
    super::clear_screen();
    show_banner();
    super::show_progress_bar(index, total, completed);

    if let Some(ctx) = chapter_ctx {
        super::show_chapter(ctx);
        println!();
    }

    println!(
        "  {} [{}/{}]  {}",
        "Exercice".bold().green(),
        (index + 1).to_string().bold(),
        total,
        exercise.title.bold(),
    );

    let review_info = match meta.next_review_days {
        Some(d) if d <= 0 => format!("  {}  {}", "│".dimmed(), "dû".yellow()),
        Some(d) => format!("  {}  {}", "│".dimmed(), format!("dans {} j", d).dimmed()),
        None => String::new(),
    };
    if exercise.starter_code_stages.is_empty() {
        println!(
            "  {}  {}   {}  {}   {}  {}{}",
            "│".dimmed(),
            difficulty_stars(exercise.difficulty),
            "│".dimmed(),
            exercise.exercise_type.to_string().dimmed(),
            "│".dimmed(),
            exercise.subject.dimmed(),
            review_info,
        );
    } else {
        let stage_label = match meta.stage {
            Some(0) => "S0 Exemple",
            Some(1) => "S1 Guide",
            Some(2) => "S2 Blancs",
            Some(3) => "S3 Squelette",
            Some(4) => "S4 Autonome",
            _ => "S2 Blancs",
        };
        println!(
            "  {}  {}   {}  {}   {}  {}   {}  {}{}",
            "│".dimmed(),
            difficulty_stars(exercise.difficulty),
            "│".dimmed(),
            exercise.exercise_type.to_string().dimmed(),
            "│".dimmed(),
            exercise.subject.dimmed(),
            "│".dimmed(),
            stage_label.dimmed(),
            review_info,
        );
    }

    println!("  {}", hr().dimmed());

    if !meta.unmet_prereqs.is_empty() {
        println!();
        println!(
            "  {} {}",
            "⚠ Prérequis non satisfaits :".bold().red(),
            meta.unmet_prereqs.join(", ").red()
        );
    }

    println!();
    render_exercise_body(exercise);
}

/// Display exercise info before editing (single run mode).
pub fn show_exercise(exercise: &Exercise, index: usize, total: usize) {
    println!();
    show_banner();

    println!(
        "  {} [{}/{}]  {}",
        "Exercice".bold().green(),
        index + 1,
        total,
        exercise.title.bold()
    );
    println!(
        "  {}  {}   {}  {}",
        "│".dimmed(),
        difficulty_stars(exercise.difficulty),
        "│".dimmed(),
        exercise.subject.dimmed(),
    );
    println!("  {}", hr().dimmed());
    println!();

    render_exercise_body(exercise);
}

/// Show the "waiting for changes" status.
pub fn show_watching(source_path: &std::path::Path) {
    println!(
        "  {} {}",
        "✎ Édition :".bold().green(),
        source_path.display().to_string().bold()
    );
    println!(
        "  {}",
        "Sauvegardez, puis [r] pour compiler & valider...".dimmed()
    );
    println!();
}

/// Show notification when a file save is detected (no auto-compile).
pub fn show_file_saved() {
    println!("  {}", "fichier sauvegardé — [r] pour compiler".dimmed());
}

/// Show editing instructions.
pub fn show_edit_instructions(source_path: &std::path::Path) {
    println!(
        "  {} {}",
        "✎ Édition :".bold().green(),
        source_path.display().to_string().bold()
    );
    println!("  {}", "Sauvegardez pour compiler & valider...".dimmed());
    println!("  {}", "Ctrl+C pour quitter".dimmed());
    println!();
}

/// Show compilation/run result.
pub fn show_result(result: &RunResult, exercise: &Exercise) {
    println!();
    if result.compile_error {
        println!("  {} {}", "╔══".red(), "ERREUR DE COMPILATION".bold().red());
        for line in result.stderr.lines() {
            let formatted = {
                let re = gcc_re();
                if let Some(caps) = re.captures(line) {
                    let lineno = &caps[1];
                    let sev = &caps[2];
                    let msg = &caps[3];
                    match sev {
                        "error" => format!(
                            "  {}  {} {} │ {}",
                            "║".red(),
                            format!("ligne {lineno}").red().bold(),
                            "error".red(),
                            msg.red()
                        ),
                        "warning" => format!(
                            "  {}  {} {} │ {}",
                            "║".red(),
                            format!("ligne {lineno}").yellow().bold(),
                            "warning".yellow(),
                            msg.yellow()
                        ),
                        _ => format!(
                            "  {}  {} {} │ {}",
                            "║".red(),
                            format!("ligne {lineno}").cyan().bold(),
                            "note".cyan(),
                            msg.cyan()
                        ),
                    }
                } else {
                    format!("  {}  {}", "║".red(), line.dimmed())
                }
            };
            println!("{formatted}");
        }
        println!("  {}", "╚══".red());
    } else if result.timeout {
        println!(
            "  {} {}",
            "╔══".red(),
            "TIMEOUT — dépassement de 10s".bold().red()
        );
        println!("  {}", "╚══".red());
    } else if result.success {
        println!(
            "  {} {} {}",
            "╔══".green(),
            "SUCCÈS".bold().green(),
            format!("({}ms)", result.duration_ms).dimmed()
        );
        if !result.stdout.is_empty() {
            for line in result.stdout.lines() {
                println!("  {} {}", "║".green(), line);
            }
        }
        println!("  {}", "╚══".green());
    } else {
        let is_test_mode = matches!(
            exercise.validation.mode,
            ValidationMode::Test | ValidationMode::Both
        );

        if is_test_mode {
            println!("  {} {}", "╔══".red(), "TESTS ÉCHOUÉS".bold().red());
            if !result.stdout.is_empty() {
                for line in result.stdout.lines() {
                    let colored_line = if line.trim_start().starts_with("OK") {
                        format!("  {}   {}", "║".red(), line.green())
                    } else if line.trim_start().starts_with("FAIL") {
                        format!("  {}   {}", "║".red(), line.red())
                    } else {
                        format!("  {}   {}", "║".red(), line.dimmed())
                    };
                    println!("{colored_line}");
                }
            }
        } else {
            println!("  {} {}", "╔══".red(), "SORTIE INCORRECTE".bold().red());

            if let Some(expected) = &exercise.validation.expected_output {
                println!("  {} {}", "║".red(), "Diff (- attendu  + obtenu):".bold());
                let exp_lines: Vec<&str> = expected.trim().lines().collect();
                let got_lines: Vec<&str> = result.stdout.trim().lines().collect();
                let max_len = exp_lines.len().max(got_lines.len());
                for i in 0..max_len {
                    match (exp_lines.get(i), got_lines.get(i)) {
                        (Some(e), Some(g)) if *e == *g => {
                            println!("  {}   {}", "║".red(), format!("  {e}").green());
                        }
                        (Some(e), Some(g)) => {
                            println!("  {}   {}", "║".red(), format!("- {e}").red());
                            println!("  {}   {}", "║".red(), format!("+ {g}").yellow());
                        }
                        (Some(e), None) => {
                            println!("  {}   {}", "║".red(), format!("- {e}").red());
                        }
                        (None, Some(g)) => {
                            println!("  {}   {}", "║".red(), format!("+ {g}").yellow());
                        }
                        (None, None) => {}
                    }
                }
            }
        }

        if !result.stderr.is_empty() {
            println!("  {} {}", "║".red(), "Stderr:".dimmed());
            for line in result.stderr.lines() {
                println!("  {}   {}", "║".red(), line.yellow());
            }
        }
        println!("  {}", "╚══".red());
    }
    println!();
}

/// Show hints for an exercise.
pub fn show_hints(exercise: &Exercise) {
    if exercise.hints.is_empty() && exercise.common_mistake.is_none() {
        println!("{}", "  Aucun indice disponible.".dimmed());
        return;
    }
    println!(
        "  {} {}",
        "💡 Indices pour".bold().cyan(),
        exercise.title.bold()
    );
    println!("  {}", hr().dimmed());
    for (i, hint) in exercise.hints.iter().enumerate() {
        println!("  {}. {hint}", i + 1);
    }
    if let Some(mistake) = &exercise.common_mistake {
        println!();
        println!(
            "  {} {}",
            "⚠ Erreur fréquente :".bold().yellow(),
            mistake.dimmed()
        );
    }
    println!();
}

/// Show solution for an exercise.
pub fn show_solution(exercise: &Exercise) {
    println!(
        "  {} {}",
        "Solution pour".bold().cyan(),
        exercise.title.bold()
    );
    println!("  {}", hr().dimmed());
    println!("{}", exercise.solution_code);
    println!("  {}", hr().dimmed());
}
