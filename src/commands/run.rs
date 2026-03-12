//! Commandes run et review — exécution et renforcement d'exercices.

use colored::Colorize;

use crate::error::{KfError, Result};
use crate::{exercises, progress};

pub fn cmd_run(exercise_id: &str) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| KfError::ExerciseNotFound(exercise_id.to_string()))?;

    let conn = progress::open_db()?;
    let subject_mastery =
        progress::get_subject(&conn, &exercise.subject)?.map(|s| s.mastery_score.get());

    crate::tui::ui_run::run_exercise(exercise, subject_mastery)
}

pub fn cmd_review() -> Result<()> {
    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;

    let due = progress::get_due_subjects(&conn)?;
    if due.is_empty() {
        println!(
            "  {}",
            "Aucun sujet à renforcer pour l'instant. Revenez plus tard !".dimmed()
        );
        return Ok(());
    }

    let (all_exercises, _) = exercises::load_all_exercises()?;

    // Build subject map: name → Subject (mastery_score + next_review_at)
    let subjects = progress::get_all_subjects(&conn)?;
    let subject_map: std::collections::HashMap<&str, &crate::models::Subject> =
        subjects.iter().map(|s| (s.name.as_str(), s)).collect();

    // Pre-build O(1) lookup maps for the review loop.
    let exercise_by_id: std::collections::HashMap<&str, &crate::models::Exercise> =
        all_exercises.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut exercise_by_subject: std::collections::HashMap<&str, &crate::models::Exercise> =
        std::collections::HashMap::new();
    for e in &all_exercises {
        exercise_by_subject.entry(e.subject.as_str()).or_insert(e);
    }

    // For each due subject, prefer the weakest exercise (by exercise_scores); fallback to first
    let weakest_by_subject = progress::get_all_weakest_exercises(&conn).unwrap_or_else(|e| {
        eprintln!("  [clings] avertissement : weakest_exercises indisponible : {e}");
        std::collections::HashMap::new()
    });
    let mut due_exercises: Vec<&crate::models::Exercise> = due
        .iter()
        .filter_map(|subject_name| {
            // Prioritise the exercise with the lowest success rate for this subject.
            if let Some(id) = weakest_by_subject.get(subject_name.as_str()) {
                if let Some(ex) = exercise_by_id.get(id.as_str()) {
                    return Some(*ex);
                }
            }
            // Fallback: first exercise belonging to this subject
            exercise_by_subject.get(subject_name.as_str()).copied()
        })
        .collect();

    // Sort by (mastery_score ASC, next_review_at ASC) — weakest and most-overdue first
    due_exercises.sort_by(|a, b| {
        let sa = subject_map.get(a.subject.as_str());
        let sb = subject_map.get(b.subject.as_str());
        let ma = sa.map_or(0.0, |s| s.mastery_score.get());
        let mb = sb.map_or(0.0, |s| s.mastery_score.get());
        ma.partial_cmp(&mb)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let ra = sa.and_then(|s| s.next_review_at).unwrap_or(i64::MAX);
                let rb = sb.and_then(|s| s.next_review_at).unwrap_or(i64::MAX);
                ra.cmp(&rb)
            })
    });

    let total = due_exercises.len();
    println!(
        "  {} {} sujet(s) à renforcer",
        "Renforcement mastery —".bold().cyan(),
        total.to_string().bold()
    );
    println!();

    for (i, exercise) in due_exercises.iter().enumerate() {
        println!(
            "  {} [{}/{}] {}",
            "Exercice".bold().cyan(),
            i + 1,
            total,
            exercise.title.bold().green()
        );
        println!("  {} {}", "Sujet:".dimmed(), exercise.subject.dimmed());
        println!();
        match cmd_run(&exercise.id) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("  {} {e}", "Erreur:".bold().red());
            }
        }
        println!();
    }

    println!(
        "  {} Session de renforcement terminée ({} exercices).",
        "✓".bold().green(),
        total
    );

    Ok(())
}
