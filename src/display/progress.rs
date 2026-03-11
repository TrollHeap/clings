use colored::Colorize;

use crate::chapters::{ChapterContext, CHAPTERS};
use crate::constants::{MINIMAP_MAX_ITEMS, PROGRESS_BAR_WIDTH};
use crate::models::Subject;

use super::mastery_bar;

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
    let bar_width = PROGRESS_BAR_WIDTH;
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
        "Progression".bold(),
        colored_bar,
        done.to_string().bold(),
        total,
        pct
    );

    // Mini-map with Unicode dots
    if total <= MINIMAP_MAX_ITEMS {
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

/// Show progress overview.
pub fn show_progress(subjects: &[Subject], streak: i64) {
    println!();
    super::show_banner();

    println!(
        "  {} (série: {} jours)\n",
        "Progression".bold().cyan(),
        streak.to_string().bold()
    );

    if subjects.is_empty() {
        println!(
            "  {}",
            "Pas encore de progrès. Lancez `clings watch` !".dimmed()
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
            "Global".bold(),
            total_mastery / count as f64
        );
    }
    println!();
}
