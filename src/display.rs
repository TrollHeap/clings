use colored::Colorize;

use crate::chapters::{ChapterContext, CHAPTERS};
use crate::models::{Exercise, Subject, ValidationMode};
use crate::runner::RunResult;

// ─── Box-drawing helpers ────────────────────────────────────────────

const HEADER_WIDTH: usize = 56;

fn hr() -> String {
    "─".repeat(HEADER_WIDTH)
}

fn header_box(title: &str) -> String {
    let pad = HEADER_WIDTH.saturating_sub(title.len() + 4);
    let left = pad / 2;
    let right = pad - left;
    format!(
        "╔{} {} {}╗",
        "═".repeat(left + 1),
        title,
        "═".repeat(right + 1)
    )
}

fn footer_box() -> String {
    format!("╚{}╝", "═".repeat(HEADER_WIDTH - 2))
}

// ─── Core display functions ─────────────────────────────────────────

/// Clear screen and move cursor to top.
pub fn clear_screen() {
    print!("\x1b[2J\x1b[H");
}

/// Display the main banner.
fn show_banner() {
    println!("  {}", header_box("KERNELFORGE").bold().cyan());
    println!(
        "  {}  {}  {}",
        "║".cyan(),
        "  NSY103 — Programmation système Linux  ".dimmed(),
        "║".cyan()
    );
    println!("  {}", footer_box().cyan());
    println!();
}

/// Display chapter indicator.
pub fn show_chapter(ctx: &ChapterContext) {
    println!(
        "  {} Chapitre {}/{} — {}  [{}/{}]",
        "▸".bold().cyan(),
        ctx.chapter_number,
        ctx.total_chapters,
        ctx.chapter_title.bold(),
        ctx.exercise_in_chapter,
        ctx.chapter_size,
    );
}

/// Display the progress bar with block characters.
pub fn show_progress_bar(current: usize, total: usize, completed: &[bool]) {
    let done = completed.iter().filter(|&&c| c).count();
    let pct = if total > 0 { done * 100 / total } else { 0 };

    // Block progress bar
    let bar_width = 30;
    let filled = if total > 0 {
        (done * bar_width) / total
    } else {
        0
    };
    let partial = if total > 0 {
        ((done * bar_width * 2) / total) % 2
    } else {
        0
    };
    let empty = bar_width - filled - if partial > 0 { 1 } else { 0 };

    let bar = format!(
        "{}{}{}",
        "█".repeat(filled),
        if partial > 0 { "▓" } else { "" },
        "░".repeat(empty)
    );

    let colored_bar = if pct >= 75 {
        bar.green()
    } else if pct >= 25 {
        bar.yellow()
    } else {
        bar.white()
    };

    println!(
        "  {} {} {}/{}  ({}%)",
        "Progress".bold(),
        colored_bar,
        done.to_string().bold(),
        total,
        pct
    );

    // Mini-map with Unicode dots
    if total <= 60 {
        let mut dots = String::with_capacity(total * 3);
        for (i, &d) in completed.iter().enumerate() {
            if i == current {
                dots.push('◉');
            } else if d {
                dots.push('●');
            } else {
                dots.push('○');
            }
        }
        println!("  {}", dots.dimmed());
    }
    println!();
}

/// Display exercise info in watch mode (rustlings-style).
pub fn show_exercise_watch(
    exercise: &Exercise,
    index: usize,
    total: usize,
    completed: &[bool],
    chapter_ctx: Option<&ChapterContext>,
    stage: Option<u8>,
) {
    clear_screen();
    show_banner();
    show_progress_bar(index, total, completed);

    if let Some(ctx) = chapter_ctx {
        show_chapter(ctx);
        println!();
    }

    println!(
        "  {} [{}/{}]  {}",
        "Exercise".bold().green(),
        (index + 1).to_string().bold(),
        total,
        exercise.title.bold(),
    );

    let stage_label = match stage {
        Some(0) => "S0 Exemple",
        Some(1) => "S1 Guide",
        Some(2) => "S2 Blancs",
        Some(3) => "S3 Squelette",
        Some(4) => "S4 Autonome",
        _ => "S2 Blancs",
    };
    println!(
        "  {}  {}   {}  {:?}   {}  {}   {}  {}",
        "│".dimmed(),
        match exercise.difficulty {
            crate::models::Difficulty::Easy => "★☆☆".green(),
            crate::models::Difficulty::Medium => "★★☆".yellow(),
            crate::models::Difficulty::Hard => "★★★".red(),
        },
        "│".dimmed(),
        exercise.exercise_type,
        "│".dimmed(),
        exercise.subject.dimmed(),
        "│".dimmed(),
        stage_label.dimmed(),
    );

    println!("  {}", hr().dimmed());
    println!();

    for line in exercise.description.lines() {
        println!("  {line}");
    }
    println!();

    if let Some(kc) = &exercise.key_concept {
        println!("  {} {}", "💡 Key concept:".bold().cyan(), kc);
    }
    if let Some(cm) = &exercise.common_mistake {
        println!("  {} {}", "⚠  Piège:".bold().yellow(), cm);
    }

    match exercise.validation.mode {
        ValidationMode::Test | ValidationMode::Both => {
            println!(
                "\n  {} Test-based validation (non supporté en CLI)",
                "⚠".yellow()
            );
        }
        _ => {}
    }

    println!();
}

/// Show keybind hints.
pub fn show_keybinds() {
    println!(
        "  {} {} hint  {} skip  {} quit  {} list  {} check",
        "Keys".bold().cyan(),
        "[h]".bold(),
        "[n]".bold(),
        "[q]".bold(),
        "[l]".bold(),
        "[c]".bold(),
    );
    println!();
}

/// Show the "waiting for changes" status.
pub fn show_watching(source_path: &std::path::Path) {
    println!(
        "  {} {}",
        "✎ Editing:".bold().green(),
        source_path.display().to_string().bold()
    );
    println!(
        "  {}",
        "Sauvegardez le fichier pour compiler & valider...".dimmed()
    );
    println!();
}

/// Display exercise info before editing (single run mode).
pub fn show_exercise(exercise: &Exercise, index: usize, total: usize) {
    println!();
    show_banner();

    println!(
        "  {} [{}/{}]  {}",
        "Exercise".bold().green(),
        index + 1,
        total,
        exercise.title.bold()
    );
    println!(
        "  {}  {}   {}  {}",
        "│".dimmed(),
        match exercise.difficulty {
            crate::models::Difficulty::Easy => "★☆☆".green(),
            crate::models::Difficulty::Medium => "★★☆".yellow(),
            crate::models::Difficulty::Hard => "★★★".red(),
        },
        "│".dimmed(),
        exercise.subject.dimmed(),
    );
    println!("  {}", hr().dimmed());
    println!();

    for line in exercise.description.lines() {
        println!("  {line}");
    }
    println!();

    if let Some(kc) = &exercise.key_concept {
        println!("  {} {}", "💡 Key concept:".bold().cyan(), kc);
    }
    if let Some(cm) = &exercise.common_mistake {
        println!("  {} {}", "⚠  Piège:".bold().yellow(), cm);
    }

    match exercise.validation.mode {
        ValidationMode::Test | ValidationMode::Both => {
            println!(
                "\n  {} Test-based validation (non supporté en CLI)",
                "⚠".yellow()
            );
        }
        _ => {}
    }

    println!();
}

/// Show editing instructions.
pub fn show_edit_instructions(source_path: &std::path::Path) {
    println!(
        "  {} {}",
        "✎ Edit:".bold().green(),
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
            println!("  {} {}", "║".red(), line.red());
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
        println!("  {} {}", "╔══".red(), "SORTIE INCORRECTE".bold().red());

        if let Some(expected) = &exercise.validation.expected_output {
            println!("  {} {}", "║".red(), "Attendu:".bold().green());
            for line in expected.trim().lines() {
                println!("  {}   {}", "║".red(), line.green());
            }
            println!("  {} {}", "║".red(), "Obtenu:".bold().red());
            for line in result.stdout.trim().lines() {
                println!("  {}   {}", "║".red(), line.red());
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

/// Show mastery update after successful exercise.
pub fn show_mastery_update(subject: &Subject, success: bool) {
    let icon = if success { "▲".green() } else { "▼".red() };
    let bar = mastery_bar(subject.mastery_score);
    println!(
        "  {} {} {} D{} │ {}/{} exercices",
        icon,
        subject.name.bold(),
        bar,
        subject.difficulty_unlocked,
        subject.attempts_success,
        subject.attempts_total
    );
    println!();
}

/// Show hints for an exercise.
pub fn show_hints(exercise: &Exercise) {
    if exercise.hints.is_empty() {
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

/// Show exercise list grouped by chapter.
pub fn show_exercise_list(
    exercises: &[Exercise],
    subjects: &[Subject],
    filter_subject: Option<&str>,
) {
    let filtered: Vec<&Exercise> = if let Some(filter) = filter_subject {
        exercises.iter().filter(|e| e.subject == filter).collect()
    } else {
        exercises.iter().collect()
    };

    if filtered.is_empty() {
        println!("{}", "  Aucun exercice trouvé.".dimmed());
        return;
    }

    let subject_map: std::collections::HashMap<&str, &Subject> =
        subjects.iter().map(|s| (s.name.as_str(), s)).collect();

    println!();
    show_banner();

    // Group by chapter
    for chapter in CHAPTERS {
        let chapter_exercises: Vec<&&Exercise> = filtered
            .iter()
            .filter(|e| chapter.subjects.iter().any(|&s| s == e.subject))
            .collect();

        if chapter_exercises.is_empty() {
            continue;
        }

        println!(
            "  {} Ch.{} — {}",
            "▸".bold().cyan(),
            chapter.number,
            chapter.title.bold()
        );

        for ex in chapter_exercises {
            let diff = match ex.difficulty {
                crate::models::Difficulty::Easy => "★☆☆".green(),
                crate::models::Difficulty::Medium => "★★☆".yellow(),
                crate::models::Difficulty::Hard => "★★★".red(),
            };
            let mastery_info = subject_map
                .get(ex.subject.as_str())
                .map(|s| format!(" [{:.1}]", s.mastery_score))
                .unwrap_or_default();

            println!(
                "    {} {} {}{}",
                diff,
                ex.id.dimmed(),
                ex.title,
                mastery_info.dimmed()
            );
        }
        println!();
    }

    // Uncategorized
    let known_subjects: std::collections::HashSet<&str> = CHAPTERS
        .iter()
        .flat_map(|ch| ch.subjects.iter().copied())
        .collect();

    let uncategorized: Vec<&&Exercise> = filtered
        .iter()
        .filter(|e| !known_subjects.contains(e.subject.as_str()))
        .collect();

    if !uncategorized.is_empty() {
        println!("  {} {}", "▸".bold(), "Divers".bold());
        for ex in uncategorized {
            let diff = match ex.difficulty {
                crate::models::Difficulty::Easy => "★☆☆".green(),
                crate::models::Difficulty::Medium => "★★☆".yellow(),
                crate::models::Difficulty::Hard => "★★★".red(),
            };
            println!("    {} {} {}", diff, ex.id.dimmed(), ex.title);
        }
        println!();
    }

    println!(
        "  {} {} exercices │ {} chapitres",
        "Total:".bold(),
        filtered.len(),
        CHAPTERS.len()
    );
    println!();
}

/// Show progress overview.
pub fn show_progress(subjects: &[Subject], streak: i64) {
    println!();
    show_banner();

    println!(
        "  {} (série: {} jours)\n",
        "Progression".bold().cyan(),
        streak.to_string().bold()
    );

    if subjects.is_empty() {
        println!(
            "  {}",
            "Pas encore de progrès. Lancez `kf watch` !".dimmed()
        );
        return;
    }

    // Group subjects by chapter
    for chapter in CHAPTERS {
        let chapter_subjects: Vec<&Subject> = subjects
            .iter()
            .filter(|s| chapter.subjects.iter().any(|&cs| cs == s.name))
            .collect();

        if chapter_subjects.is_empty() {
            continue;
        }

        println!(
            "  {} Ch.{} — {}",
            "▸".bold().cyan(),
            chapter.number,
            chapter.title.bold()
        );

        for sub in chapter_subjects {
            let bar = mastery_bar(sub.mastery_score);
            let success_rate = if sub.attempts_total > 0 {
                format!("{}/{}", sub.attempts_success, sub.attempts_total)
            } else {
                "—".to_string()
            };
            println!(
                "    {:<20} {} D{} │ {} │ SRS {}j",
                sub.name, bar, sub.difficulty_unlocked, success_rate, sub.srs_interval_days
            );
        }
        println!();
    }

    let total_mastery: f64 = subjects.iter().map(|s| s.mastery_score).sum();
    let count = subjects.len();
    if count > 0 {
        println!(
            "  {} {:.1}/5.0 moyenne globale",
            "Overall".bold(),
            total_mastery / count as f64
        );
    }
    println!();
}

/// Create a visual mastery bar with block characters.
fn mastery_bar(score: f64) -> String {
    let filled = (score / 5.0 * 10.0).round() as usize;
    let empty = 10 - filled.min(10);
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    let colored = if score >= 4.0 {
        bar.green().to_string()
    } else if score >= 2.0 {
        bar.yellow().to_string()
    } else {
        bar.red().to_string()
    };
    format!("{} {:.1}", colored, score)
}
