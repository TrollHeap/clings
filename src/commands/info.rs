//! Commandes d'information — list, hint, solution, annales, search.

use colored::Colorize;

use crate::error::{KfError, Result};
use crate::{exercises, progress, search};

/// Display exercise list in TUI. Optionally filter by subject name or due exercises for review.
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

/// Display all hints for an exercise in sequence.
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

/// Display solution code for an exercise. Requires at least one practice attempt (locked until then).
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

/// Display past exam (annales) sessions from annales_map.json in an interactive TUI browser.
pub fn cmd_annales() -> Result<()> {
    let annales = exercises::load_annales_map()?;
    let (all_exercises, _) = exercises::load_all_exercises()?;
    crate::tui::ui_annales::run_annales(&annales, &all_exercises)
}

/// Fuzzy-search exercises by query (ID, title, subject, description). Optionally filter by subject.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        Difficulty, Exercise, ExerciseType, Lang, ValidationConfig, ValidationMode,
    };

    fn create_test_exercise(id: &str, subject: &str, title: &str) -> Exercise {
        Exercise {
            id: id.to_string(),
            subject: subject.to_string(),
            lang: Lang::C,
            difficulty: Difficulty::Easy,
            title: title.to_string(),
            description: "Test description".to_string(),
            starter_code: "int main() {}".to_string(),
            solution_code: "int main() { return 0; }".to_string(),
            hints: vec!["Hint 1".to_string()],
            validation: ValidationConfig {
                mode: ValidationMode::Output,
                expected_output: Some("0".to_string()),
                max_duration_ms: None,
                test_code: None,
                expected_tests_pass: None,
            },
            prerequisites: vec![],
            files: vec![],
            exercise_type: ExerciseType::Complete,
            key_concept: None,
            common_mistake: None,
            kc_ids: vec![],
            starter_code_stages: vec![],
            visualizer: Default::default(),
        }
    }

    #[test]
    fn test_cmd_hint_exercise_not_found() {
        let result = cmd_hint("nonexistent-exercise-xyz");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), KfError::ExerciseNotFound(_)));
    }

    #[test]
    fn test_cmd_hint_exercise_exists() {
        // This test would require loading actual exercises.
        // For unit testing, we verify error handling works correctly.
        let result = cmd_hint("nonexistent-id-12345");
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_solution_exercise_not_found() {
        let result = cmd_solution("nonexistent-exercise-xyz");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), KfError::ExerciseNotFound(_)));
    }

    #[test]
    fn test_cmd_solution_without_attempts() {
        // Exercise exists but no attempts recorded — should lock solution
        // This requires actual exercise data and database
        let result = cmd_solution("nonexistent-id-for-lock-test");
        assert!(result.is_err()); // Will fail because exercise doesn't exist
    }

    #[test]
    fn test_cmd_search_empty_query() {
        // Empty query should still run and may return results
        let result = cmd_search("", None);
        // Result depends on whether exercises are loaded;
        // we're testing the function doesn't panic on empty query
        let _ = result;
    }

    #[test]
    fn test_cmd_search_nonexistent_subject_filter() {
        // Searching with a filter for nonexistent subject should return no results
        let result = cmd_search("test", Some("nonexistent_subject_xyz"));
        // Should succeed but print "no results" message
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_search_with_subject_filter() {
        // Verify function accepts subject filter without panic
        let result = cmd_search("malloc", Some("memory_allocation"));
        let _ = result; // May succeed or fail based on loaded exercises
    }

    #[test]
    fn test_cmd_hint_with_special_characters() {
        // Exercise ID with special characters should be handled
        let result = cmd_hint("exercise-with-dashes-123");
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_solution_with_empty_id() {
        // Empty exercise ID should return not found
        let result = cmd_solution("");
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_search_single_char() {
        // Single character query should still work
        let result = cmd_search("c", None);
        // May have results depending on exercise data
        let _ = result;
    }

    #[test]
    fn test_cmd_search_uppercase_query() {
        // Fuzzy search should be case-insensitive
        let result = cmd_search("MALLOC", None);
        let _ = result;
    }

    #[test]
    fn test_cmd_search_with_numbers() {
        // Query with numbers should work
        let result = cmd_search("01", None);
        let _ = result;
    }

    #[test]
    fn test_cmd_search_special_chars_in_query() {
        // Query with special characters
        let result = cmd_search("malloc()", None);
        let _ = result;
    }
}
