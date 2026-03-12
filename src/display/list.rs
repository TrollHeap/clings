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
    let filtered: Vec<&Exercise> = if let Some(filter) = filter_subject {
        exercises.iter().filter(|e| e.subject == filter).collect()
    } else {
        exercises.iter().collect()
    };

    // Filtre --due : garder uniquement les exercices des sujets dus
    let filtered: Vec<&Exercise> = if let Some(due) = filter_due {
        filtered
            .into_iter()
            .filter(|e| due.iter().any(|d| d == &e.subject))
            .collect()
    } else {
        filtered
    };

    if filtered.is_empty() {
        println!("{}", "  Aucun exercice trouvé.".dimmed());
        return;
    }

    let subject_map: std::collections::HashMap<&str, &Subject> =
        subjects.iter().map(|s| (s.name.as_str(), s)).collect();

    println!();
    show_banner();

    if filter_due.is_some() {
        println!("  {} exercices dus en révision SRS", "★".bold().yellow());
        println!();
    }

    // Group by chapter
    for chapter in CHAPTERS {
        let chapter_exercises: Vec<&Exercise> = filtered
            .iter()
            .copied()
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
            println!(
                "{}",
                render_exercise_row(ex, subject_map.get(ex.subject.as_str()).copied())
            );
        }
        println!();
    }

    // Uncategorized
    let known_subjects: std::collections::HashSet<&str> = CHAPTERS
        .iter()
        .flat_map(|ch| ch.subjects.iter().copied())
        .collect();

    let uncategorized: Vec<&Exercise> = filtered
        .iter()
        .copied()
        .filter(|e| !known_subjects.contains(e.subject.as_str()))
        .collect();

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
