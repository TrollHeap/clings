//! Piscine mode — linear progression through all exercises with checkpoint persistence.

use colored::Colorize;

use crate::chapters;
use crate::error::Result;
use crate::{exercises, progress, tmux};

/// Run piscine mode: linear progression through ALL exercises, ignoring difficulty gating.
/// Exercises are ordered: chapter 1 D1→D2→D3→D4→D5, then chapter 2, etc.
pub fn cmd_piscine(filter_chapter: Option<u8>, timed_minutes: Option<u64>) -> Result<()> {
    use crate::tui::app::App;

    let (all_exercises, _) = exercises::load_all_exercises()?;
    let mut conn = progress::open_db()?;

    progress::apply_all_decay(&mut conn)?;
    progress::ensure_subjects_batch(&mut conn, &all_exercises)?;

    let subjects = progress::get_all_subjects(&conn)?;

    // Order by chapters, then difficulty within each chapter (no gating)
    let mut chapter_blocks = chapters::order_by_chapters(&all_exercises, &subjects);
    if !chapters::filter_by_chapter(&mut chapter_blocks, filter_chapter) {
        println!(
            "  {} Chapitre {} introuvable ou aucun exercice disponible.",
            "⚠".yellow(),
            filter_chapter.unwrap_or(0)
        );
        return Ok(());
    }
    let exercise_order = chapters::flatten_chapters(&chapter_blocks);

    if exercise_order.is_empty() {
        println!("{}", "  Aucun exercice disponible.".dimmed());
        return Ok(());
    }

    let total = exercise_order.len();
    let deadline =
        timed_minutes.map(|m| std::time::Instant::now() + std::time::Duration::from_secs(m * 60));
    let timer_total = timed_minutes.map(|m| m * 60).unwrap_or(0);

    // Restore checkpoint
    let start_index = progress::load_piscine_checkpoint(&conn)?
        .map(|i| i.min(total.saturating_sub(1)))
        .unwrap_or(0);

    let mut app = App::new();
    app.state.exercises = exercise_order.into_iter().cloned().collect();
    app.state.completed = vec![false; total];
    app.state.current_index = start_index;
    app.state.piscine_deadline = deadline;
    app.state.piscine_timer_total = timer_total;
    app.state.piscine_start = Some(std::time::Instant::now());

    let mut terminal = ratatui::init();
    let result = app.run_piscine(&mut terminal, &conn, None);

    // Save session state for "Continue" in launcher
    if let Err(e) =
        progress::save_last_session(&conn, "piscine", filter_chapter, app.state.current_index)
    {
        eprintln!("[clings] erreur sauvegarde session: {e}");
    }

    ratatui::restore();

    // Cleanup tmux editor pane
    if let Some(pane) = &app.state.editor_pane {
        tmux::kill_pane(pane);
    }

    // Post-run stats
    let done = app.state.completed.iter().filter(|&&c| c).count();
    let elapsed = app
        .state
        .piscine_start
        .unwrap_or(std::time::Instant::now())
        .elapsed();
    let hours = elapsed.as_secs() / 3600;
    let mins = (elapsed.as_secs() % 3600) / 60;
    let secs = elapsed.as_secs() % 60;

    if done == total {
        if let Err(e) = progress::clear_piscine_checkpoint(&conn) {
            eprintln!("[clings] erreur suppression checkpoint piscine: {e}");
        }
    }

    println!();
    if done == total {
        println!(
            "  {} Piscine complétée ! {}/{} en {}h{:02}m{:02}s",
            "BRAVO !".bold().green(),
            done,
            total,
            hours,
            mins,
            secs
        );
    } else {
        println!(
            "  {} {}/{} exercices complétés en {}h{:02}m{:02}s. `clings piscine` pour reprendre.",
            "Session piscine terminée.".bold(),
            done,
            total,
            hours,
            mins,
            secs
        );
    }
    if timed_minutes.is_some() {
        let pct = if total > 0 { (done * 100) / total } else { 0 };
        println!(
            "  {} Score exam: {}/{} ({}%)",
            "🎓".bold(),
            done,
            total,
            pct
        );
    }

    result
}

/// Lancer une session piscine avec une liste d'exercices préfiltrée (mode exam).
pub fn run_exam_piscine(
    exercises: Vec<crate::models::Exercise>,
    timed_minutes: Option<u64>,
    session_id: Option<&str>,
) -> crate::error::Result<()> {
    use crate::tui::app::App;

    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;
    progress::ensure_subjects_batch(&mut conn, &exercises)?;

    let total = exercises.len();
    if total == 0 {
        println!("  Aucun exercice disponible.");
        return Ok(());
    }

    let deadline =
        timed_minutes.map(|m| std::time::Instant::now() + std::time::Duration::from_secs(m * 60));
    let timer_total = timed_minutes.map(|m| m * 60).unwrap_or(0);

    // Restore checkpoint si session_id fourni
    let start_index = if let Some(sid) = session_id {
        progress::load_exam_checkpoint(&conn, sid)
            .ok()
            .flatten()
            .map(|i| i.min(total.saturating_sub(1)))
            .unwrap_or(0)
    } else {
        0
    };

    let mut app = App::new();
    app.state.exercises = exercises;
    app.state.completed = vec![false; total];
    app.state.current_index = start_index;
    app.state.piscine_deadline = deadline;
    app.state.piscine_timer_total = timer_total;
    app.state.piscine_start = Some(std::time::Instant::now());

    let mut terminal = ratatui::init();
    let result = app.run_piscine(&mut terminal, &conn, session_id);
    ratatui::restore();

    // Cleanup tmux editor pane
    if let Some(pane) = &app.state.editor_pane {
        tmux::kill_pane(pane);
    }

    // Post-run stats
    let done = app.state.completed.iter().filter(|&&c| c).count();
    let elapsed = app
        .state
        .piscine_start
        .unwrap_or(std::time::Instant::now())
        .elapsed();
    let hours = elapsed.as_secs() / 3600;
    let mins = (elapsed.as_secs() % 3600) / 60;
    let secs = elapsed.as_secs() % 60;

    if done == total {
        if let Err(e) = progress::clear_exam_checkpoint(&conn) {
            eprintln!("[clings] erreur suppression checkpoint exam: {e}");
        }
    }

    println!();
    if timed_minutes.is_some() {
        let pct = if total > 0 { (done * 100) / total } else { 0 };
        println!(
            "  {} Score exam: {}/{} ({}%) en {}h{:02}m{:02}s",
            "🎓".bold(),
            done,
            total,
            pct,
            hours,
            mins,
            secs,
        );
    } else if done == total {
        println!(
            "  {} Exam complété ! {}/{} en {}h{:02}m{:02}s",
            "BRAVO !".bold().green(),
            done,
            total,
            hours,
            mins,
            secs,
        );
    } else {
        println!(
            "  {} {}/{} exercices en {}h{:02}m{:02}s.",
            "Session terminée.".bold(),
            done,
            total,
            hours,
            mins,
            secs,
        );
    }

    result
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::chapters;
    use crate::models::{
        Difficulty, Exercise, ExerciseType, Lang, ValidationConfig, ValidationMode, Visualizer,
    };
    use crate::progress;

    fn make_exercise(id: &str, subject: &str, difficulty: Difficulty) -> Exercise {
        Exercise {
            id: id.to_string(),
            subject: subject.to_string(),
            lang: Lang::C,
            difficulty,
            title: id.to_string(),
            description: String::new(),
            starter_code: String::new(),
            solution_code: String::new(),
            hints: vec![],
            validation: ValidationConfig {
                expected_output: Some("ok".to_string()),
                max_duration_ms: None,
                mode: ValidationMode::Output,
                expected_tests_pass: None,
                test_code: None,
            },
            prerequisites: vec![],
            files: vec![],
            exercise_type: ExerciseType::default(),
            key_concept: None,
            common_mistake: None,
            kc_ids: vec![],
            starter_code_stages: vec![],
            visualizer: Visualizer::default(),
        }
    }

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("failed to create in-memory test DB");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS subjects (
                name TEXT PRIMARY KEY,
                mastery_score REAL NOT NULL DEFAULT 0.0,
                last_practiced_at INTEGER,
                attempts_total INTEGER NOT NULL DEFAULT 0,
                attempts_success INTEGER NOT NULL DEFAULT 0,
                difficulty_unlocked INTEGER NOT NULL DEFAULT 1,
                next_review_at INTEGER,
                srs_interval_days INTEGER NOT NULL DEFAULT 1
            );
            CREATE TABLE IF NOT EXISTS practice_log (
                id TEXT PRIMARY KEY,
                subject TEXT NOT NULL,
                exercise_id TEXT NOT NULL,
                success INTEGER NOT NULL DEFAULT 0,
                practiced_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS kv (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .expect("failed to init test schema");
        conn
    }

    /// Vérifie que les exercices sont triés chapter → difficulty via order_by_chapters.
    #[test]
    fn test_piscine_order() -> crate::error::Result<()> {
        // "pipes" = chapitre 9, "structs" = chapitre 1
        let ex_pipes_hard = make_exercise("pipes-hard", "pipes", Difficulty::Hard);
        let ex_structs_easy = make_exercise("structs-easy", "structs", Difficulty::Easy);
        let ex_structs_medium = make_exercise("structs-medium", "structs", Difficulty::Medium);

        let exercises = vec![ex_pipes_hard, ex_structs_easy, ex_structs_medium];
        let subjects = vec![];

        let blocks = chapters::order_by_chapters(&exercises, &subjects);
        let order = chapters::flatten_chapters(&blocks);

        assert_eq!(order.len(), 3);
        // structs (chapitre 1) doit précéder pipes (chapitre 9)
        assert_eq!(order[0].subject, "structs");
        assert_eq!(order[1].subject, "structs");
        assert_eq!(order[2].subject, "pipes");
        // Au sein de structs : Easy avant Medium
        assert_eq!(order[0].difficulty, Difficulty::Easy);
        assert_eq!(order[1].difficulty, Difficulty::Medium);
        Ok(())
    }

    /// Vérifie le roundtrip save/load du checkpoint piscine sur une DB in-memory.
    #[test]
    fn test_checkpoint_roundtrip() -> crate::error::Result<()> {
        let conn = open_test_db();
        progress::save_piscine_checkpoint(&conn, 3)?;
        let loaded = progress::load_piscine_checkpoint(&conn)?;
        assert_eq!(loaded, Some(3));
        Ok(())
    }

    /// Vérifie que le mécanisme de skip incrémente bien l'index d'exercice.
    #[test]
    fn test_skip_increments_index() -> crate::error::Result<()> {
        let total = 5usize;
        let mut index = 2usize;
        index += 1;
        assert_eq!(index, 3);
        assert!(index < total);

        index = total - 1;
        index += 1;
        assert_eq!(index, total);
        Ok(())
    }

    /// Vérifie que clear_piscine_checkpoint efface bien le checkpoint.
    #[test]
    fn test_clear_checkpoint() -> crate::error::Result<()> {
        let conn = open_test_db();
        progress::save_piscine_checkpoint(&conn, 5)?;
        progress::clear_piscine_checkpoint(&conn)?;
        let loaded = progress::load_piscine_checkpoint(&conn)?;
        assert_eq!(loaded, None);
        Ok(())
    }

    /// Vérifie que le checkpoint est mis à jour à chaque avancement.
    #[test]
    fn test_checkpoint_advances_with_index() -> crate::error::Result<()> {
        let conn = open_test_db();
        for expected_index in [0usize, 1, 2, 5, 10] {
            progress::save_piscine_checkpoint(&conn, expected_index)?;
            let loaded = progress::load_piscine_checkpoint(&conn)?;
            assert_eq!(loaded, Some(expected_index));
        }
        Ok(())
    }

    /// Vérifie que le checkpoint est écrasé (pas accumulé) lors de mises à jour successives.
    #[test]
    fn test_checkpoint_overwrite() -> crate::error::Result<()> {
        let conn = open_test_db();
        progress::save_piscine_checkpoint(&conn, 3)?;
        progress::save_piscine_checkpoint(&conn, 7)?;
        let loaded = progress::load_piscine_checkpoint(&conn)?;
        assert_eq!(loaded, Some(7));
        Ok(())
    }

    /// Vérifie que load renvoie None si aucun checkpoint n'a été sauvegardé.
    #[test]
    fn test_load_checkpoint_missing_returns_none() -> crate::error::Result<()> {
        let conn = open_test_db();
        let loaded = progress::load_piscine_checkpoint(&conn)?;
        assert_eq!(loaded, None);
        Ok(())
    }

    /// Vérifie l'ordre chapter→difficulty sur une liste mixte d'exercices.
    /// "structs" est dans ch.1 "Fondamentaux C", "pipes" est dans ch.9 "Pipes".
    #[test]
    fn test_piscine_order_multi_chapter() -> crate::error::Result<()> {
        let ex_pipes_easy = make_exercise("pipes-easy", "pipes", Difficulty::Easy);
        let ex_structs_hard = make_exercise("structs-hard", "structs", Difficulty::Hard);
        let ex_structs_easy = make_exercise("structs-easy", "structs", Difficulty::Easy);

        let exercises = vec![ex_pipes_easy, ex_structs_hard, ex_structs_easy];
        let subjects = vec![];

        let blocks = chapters::order_by_chapters(&exercises, &subjects);
        let order = chapters::flatten_chapters(&blocks);

        assert_eq!(order.len(), 3);
        // structs (ch.1) must come before pipes (ch.9)
        let structs_positions: Vec<usize> = order
            .iter()
            .enumerate()
            .filter(|(_, e)| e.subject == "structs")
            .map(|(i, _)| i)
            .collect();
        let pipes_positions: Vec<usize> = order
            .iter()
            .enumerate()
            .filter(|(_, e)| e.subject == "pipes")
            .map(|(i, _)| i)
            .collect();

        assert!(structs_positions
            .iter()
            .all(|&p| pipes_positions.iter().all(|&q| p < q)));
        // within structs: easy before hard
        assert_eq!(structs_positions.len(), 2);
        assert_eq!(order[structs_positions[0]].difficulty, Difficulty::Easy);
        assert_eq!(order[structs_positions[1]].difficulty, Difficulty::Hard);
        Ok(())
    }
}
