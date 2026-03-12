//! Commandes d'information — list, hint, solution, annales, search.

use colored::Colorize;

use crate::error::{KfError, Result};
use crate::{display, exercises, progress, search};

pub fn cmd_list(filter_subject: Option<&str>, filter_due: bool) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let conn = progress::open_db()?;
    let subjects = progress::get_all_subjects(&conn)?;

    let due_subjects: Option<Vec<String>> = if filter_due {
        Some(progress::get_due_subjects(&conn)?)
    } else {
        None
    };

    display::show_exercise_list(
        &all_exercises,
        &subjects,
        filter_subject,
        due_subjects.as_deref(),
    );
    Ok(())
}

pub fn cmd_hint(exercise_id: &str) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| KfError::ExerciseNotFound(exercise_id.to_string()))?;
    display::show_hints(exercise);
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

    display::show_solution(exercise);
    Ok(())
}

pub fn cmd_annales() -> Result<()> {
    let annales = exercises::load_annales_map()?;
    let (all_exercises, _) = exercises::load_all_exercises()?;
    display::show_annales(&annales, &all_exercises);
    Ok(())
}

pub fn cmd_search(query: &str, filter_subject: Option<&str>) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let conn = progress::open_db()?;
    let subjects = progress::get_all_subjects(&conn)?;

    let results = search::search_exercises(&all_exercises, query, filter_subject);
    if results.is_empty() {
        println!("Aucun exercice trouvé pour « {query} ».");
    } else {
        let matched: Vec<crate::models::Exercise> =
            results.iter().map(|(ex, _)| (*ex).clone()).collect();
        display::show_exercise_list(&matched, &subjects, None, None);
    }
    Ok(())
}
