use colored::Colorize;

use crate::chapters::CHAPTERS;
use crate::models::{Exercise, Subject};

use super::{difficulty_stars, show_banner};

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

    let uncategorized: Vec<&Exercise> = filtered
        .iter()
        .copied()
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
