//! Commandes d'information — list, hint, solution, annales, search.

use colored::Colorize;

use crate::error::{KfError, Result};
use crate::{exercises, progress, search};

pub fn cmd_list(filter_subject: Option<&str>, filter_due: bool) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let conn = progress::open_db()?;
    let subjects = progress::get_all_subjects(&conn)?;

    let due_subjects: Option<Vec<String>> = if filter_due {
        Some(progress::get_due_subjects(&conn)?)
    } else {
        None
    };

    crate::tui::ui_list::run_list(
        &all_exercises,
        &subjects,
        filter_subject,
        due_subjects.as_deref(),
    )
}

pub fn cmd_hint(exercise_id: &str) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| KfError::ExerciseNotFound(exercise_id.to_string()))?;

    println!();
    println!(
        "  {} — {}",
        "Indices".bold().cyan(),
        exercise.title.bold().green()
    );
    println!();
    for (i, hint) in exercise.hints.iter().enumerate() {
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
    Ok(())
}

pub fn cmd_solution(exercise_id: &str) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| KfError::ExerciseNotFound(exercise_id.to_string()))?;

    let conn = progress::open_db()?;
    let mut stmt =
        conn.prepare_cached("SELECT COUNT(*) FROM practice_log WHERE exercise_id = ?1")?;
    let count: i64 = stmt.query_row([exercise_id], |row| row.get(0))?;

    if count == 0 {
        println!(
            "  {} Vous devez tenter l'exercice au moins une fois avant de voir la solution.",
            "Verrouillé:".bold().yellow()
        );
        println!("  Lancer : clings run {exercise_id}");
        return Ok(());
    }

    println!();
    println!(
        "  {} — {}",
        "Solution".bold().cyan(),
        exercise.title.bold().green()
    );
    println!();
    println!("  Code solution :");
    println!();
    for line in exercise.solution_code.lines() {
        println!("      {}", line);
    }
    println!();
    Ok(())
}

pub fn cmd_annales() -> Result<()> {
    let annales = exercises::load_annales_map()?;
    let (all_exercises, _) = exercises::load_all_exercises()?;
    crate::tui::ui_annales::run_annales(&annales, &all_exercises)
}

pub fn cmd_search(query: &str, filter_subject: Option<&str>) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let conn = progress::open_db()?;
    let subjects = progress::get_all_subjects(&conn)?;

    let results = search::search_exercises(&all_exercises, query, filter_subject);
    if results.is_empty() {
        println!("  {} Aucun exercice trouvé pour « {query} ».", "✗".dimmed());
    } else {
        println!();
        println!(
            "  {} {} résultats trouvé(s) pour « {query} »",
            "🔍".cyan(),
            results.len().to_string().bold()
        );
        println!();

        // Build subject map for mastery score display
        let subject_map: std::collections::HashMap<&str, &crate::models::Subject> =
            subjects.iter().map(|s| (s.name.as_str(), s)).collect();

        for (i, (idx, _score)) in results.iter().enumerate() {
            let exercise = &all_exercises[*idx];
            let mastery = subject_map
                .get(exercise.subject.as_str())
                .map(|s| s.mastery_score.get())
                .unwrap_or(0.0);
            let diff_stars = match exercise.difficulty {
                crate::models::Difficulty::Easy => "★☆☆☆☆".green(),
                crate::models::Difficulty::Medium => "★★☆☆☆".yellow(),
                crate::models::Difficulty::Hard => "★★★☆☆".red(),
                crate::models::Difficulty::Advanced => "★★★★☆".magenta(),
                crate::models::Difficulty::Expert => "★★★★★".cyan(),
            };

            println!(
                "  {} [{}] {} | {} | {}",
                (i + 1).to_string().bold(),
                exercise.id.yellow(),
                exercise.title.bold().green(),
                diff_stars,
                format!("{:.1}/5.0", mastery).dimmed()
            );
        }
        println!();
    }
    Ok(())
}
