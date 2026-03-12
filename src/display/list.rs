//! Exercise list display — grouped by NSY103 chapter.

use colored::Colorize;

use crate::chapters::CHAPTERS;
use crate::models::{Exercise, Subject};

use super::{difficulty_stars, show_banner};

fn render_exercise_row(ex: &Exercise, subject: Option<&Subject>) -> String {
    let diff = difficulty_stars(ex.difficulty);
    let mastery_info = subject
        .map(|s| format!(" [{:.1}]", s.mastery_score.get()))
        .unwrap_or_default();
    let kc_info = if !ex.kc_ids.is_empty() {
        format!(" [{}]", ex.kc_ids.join(", "))
    } else {
        String::new()
    };
    format!(
        "    {} {} {}{}{}",
        diff,
        ex.id.dimmed(),
        ex.title,
        mastery_info.dimmed(),
        kc_info.dimmed()
    )
}

/// Show exercise list grouped by chapter.
pub fn show_exercise_list(
    exercises: &[Exercise],
    subjects: &[Subject],
    filter_subject: Option<&str>,
    filter_due: Option<&[String]>,
) {
    // Single-pass filter combining subject and due filters.
    let filtered: Vec<&Exercise> = exercises
        .iter()
        .filter(|e| {
            filter_subject.is_none_or(|f| e.subject == f)
                && filter_due.is_none_or(|due| due.iter().any(|d| d == &e.subject))
        })
        .collect();

    if filtered.is_empty() {
        println!("{}", "  Aucun exercice trouvé.".dimmed());
        return;
    }

    let subject_map: std::collections::HashMap<&str, &Subject> =
        subjects.iter().map(|s| (s.name.as_str(), s)).collect();

    // Pre-build subject → chapter index for O(1) grouping.
    let subject_to_chapter: std::collections::HashMap<&str, usize> = CHAPTERS
        .iter()
        .enumerate()
        .flat_map(|(i, ch)| ch.subjects.iter().copied().map(move |s| (s, i)))
        .collect();

    // Group exercises into chapter buckets in a single O(n) pass.
    let mut by_chapter: Vec<Vec<&Exercise>> = vec![Vec::new(); CHAPTERS.len()];
    let mut uncategorized: Vec<&Exercise> = Vec::new();
    for ex in &filtered {
        if let Some(&ch_idx) = subject_to_chapter.get(ex.subject.as_str()) {
            by_chapter[ch_idx].push(ex);
        } else {
            uncategorized.push(ex);
        }
    }

    println!();
    show_banner();

    if filter_due.is_some() {
        println!("  {} exercices dus en révision SRS", "★".bold().yellow());
        println!();
    }

    // Render in chapter order.
    for (chapter, chapter_exercises) in CHAPTERS.iter().zip(by_chapter.iter()) {
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
            println!(
                "{}",
                render_exercise_row(ex, subject_map.get(ex.subject.as_str()).copied())
            );
        }
        println!();
    }

    // Uncategorized exercises (subjects not in any chapter).
    if !uncategorized.is_empty() {
        println!("  {} {}", "▸".bold(), "Divers".bold());
        for ex in uncategorized {
            println!("{}", render_exercise_row(ex, None));
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
