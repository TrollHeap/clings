//! Mode watch — progression SRS par chapitre.

use std::io::Write;

use colored::Colorize;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use crate::constants::{
    CONSECUTIVE_FAILURE_THRESHOLD, MSG_EXERCISE_SOLVED_ADVANCING, MSG_PRESS_KEY_RETURN,
    SECS_PER_DAY, SUCCESS_PAUSE_SECS,
};
use crate::error::Result;
use crate::watcher::WatchAction;
use crate::{chapters, display, exercises, progress, runner, tmux, watcher};

pub fn cmd_watch(filter_chapter: Option<u8>) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let mut conn = progress::open_db()?;

    progress::apply_all_decay(&mut conn)?;

    progress::ensure_subjects_batch(&mut conn, &all_exercises)?;

    let subjects = progress::get_all_subjects(&conn)?;

    // Filter out exercises above unlocked difficulty for each subject
    let subject_map: std::collections::HashMap<&str, i32> = subjects
        .iter()
        .map(|s| (s.name.as_str(), s.difficulty_unlocked))
        .collect();
    let mastery_map: std::collections::HashMap<&str, f64> = subjects
        .iter()
        .map(|s| (s.name.as_str(), s.mastery_score.get()))
        .collect();
    let gated_exercises: Vec<crate::models::Exercise> = all_exercises
        .iter()
        .filter(|ex| {
            let unlocked = subject_map.get(ex.subject.as_str()).copied().unwrap_or(1);
            (ex.difficulty as i32) <= unlocked
        })
        .cloned()
        .collect();

    let mut chapter_blocks = chapters::order_by_chapters(&gated_exercises, &subjects);
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

    // Enable raw mode for keyboard input if possible
    let _raw_guard = crate::enable_raw_mode();

    // Pre-compute next_review_days from already-loaded subjects to avoid N+1 DB queries.
    let now_ts = chrono::Utc::now().timestamp();
    let review_map: std::collections::HashMap<&str, Option<i64>> = subjects
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.next_review_at.map(|ts| (ts - now_ts) / SECS_PER_DAY),
            )
        })
        .collect();

    // Pre-build id→exercise map for O(1) prerequisite lookups inside the watch loop.
    let exercise_by_id: std::collections::HashMap<&str, &crate::models::Exercise> =
        all_exercises.iter().map(|e| (e.id.as_str(), e)).collect();

    let mut index = 0;
    while index < total {
        let exercise = exercise_order[index];

        // Select starter code stage based on subject mastery
        let (source_path, current_stage) = runner::prepare_exercise_source(&conn, exercise)?;

        // Display exercise
        let ch_ctx = chapters::chapter_context_at(&chapter_blocks, index);
        let next_review = review_map.get(exercise.subject.as_str()).copied().flatten();
        let unmet_prereqs: Vec<String> = exercise
            .prerequisites
            .iter()
            .filter_map(|pid| {
                let subj = exercise_by_id.get(pid.as_str())?.subject.as_str();
                let mastery = *mastery_map.get(subj).unwrap_or(&0.0);
                (mastery < 2.0).then(|| pid.clone())
            })
            .collect();
        let watch_meta = display::WatchMeta {
            stage: current_stage,
            next_review_days: next_review,
            unmet_prereqs,
        };
        display::show_exercise_watch(
            exercise,
            index,
            total,
            &completed,
            Some(&ch_ctx),
            &watch_meta,
        );
        display::show_watching(&source_path);
        display::show_keybinds_with_vis(!exercise.visualizer.steps.is_empty(), false, true);

        // Open/update neovim pane in tmux
        editor_pane = tmux::update_editor_pane(editor_pane.as_deref(), &source_path);

        let source_for_change = source_path.clone();
        let mut hint_shown = false;
        let mut vis_active = false;
        let mut vis_step: usize = 0;
        let mut vis_lines: usize = 0;
        let mut already_recorded = false;
        let mut consecutive_failures: u32 = 0;

        let action = watcher::watch_file_interactive(
            &source_path,
            // On file change: notify only, no auto-compile
            || {
                display::show_file_saved();
                display::show_keybinds_with_vis(!exercise.visualizer.steps.is_empty(), false, true);
                WatchAction::Continue
            },
            // On keyboard input
            |key_event| {
                if key_event.kind != KeyEventKind::Press {
                    return None;
                }

                // Visualizer navigation (arrow keys) or close on any other key
                if vis_active {
                    match key_event.code {
                        KeyCode::Right => {
                            vis_step = display::vis_step_forward(
                                vis_step,
                                exercise.visualizer.steps.len(),
                            );
                            print!("\x1b[{}A\x1b[J", vis_lines);
                            let _ = std::io::stdout().flush(); // best-effort flush
                            vis_lines = display::show_visualizer(exercise, vis_step);
                            return None;
                        }
                        KeyCode::Left => {
                            vis_step = display::vis_step_back(vis_step);
                            print!("\x1b[{}A\x1b[J", vis_lines);
                            let _ = std::io::stdout().flush(); // best-effort flush
                            vis_lines = display::show_visualizer(exercise, vis_step);
                            return None;
                        }
                        _ => {
                            vis_active = false;
                            display::show_exercise_watch(
                                exercise,
                                index,
                                total,
                                &completed,
                                None,
                                &watch_meta,
                            );
                            display::show_keybinds_with_vis(
                                !exercise.visualizer.steps.is_empty(),
                                false,
                                true,
                            );
                            return None;
                        }
                    }
                }

                match key_event.code {
                    KeyCode::Char('v') | KeyCode::Char('V') => {
                        if !exercise.visualizer.steps.is_empty() {
                            vis_step = 0;
                            vis_active = true;
                            vis_lines = display::show_visualizer(exercise, vis_step);
                        }
                        None
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        if !hint_shown {
                            println!();
                            display::show_hints(exercise);
                            hint_shown = true;
                        }
                        None
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => Some(WatchAction::Skip),
                    KeyCode::Char('j') | KeyCode::Char('J') => Some(WatchAction::Next),
                    KeyCode::Char('k') | KeyCode::Char('K') => Some(WatchAction::Prev),
                    KeyCode::Char('q') | KeyCode::Char('Q') => Some(WatchAction::Quit),
                    KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(WatchAction::Quit)
                    }
                    KeyCode::Char('z') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(WatchAction::Quit)
                    }
                    KeyCode::Char('l') | KeyCode::Char('L') => {
                        // Quick exercise list — reuse subjects already loaded at session start
                        display::show_exercise_list(&all_exercises, &subjects, None, None);
                        println!("  {}", MSG_PRESS_KEY_RETURN.dimmed());
                        None
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        // Explicit run: compile and check now
                        let result = runner::compile_and_run(&source_for_change, exercise);
                        display::show_result(&result, exercise);
                        if result.success {
                            consecutive_failures = 0;
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
                        consecutive_failures += 1;
                        if consecutive_failures >= CONSECUTIVE_FAILURE_THRESHOLD as u32
                            && !exercise.hints.is_empty()
                        {
                            println!();
                            println!(
                                "  {}",
                                "Indice automatique après 3 tentatives :".dimmed().yellow()
                            );
                            display::show_hints(exercise);
                        }
                        display::show_keybinds_with_vis(
                            !exercise.visualizer.steps.is_empty(),
                            false,
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
                index += 1;
            }
            WatchAction::Skip | WatchAction::Next => {
                if index + 1 < total {
                    index += 1;
                }
            }
            WatchAction::Prev => {
                index = index.saturating_sub(1);
            }
            WatchAction::Quit => {
                break;
            }
            WatchAction::Continue => {}
        }
    }

    // Cleanup (raw mode restored automatically by _raw_guard drop)
    drop(_raw_guard);
    if let Some(pane) = &editor_pane {
        tmux::kill_pane(pane);
    }

    let done = completed.iter().filter(|&&c| c).count();
    if done == total {
        println!(
            "\n  {} Tous les exercices complétés ! Lancez `clings progress` pour voir vos stats.",
            "Félicitations !".bold().green()
        );
    } else {
        println!(
            "\n  {} {}/{} exercices complétés. Lancez `clings watch` pour continuer.",
            "Session terminée.".bold(),
            done,
            total
        );
    }

    Ok(())
}
