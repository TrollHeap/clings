use colored::{ColoredString, Colorize};
use serde::Deserialize;

use crate::chapters::{ChapterContext, CHAPTERS};
use crate::models::{Difficulty, Exercise, Subject, ValidationMode, VisVar};
use crate::runner::RunResult;

/// Une question d'annale NSY103.
#[derive(Debug, Deserialize)]
pub struct AnnaleQuestion {
    pub number: u32,
    pub points: f32,
    pub title: String,
    pub summary: String,
    pub subjects: Vec<String>,
}

/// Un examen NSY103 avec ses questions et le mapping vers les exercices.
#[derive(Debug, Deserialize)]
pub struct AnnaleExam {
    pub title: String,
    pub date: String,
    pub total_points: f32,
    pub questions: Vec<AnnaleQuestion>,
}

/// Render difficulty as colored star string.
pub fn difficulty_stars(d: Difficulty) -> ColoredString {
    match d {
        Difficulty::Easy => "★☆☆☆☆".green(),
        Difficulty::Medium => "★★☆☆☆".yellow(),
        Difficulty::Hard => "★★★☆☆".red(),
        Difficulty::Advanced => "★★★★☆".magenta(),
        Difficulty::Expert => "★★★★★".cyan(),
    }
}

// ─── Box-drawing helpers ────────────────────────────────────────────

const HEADER_WIDTH: usize = 56;
/// Visible text width between the ║ chars (HEADER_WIDTH - 2 - 2 side spaces = 52).
const INNER_W: usize = HEADER_WIDTH - 4;

fn hr() -> String {
    "─".repeat(HEADER_WIDTH)
}

fn header_box(title: &str) -> String {
    let pad = HEADER_WIDTH.saturating_sub(title.chars().count() + 6);
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
    let subtitle = "NSY103 — Programmation système Linux";
    let subtitle_len = subtitle.chars().count();
    let inner = HEADER_WIDTH - 2; // 54 chars between ║ and ║
    let total_pad = inner.saturating_sub(subtitle_len);
    let lp = total_pad / 2;
    let rp = total_pad - lp;
    println!(
        "  {}{}{}{}{}",
        "║".cyan(),
        " ".repeat(lp),
        subtitle.dimmed(),
        " ".repeat(rp),
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
        "  {}  {}   {}  {}   {}  {}   {}  {}",
        "│".dimmed(),
        difficulty_stars(exercise.difficulty),
        "│".dimmed(),
        exercise.exercise_type.to_string().dimmed(),
        "│".dimmed(),
        exercise.subject.dimmed(),
        "│".dimmed(),
        stage_label.dimmed(),
    );

    println!("  {}", hr().dimmed());
    println!();

    for line in exercise.description.lines() {
        if line.chars().count() > 72 {
            for wrapped in wrap_text(line, 72) {
                println!("  {wrapped}");
            }
        } else {
            println!("  {line}");
        }
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

/// Show keybind hints with optional visualizer key.
pub fn show_keybinds_with_vis(has_visualizer: bool) {
    if has_visualizer {
        println!(
            "  {} {} hint  {} skip  {} quit  {} list  {} check  {} visualiser",
            "Keys".bold().cyan(),
            "[h]".bold(),
            "[n]".bold(),
            "[q]".bold(),
            "[l]".bold(),
            "[c]".bold(),
            "[v]".bold(),
        );
    } else {
        show_keybinds();
        return;
    }
    println!();
}

/// Render a row of the visualizer with two equal columns (plain strings, colored inside).
/// Both args must be plain (no ANSI) so format! can count visible chars correctly.
fn vis_row(left: &str, right: &str) {
    const COL_W: usize = 26;
    let lp = format!("{:<COL_W$}", left).green();
    let rp = format!("{:<COL_W$}", right).cyan();
    println!("  {} {} {}{}", "║".yellow(), lp, rp, "║".yellow());
}

/// Like vis_row but right column is dimmed (e.g. heap vide).
fn vis_row_dim_right(left: &str, right: &str) {
    const COL_W: usize = 26;
    let lp = format!("{:<COL_W$}", left).green();
    let rp = format!("{:<COL_W$}", right).dimmed();
    println!("  {} {} {}{}", "║".yellow(), lp, rp, "║".yellow());
}

/// Word-wrap `text` to at most `width` visible chars per line.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current.clone());
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Render a single variable line.
fn fmt_var(v: &VisVar) -> String {
    format!("{}: {}", v.name, v.value)
}

/// Display a visualizer step for the given exercise.
/// Returns the number of terminal lines printed, so the caller can erase them
/// with `\x1b[{n}A\x1b[J` before redrawing on navigation.
pub fn show_visualizer(exercise: &Exercise, step: usize) -> usize {
    let steps = &exercise.visualizer.steps;
    if steps.is_empty() {
        return 0;
    }
    let step = step.min(steps.len() - 1);
    let s = &steps[step];
    let n = steps.len();
    let mut lines = 0usize;

    let title = format!("Visualiseur — {}/{}", step + 1, n);
    println!("  {}", header_box(&title).yellow());
    lines += 1;

    // Step progress dots — printed char-by-char to avoid ANSI padding issues
    print!("  {} ", "║".yellow());
    let mut visible_len = 0usize;
    for i in 0..n {
        if i > 0 {
            print!(" ");
            visible_len += 1;
        }
        if i == step {
            print!("{}", "●".yellow());
        } else {
            print!("{}", "○".dimmed());
        }
        visible_len += 1;
    }
    let step_info = format!("  Etape {}/{}", step + 1, n);
    print!("{}", step_info);
    visible_len += step_info.len();
    let pad = INNER_W.saturating_sub(visible_len);
    print!("{}", " ".repeat(pad));
    println!(" {}", "║".yellow());
    lines += 1;

    // Label row — prefer step_label (more descriptive), fallback to label
    let label_plain = if !s.step_label.is_empty() {
        s.step_label.clone()
    } else {
        s.label.clone()
    };
    let padded_label = format!("{:<INNER_W$}", label_plain);
    println!(
        "  {} {} {}",
        "║".yellow(),
        padded_label.bold(),
        "║".yellow()
    );
    lines += 1;

    // Separator
    println!(
        "  {}",
        format!("╠{}╣", "═".repeat(HEADER_WIDTH - 2)).yellow()
    );
    lines += 1;

    // Column headers (plain strings → colored inside vis_row)
    vis_row("STACK", "HEAP");
    lines += 1;

    // Variables
    let max_rows = s.stack.len().max(s.heap.len()).max(1);
    for i in 0..max_rows {
        let left = s.stack.get(i).map(fmt_var).unwrap_or_default();
        if s.heap.is_empty() && i == 0 {
            vis_row_dim_right(&left, "(vide)");
        } else {
            let right = s.heap.get(i).map(fmt_var).unwrap_or_default();
            vis_row(&left, &right);
        }
    }
    lines += max_rows;

    // Explanation separator
    println!(
        "  {}",
        format!("╠{}╣", "═".repeat(HEADER_WIDTH - 2)).yellow()
    );
    lines += 1;

    // Explanation — word-wrapped, padded, then dimmed
    const WRAP_W: usize = HEADER_WIDTH - 6; // 50
    if !s.explanation.is_empty() {
        let exp_lines = wrap_text(&s.explanation, WRAP_W);
        for line in &exp_lines {
            let padded = format!("{:<INNER_W$}", line);
            println!("  {} {} {}", "║".yellow(), padded.dimmed(), "║".yellow());
        }
        lines += exp_lines.len();
    }

    // Navigation hints — ASCII arrows to avoid raw-mode multi-byte issues
    let nav = "[<] prec   [>] suiv   [v] fermer";
    let padded_nav = format!("{:<INNER_W$}", nav);
    println!(
        "  {} {} {}",
        "║".yellow(),
        padded_nav.dimmed(),
        "║".yellow()
    );
    lines += 1;

    println!("  {}", footer_box().yellow());
    lines += 1;

    println!();
    lines += 1;

    lines
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
        difficulty_stars(exercise.difficulty),
        "│".dimmed(),
        exercise.subject.dimmed(),
    );
    println!("  {}", hr().dimmed());
    println!();

    for line in exercise.description.lines() {
        if line.chars().count() > 72 {
            for wrapped in wrap_text(line, 72) {
                println!("  {wrapped}");
            }
        } else {
            println!("  {line}");
        }
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
            let diff = difficulty_stars(ex.difficulty);
            let mastery_info = subject_map
                .get(ex.subject.as_str())
                .map(|s| format!(" [{:.1}]", s.mastery_score))
                .unwrap_or_default();
            let kc_info = if !ex.kc_ids.is_empty() {
                format!(" [{}]", ex.kc_ids.join(", "))
            } else {
                String::new()
            };

            println!(
                "    {} {} {}{}{}",
                diff,
                ex.id.dimmed(),
                ex.title,
                mastery_info.dimmed(),
                kc_info.dimmed()
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
            let diff = difficulty_stars(ex.difficulty);
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
pub fn mastery_bar(score: f64) -> String {
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

/// Show global statistics: streak, average mastery, top/bottom subjects.
pub fn show_stats(subjects: &[Subject], streak: u32) {
    println!();
    show_banner();

    println!("  {}", header_box("KernelForge — Statistiques").cyan());
    println!();

    // Streak
    println!(
        "  {} {}  {}",
        "Série:".bold().cyan(),
        streak.to_string().bold().yellow(),
        "jours consécutifs".dimmed()
    );

    // Average mastery
    if subjects.is_empty() {
        println!("  {}", "Aucun sujet pratiqué pour l'instant.".dimmed());
        println!();
        println!("  {}", footer_box().cyan());
        return;
    }

    let total_mastery: f64 = subjects.iter().map(|s| s.mastery_score).sum();
    let avg = total_mastery / subjects.len() as f64;
    println!(
        "  {} {}",
        "Maîtrise moyenne:".bold().cyan(),
        mastery_bar(avg)
    );
    println!();

    // Sort by mastery descending
    let mut sorted: Vec<&Subject> = subjects.iter().collect();
    sorted.sort_by(|a, b| {
        b.mastery_score
            .partial_cmp(&a.mastery_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Header row
    println!("  {}", hr().dimmed());
    println!(
        "  {:<22} {:<16} {}",
        "Sujet".bold(),
        "Mastery".bold(),
        "Bar".bold()
    );
    println!("  {}", hr().dimmed());

    const TOP_N: usize = 5;

    // Top subjects
    if sorted.len() > TOP_N * 2 {
        println!("  {}", "── Top sujets ──".dimmed());
        for sub in sorted.iter().take(TOP_N) {
            println!(
                "  {:<22} {:<6.1}  {}",
                sub.name,
                sub.mastery_score,
                mastery_bar(sub.mastery_score)
            );
        }
        println!("  {}", "── À renforcer ──".dimmed());
        for sub in sorted.iter().rev().take(TOP_N) {
            println!(
                "  {:<22} {:<6.1}  {}",
                sub.name,
                sub.mastery_score,
                mastery_bar(sub.mastery_score)
            );
        }
    } else {
        for sub in &sorted {
            println!(
                "  {:<22} {:<6.1}  {}",
                sub.name,
                sub.mastery_score,
                mastery_bar(sub.mastery_score)
            );
        }
    }

    println!("  {}", hr().dimmed());
    println!();
    println!("  {}", "Continuez à pratiquer !".bold().green());
    println!();
    println!("  {}", footer_box().cyan());
    println!();
}

/// Affiche les annales NSY103 avec le mapping vers les exercices KernelForge.
pub fn show_annales(annales: &[AnnaleExam], exercises: &[Exercise]) {
    println!();
    show_banner();
    println!(
        "  {} {}\n",
        "Annales NSY103".bold().cyan(),
        "— correspondance exercices KernelForge".dimmed()
    );

    for exam in annales {
        println!(
            "  {} {} — {} ({}pt)",
            "▸".bold().cyan(),
            exam.title.bold(),
            exam.date.dimmed(),
            exam.total_points
        );
        println!("  {}", hr().dimmed());

        for q in &exam.questions {
            let pts = format!("({:.0}pt)", q.points);
            println!(
                "  Q{} {} {} — {}",
                q.number,
                pts.dimmed(),
                q.title.bold(),
                q.summary.dimmed()
            );

            if !q.subjects.is_empty() {
                println!(
                    "    {} {}",
                    "Sujets:".dimmed(),
                    q.subjects.join(", ").cyan()
                );
            }

            let related: Vec<&Exercise> = exercises
                .iter()
                .filter(|e| q.subjects.iter().any(|s| s == &e.subject))
                .collect();

            if related.is_empty() {
                println!("    {}", "Aucun exercice associé.".dimmed());
            } else {
                let ids: Vec<&str> = related.iter().map(|e| e.id.as_str()).collect();
                let shown = &ids[..ids.len().min(5)];
                let more = if ids.len() > 5 {
                    format!(" +{} autres", ids.len() - 5)
                } else {
                    String::new()
                };
                println!(
                    "    {} {}{}",
                    "Exercices:".dimmed(),
                    shown.join(", ").green(),
                    more.dimmed()
                );
            }
            println!();
        }
    }

    println!(
        "  {} `kf list --subject <sujet>` pour voir tous les exercices d'un sujet.",
        "Astuce:".bold().yellow()
    );
    println!();
}
