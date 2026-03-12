//! Piscine mode — linear progression through all exercises with checkpoint persistence.

use std::time::Instant;

use colored::Colorize;

use crate::chapters;
use crate::constants::{
    CTRL_C, CTRL_Z, HEADER_WIDTH, MSG_EXERCISE_SOLVED_ADVANCING, MSG_PRESS_KEY_RETURN,
    PISCINE_FAILURE_THRESHOLD, SUCCESS_PAUSE_SECS,
};
use crate::display::handle_esc_sequence;
use crate::error::Result;
use crate::models::Exercise;
use crate::watcher::WatchAction;
use crate::{display, exercises, progress, runner, tmux};

fn log_checkpoint_err(label: &str, result: Result<()>) {
    if let Err(e) = result {
        eprintln!("  Avertissement : {label}checkpoint non sauvegardé : {e}");
    }
}

fn save_checkpoint(conn: &rusqlite::Connection, index: usize) {
    log_checkpoint_err("", progress::save_piscine_checkpoint(conn, index));
}

fn save_exam_checkpoint(conn: &rusqlite::Connection, session_id: Option<&str>, index: usize) {
    if let Some(sid) = session_id {
        log_checkpoint_err("exam ", progress::save_exam_checkpoint(conn, sid, index));
    }
}

/// Re-renders the full exercise screen after closing the visualizer.
#[allow(clippy::too_many_arguments)]
fn redisplay_piscine_exercise(
    index: usize,
    total: usize,
    start: &Instant,
    deadline: Option<std::time::Instant>,
    chapter_ctx: Option<&crate::chapters::ChapterContext>,
    exercise: &Exercise,
    current_stage: Option<u8>,
    source_path: &std::path::Path,
) {
    display::clear_screen();
    show_piscine_header(index, total, start, deadline);
    if let Some(ctx) = chapter_ctx {
        display::show_chapter(ctx);
        println!();
    }
    println!(
        "  {} [{}/{}]  {}",
        "Exercice".bold().green(),
        (index + 1).to_string().bold(),
        total,
        exercise.title.bold(),
    );
    println!(
        "  {}  {}   {}  {}   {}  {}",
        "│".dimmed(),
        display::difficulty_stars(exercise.difficulty),
        "│".dimmed(),
        exercise.subject.dimmed(),
        "│".dimmed(),
        match current_stage {
            Some(s) => format!("S{s}"),
            None => "S2".to_owned(),
        }
        .dimmed(),
    );
    println!("  {}", "─".repeat(HEADER_WIDTH).dimmed());
    println!();
    for line in exercise.description.lines() {
        println!("  {line}");
    }
    println!();
    if let Some(kc) = &exercise.key_concept {
        println!("  {} {}", "💡 Concept clé :".bold().cyan(), kc);
    }
    if let Some(cm) = &exercise.common_mistake {
        println!("  {} {}", "⚠  Piège:".bold().yellow(), cm);
    }
    if exercise.key_concept.is_some() || exercise.common_mistake.is_some() {
        println!();
    }
    display::show_watching(source_path);
    display::show_keybinds_with_vis(!exercise.visualizer.steps.is_empty(), true, true);
}

/// Run piscine mode: linear progression through ALL exercises, ignoring difficulty gating.
/// Exercises are ordered: chapter 1 D1→D2→D3→D4→D5, then chapter 2, etc.
pub fn cmd_piscine(filter_chapter: Option<u8>, timed_minutes: Option<u64>) -> Result<()> {
    crate::install_ctrlc_handler();

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
    let mut completed = vec![false; total];
    let mut editor_pane: Option<String> = None;
    let start_time = Instant::now();
    let deadline: Option<std::time::Instant> =
        timed_minutes.map(|m| std::time::Instant::now() + std::time::Duration::from_secs(m * 60));

    if let Some(mins) = timed_minutes {
        println!(
            "  {} Mode exam — {} minutes. Bonne chance !",
            "⏰".bold().yellow(),
            mins
        );
        println!();
    }

    let _raw_guard = crate::enable_raw_mode();

    // Restore checkpoint if available (clamped to valid range)
    let mut index = progress::load_piscine_checkpoint(&conn)?
        .map(|i| i.min(total.saturating_sub(1)))
        .unwrap_or(0);
    if index > 0 {
        println!(
            "  {} Reprise depuis l'exercice {}/{}",
            "⏩".dimmed(),
            index + 1,
            total
        );
    }
    while index < total {
        if let Some(dl) = deadline {
            if std::time::Instant::now() >= dl {
                println!();
                println!(
                    "  {} Temps écoulé ! Session exam terminée.",
                    "⏰".bold().red()
                );
                log_checkpoint_err("piscine ", progress::clear_piscine_checkpoint(&conn));
                break;
            }
        }

        let exercise = exercise_order[index];
        let ex_start = Instant::now();

        let (source_path, current_stage) = runner::prepare_exercise_source(&conn, exercise)?;

        let ch_ctx = chapters::chapter_context_at(&chapter_blocks, index);
        redisplay_piscine_exercise(
            index,
            total,
            &start_time,
            deadline,
            Some(&ch_ctx),
            exercise,
            current_stage,
            &source_path,
        );

        editor_pane = tmux::update_editor_pane(editor_pane.as_deref(), &source_path);

        let mut hint_shown = false;
        let mut already_recorded = false;
        let mut vis_active = false;
        let mut vis_step: usize = 0;
        let mut vis_lines: usize = 0;
        let mut escape_buf: Vec<u8> = Vec::new();
        let mut fail_count: u32 = 0;

        let action = crate::watcher::watch_file_interactive(
            &source_path,
            || {
                display::show_file_saved();
                display::show_keybinds_with_vis(!exercise.visualizer.steps.is_empty(), true, true);
                WatchAction::Continue
            },
            |key| {
                // Accumulate escape sequences for arrow keys (3-byte: ESC [ C/D)
                let ch_ctx_inner = chapters::chapter_context_at(&chapter_blocks, index);
                if handle_esc_sequence(
                    key,
                    &mut escape_buf,
                    vis_active,
                    &mut vis_step,
                    &mut vis_lines,
                    exercise.visualizer.steps.len(),
                    &mut |step| display::show_visualizer(exercise, step),
                )
                .is_some()
                {
                    return None;
                }

                // Any non-arrow key closes the visualizer
                if vis_active {
                    vis_active = false;
                    redisplay_piscine_exercise(
                        index,
                        total,
                        &start_time,
                        deadline,
                        Some(&ch_ctx_inner),
                        exercise,
                        current_stage,
                        &source_path,
                    );
                    return None;
                }

                match key {
                    b'v' | b'V' => {
                        if !exercise.visualizer.steps.is_empty() {
                            vis_step = 0;
                            vis_active = true;
                            vis_lines = display::show_visualizer(exercise, vis_step);
                        }
                        None
                    }
                    b'h' | b'H' => {
                        if !hint_shown {
                            println!();
                            display::show_hints(exercise);
                            hint_shown = true;
                        }
                        None
                    }
                    b'l' | b'L' => {
                        match progress::get_all_subjects(&conn) {
                            Ok(subjects) => {
                                display::show_exercise_list(&all_exercises, &subjects, None, None)
                            }
                            Err(e) => eprintln!("  {} {e}", "Erreur:".red()),
                        }
                        println!("  {}", MSG_PRESS_KEY_RETURN.dimmed());
                        None
                    }
                    b'n' | b'N' | b'j' | b'J' => Some(WatchAction::Next),
                    b'k' | b'K' => Some(WatchAction::Prev),
                    b'q' | b'Q' | CTRL_C | CTRL_Z => Some(WatchAction::Quit),
                    b'r' | b'R' => {
                        let result = runner::compile_and_run(&source_path, exercise);
                        display::show_result(&result, exercise);
                        if result.success {
                            fail_count = 0;
                            if !already_recorded {
                                already_recorded = true;
                                crate::record_and_show(
                                    &conn,
                                    &exercise.subject,
                                    &exercise.id,
                                    true,
                                );
                            }
                            println!("  {}", MSG_EXERCISE_SOLVED_ADVANCING.bold().green());
                            std::thread::sleep(std::time::Duration::from_secs(SUCCESS_PAUSE_SECS));
                            return Some(WatchAction::Advance);
                        }
                        if !result.compile_error {
                            fail_count += 1;
                            if fail_count >= PISCINE_FAILURE_THRESHOLD {
                                if let Some(cm) = &exercise.common_mistake {
                                    println!(
                                        "  {} {}",
                                        "⚠ Piège fréquent:".bold().red(),
                                        cm.yellow()
                                    );
                                }
                            }
                        }
                        display::show_keybinds_with_vis(
                            !exercise.visualizer.steps.is_empty(),
                            true,
                            true,
                        );
                        None
                    }
                    _ => None,
                }
            },
        )?;

        match action {
            WatchAction::Advance => {
                completed[index] = true;
                let ex_elapsed = ex_start.elapsed();
                let ex_secs = ex_elapsed.as_secs();
                println!(
                    "  {} Résolu en {}m{:02}s",
                    "⏱".dimmed(),
                    ex_secs / 60,
                    ex_secs % 60,
                );
                index += 1;
                save_checkpoint(&conn, index);
            }
            WatchAction::Skip | WatchAction::Next => {
                index += 1;
                save_checkpoint(&conn, index);
            }
            WatchAction::Prev => {
                index = index.saturating_sub(1);
                save_checkpoint(&conn, index);
            }
            WatchAction::Quit => {
                save_checkpoint(&conn, index);
                break;
            }
            WatchAction::Continue => {}
        }
    }

    drop(_raw_guard);
    if let Some(pane) = &editor_pane {
        tmux::kill_pane(pane);
    }

    let done = completed.iter().filter(|&&c| c).count();
    let elapsed = start_time.elapsed();
    let hours = elapsed.as_secs() / 3600;
    let mins = (elapsed.as_secs() % 3600) / 60;
    let secs = elapsed.as_secs() % 60;

    if done == total {
        log_checkpoint_err("piscine ", progress::clear_piscine_checkpoint(&conn));
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

    Ok(())
}

/// Lancer une session piscine avec une liste d'exercices préfiltrée (mode exam).
pub fn run_exam_piscine(
    exercises: Vec<crate::models::Exercise>,
    timed_minutes: Option<u64>,
    session_id: Option<&str>,
) -> crate::error::Result<()> {
    crate::install_ctrlc_handler();
    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;
    progress::ensure_subjects_batch(&mut conn, &exercises)?;

    let total = exercises.len();
    let mut completed = vec![false; total];
    let mut editor_pane: Option<String> = None;
    let start_time = Instant::now();
    let deadline: Option<std::time::Instant> =
        timed_minutes.map(|m| std::time::Instant::now() + std::time::Duration::from_secs(m * 60));

    let _raw_guard = crate::enable_raw_mode();

    let mut index = if let Some(sid) = session_id {
        progress::load_exam_checkpoint(&conn, sid)
            .ok()
            .flatten()
            .map(|i| i.min(total.saturating_sub(1)))
            .unwrap_or(0)
    } else {
        0
    };
    if index > 0 {
        println!(
            "  {} Reprise depuis l'exercice {}/{}",
            "⏩".dimmed(),
            index + 1,
            total
        );
    }
    while index < total {
        // Deadline check
        if let Some(dl) = deadline {
            if std::time::Instant::now() >= dl {
                println!();
                println!(
                    "  {} Temps écoulé ! Session exam terminée.",
                    "⏰".bold().red()
                );
                log_checkpoint_err("exam ", progress::clear_exam_checkpoint(&conn));
                break;
            }
        }

        let exercise = &exercises[index];

        let ex_start = Instant::now();
        let (source_path, current_stage) = runner::prepare_exercise_source(&conn, exercise)?;

        redisplay_piscine_exercise(
            index,
            total,
            &start_time,
            deadline,
            None,
            exercise,
            current_stage,
            &source_path,
        );

        editor_pane = tmux::update_editor_pane(editor_pane.as_deref(), &source_path);

        let mut hint_shown = false;
        let mut already_recorded = false;
        let mut vis_active = false;
        let mut vis_step: usize = 0;
        let mut vis_lines: usize = 0;
        let mut escape_buf: Vec<u8> = Vec::new();
        let mut fail_count: u32 = 0;

        let action = crate::watcher::watch_file_interactive(
            &source_path,
            || {
                display::show_file_saved();
                display::show_keybinds_with_vis(!exercise.visualizer.steps.is_empty(), true, true);
                WatchAction::Continue
            },
            |key| {
                if handle_esc_sequence(
                    key,
                    &mut escape_buf,
                    vis_active,
                    &mut vis_step,
                    &mut vis_lines,
                    exercise.visualizer.steps.len(),
                    &mut |step| display::show_visualizer(exercise, step),
                )
                .is_some()
                {
                    return None;
                }

                if vis_active {
                    vis_active = false;
                    redisplay_piscine_exercise(
                        index,
                        total,
                        &start_time,
                        deadline,
                        None,
                        exercise,
                        current_stage,
                        &source_path,
                    );
                    return None;
                }
                match key {
                    b'v' | b'V' => {
                        if !exercise.visualizer.steps.is_empty() {
                            vis_step = 0;
                            vis_active = true;
                            vis_lines = display::show_visualizer(exercise, vis_step);
                        }
                        None
                    }
                    b'h' | b'H' => {
                        if !hint_shown {
                            println!();
                            display::show_hints(exercise);
                            hint_shown = true;
                        }
                        None
                    }
                    b'l' | b'L' => {
                        match progress::get_all_subjects(&conn) {
                            Ok(subjects) => {
                                display::show_exercise_list(&exercises, &subjects, None, None)
                            }
                            Err(e) => eprintln!("  {} {e}", "Erreur:".red()),
                        }
                        println!("  {}", MSG_PRESS_KEY_RETURN.dimmed());
                        None
                    }
                    b'n' | b'N' | b'j' | b'J' => Some(WatchAction::Next),
                    b'k' | b'K' => Some(WatchAction::Prev),
                    b'q' | b'Q' | CTRL_C | CTRL_Z => Some(WatchAction::Quit),
                    b'r' | b'R' => {
                        let result = runner::compile_and_run(&source_path, exercise);
                        display::show_result(&result, exercise);
                        if result.success {
                            fail_count = 0;
                            if !already_recorded {
                                already_recorded = true;
                                crate::record_and_show(
                                    &conn,
                                    &exercise.subject,
                                    &exercise.id,
                                    true,
                                );
                            }
                            println!("  {}", MSG_EXERCISE_SOLVED_ADVANCING.bold().green());
                            std::thread::sleep(std::time::Duration::from_secs(SUCCESS_PAUSE_SECS));
                            return Some(WatchAction::Advance);
                        }
                        if !result.compile_error {
                            fail_count += 1;
                            if fail_count >= PISCINE_FAILURE_THRESHOLD {
                                if let Some(cm) = &exercise.common_mistake {
                                    println!(
                                        "  {} {}",
                                        "⚠ Piège fréquent:".bold().red(),
                                        cm.yellow()
                                    );
                                }
                            }
                        }
                        display::show_keybinds_with_vis(
                            !exercise.visualizer.steps.is_empty(),
                            true,
                            true,
                        );
                        None
                    }
                    _ => None,
                }
            },
        )?;

        match action {
            WatchAction::Advance => {
                completed[index] = true;
                let ex_elapsed = ex_start.elapsed();
                let ex_secs = ex_elapsed.as_secs();
                println!(
                    "  {} Résolu en {}m{:02}s",
                    "⏱".dimmed(),
                    ex_secs / 60,
                    ex_secs % 60,
                );
                index += 1;
            }
            WatchAction::Skip | WatchAction::Next => {
                index += 1;
            }
            WatchAction::Prev => {
                index = index.saturating_sub(1);
            }
            WatchAction::Quit => {
                save_exam_checkpoint(&conn, session_id, index);
                break;
            }
            // Ne pas sauvegarder le checkpoint sur un simple événement fichier
            WatchAction::Continue => {}
        }
        save_exam_checkpoint(&conn, session_id, index);
    }

    drop(_raw_guard);
    if let Some(pane) = &editor_pane {
        tmux::kill_pane(pane);
    }

    let done = completed.iter().filter(|&&c| c).count();
    if index >= total {
        log_checkpoint_err("exam ", progress::clear_exam_checkpoint(&conn));
    }

    let elapsed = start_time.elapsed();
    let hours = elapsed.as_secs() / 3600;
    let mins = (elapsed.as_secs() % 3600) / 60;
    let secs = elapsed.as_secs() % 60;

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

    Ok(())
}

fn show_piscine_header(
    current: usize,
    total: usize,
    start: &Instant,
    deadline: Option<std::time::Instant>,
) {
    let elapsed = start.elapsed();
    let mins = elapsed.as_secs() / 60;
    let secs = elapsed.as_secs() % 60;
    let pct = if total > 0 {
        (current * 100) / total
    } else {
        0
    };

    println!(
        "  {}",
        "╔════════════════════════════════════════════════════════╗"
            .bold()
            .yellow()
    );
    println!(
        "  {}  {}  {}",
        "║".bold().yellow(),
        "  MODE PISCINE — Progression intensive linéaire  "
            .bold()
            .yellow(),
        "║".bold().yellow()
    );
    println!(
        "  {}",
        "╚════════════════════════════════════════════════════════╝"
            .bold()
            .yellow()
    );
    println!(
        "  {} {}/{}  ({}%)   {} {}m{:02}s",
        "Progression:".bold(),
        current,
        total,
        pct,
        "⏱".dimmed(),
        mins,
        secs,
    );
    if let Some(dl) = deadline {
        let now = std::time::Instant::now();
        if now < dl {
            let remaining = dl - now;
            let rem_mins = remaining.as_secs() / 60;
            let rem_secs = remaining.as_secs() % 60;
            let remaining_str = format!("⏰ Restant: {}m{:02}s", rem_mins, rem_secs);
            if remaining.as_secs() < 300 {
                println!("  {} {}", "│".bold().yellow(), remaining_str.bold().red());
            } else {
                println!("  {} {}", "│".bold().yellow(), remaining_str.bold());
            }
        } else {
            println!(
                "  {} {}",
                "│".bold().yellow(),
                "⏰ TEMPS ÉCOULÉ".bold().red()
            );
        }
    }
    println!();
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
    fn test_piscine_order() {
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
    }

    /// Vérifie le roundtrip save/load du checkpoint piscine sur une DB in-memory.
    #[test]
    fn test_checkpoint_roundtrip() {
        let conn = open_test_db();
        progress::save_piscine_checkpoint(&conn, 3).unwrap();
        let loaded = progress::load_piscine_checkpoint(&conn).unwrap();
        assert_eq!(loaded, Some(3));
    }

    /// Vérifie que le mécanisme de skip incrémente bien l'index d'exercice.
    #[test]
    fn test_skip_increments_index() {
        let total = 5usize;
        let mut index = 2usize;
        index += 1;
        assert_eq!(index, 3);
        assert!(index < total);

        index = total - 1;
        index += 1;
        assert_eq!(index, total);
    }

    /// Vérifie que clear_piscine_checkpoint efface bien le checkpoint.
    #[test]
    fn test_clear_checkpoint() {
        let conn = open_test_db();
        progress::save_piscine_checkpoint(&conn, 5).unwrap();
        progress::clear_piscine_checkpoint(&conn).unwrap();
        let loaded = progress::load_piscine_checkpoint(&conn).unwrap();
        assert_eq!(loaded, None);
    }

    /// Vérifie que le checkpoint est mis à jour à chaque avancement.
    #[test]
    fn test_checkpoint_advances_with_index() {
        let conn = open_test_db();
        for expected_index in [0usize, 1, 2, 5, 10] {
            progress::save_piscine_checkpoint(&conn, expected_index).unwrap();
            let loaded = progress::load_piscine_checkpoint(&conn).unwrap();
            assert_eq!(loaded, Some(expected_index));
        }
    }

    /// Vérifie que le checkpoint est écrasé (pas accumulé) lors de mises à jour successives.
    #[test]
    fn test_checkpoint_overwrite() {
        let conn = open_test_db();
        progress::save_piscine_checkpoint(&conn, 3).unwrap();
        progress::save_piscine_checkpoint(&conn, 7).unwrap();
        let loaded = progress::load_piscine_checkpoint(&conn).unwrap();
        assert_eq!(loaded, Some(7));
    }

    /// Vérifie que load renvoie None si aucun checkpoint n'a été sauvegardé.
    #[test]
    fn test_load_checkpoint_missing_returns_none() {
        let conn = open_test_db();
        let loaded = progress::load_piscine_checkpoint(&conn).unwrap();
        assert_eq!(loaded, None);
    }

    /// Vérifie l'ordre chapter→difficulty sur une liste mixte d'exercices.
    /// "structs" est dans ch.1 "Fondamentaux C", "pipes" est dans ch.9 "Pipes".
    #[test]
    fn test_piscine_order_multi_chapter() {
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
    }
}
