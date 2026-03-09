use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use colored::Colorize;

use crate::chapters;
use crate::error::Result;
use crate::models::ValidationMode;
use crate::watcher::WatchAction;
use crate::{display, exercises, progress, runner, tmux};

/// Run piscine mode: linear progression through ALL exercises, ignoring difficulty gating.
/// Exercises are ordered: chapter 1 D1→D2→D3→D4→D5, then chapter 2, etc.
pub fn cmd_piscine() -> Result<()> {
    crate::install_ctrlc_handler();

    let (all_exercises, _) = exercises::load_all_exercises()?;
    let mut conn = progress::open_db()?;

    progress::ensure_subjects_batch(&mut conn, &all_exercises)?;

    let subjects = progress::get_all_subjects(&conn)?;

    // Order by chapters, then difficulty within each chapter (no gating)
    let chapter_blocks = chapters::order_by_chapters(&all_exercises, &subjects);
    let exercise_order = chapters::flatten_chapters(&chapter_blocks);

    if exercise_order.is_empty() {
        println!("{}", "  Aucun exercice disponible.".dimmed());
        return Ok(());
    }

    let total = exercise_order.len();
    let mut completed = vec![false; total];
    let mut editor_pane: Option<String> = None;
    let start_time = Instant::now();

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
        let exercise = exercise_order[index];

        if matches!(exercise.validation.mode, ValidationMode::Test) {
            println!(
                "  {} Exercice {} ignoré (validation Test non supportée en CLI)",
                "⚠".yellow(),
                exercise.id
            );
            completed[index] = true;
            index += 1;
            continue;
        }

        let subject_mastery =
            progress::get_subject(&conn, &exercise.subject)?.map(|s| s.mastery_score);
        let current_stage = subject_mastery.map(runner::mastery_to_stage);
        let source_path = runner::write_starter_code(exercise, subject_mastery)?;

        // Piscine display
        display::clear_screen();
        show_piscine_header(index, total, &start_time);

        let ch_ctx = chapters::chapter_context_at(&chapter_blocks, index);
        display::show_chapter(&ch_ctx);
        println!();

        println!(
            "  {} [{}/{}]  {}",
            "Exercise".bold().green(),
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
                None => "S2".to_string(),
            }
            .dimmed(),
        );
        println!("  {}", "─".repeat(56).dimmed());
        println!();

        for line in exercise.description.lines() {
            println!("  {line}");
        }
        println!();
        display::show_watching(&source_path);
        display::show_keybinds();

        editor_pane = tmux::update_editor_pane(editor_pane.as_deref(), &source_path);

        let exercise_clone = exercise.clone();
        let conn_for_watch = progress::open_db()?;
        let source_for_change = source_path.clone();
        let mut hint_shown = false;
        let already_recorded = Arc::new(AtomicBool::new(false));
        let already_recorded_key = Arc::clone(&already_recorded);

        let action = crate::watcher::watch_file_interactive(
            &source_path,
            || {
                let result = runner::compile_and_run(&source_for_change, &exercise_clone);
                display::show_result(&result, &exercise_clone);

                if result.success && !already_recorded.swap(true, Ordering::SeqCst) {
                    crate::record_and_show(
                        &conn_for_watch,
                        &exercise_clone.subject,
                        &exercise_clone.id,
                        true,
                    );
                    println!(
                        "  {}",
                        "Exercice résolu ! Avancement dans 2s...".bold().green()
                    );
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    return WatchAction::Advance;
                } else if result.success {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    return WatchAction::Advance;
                }

                if !result.compile_error {
                    crate::record_and_show(
                        &conn_for_watch,
                        &exercise_clone.subject,
                        &exercise_clone.id,
                        false,
                    );
                }

                display::show_keybinds();
                WatchAction::Continue
            },
            |key| match key {
                b'h' | b'H' => {
                    if !hint_shown {
                        println!();
                        display::show_hints(&exercise_clone);
                        hint_shown = true;
                    }
                    None
                }
                b'n' | b'N' => Some(WatchAction::Skip),
                b'q' | b'Q' => Some(WatchAction::Quit),
                b'c' | b'C' => {
                    let result = runner::compile_and_run(&source_for_change, &exercise_clone);
                    display::show_result(&result, &exercise_clone);
                    if result.success {
                        if !already_recorded_key.swap(true, Ordering::SeqCst) {
                            crate::record_and_show(
                                &conn_for_watch,
                                &exercise_clone.subject,
                                &exercise_clone.id,
                                true,
                            );
                        }
                        println!("  {}", "Exercise solved! Advancing...".bold().green());
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        return Some(WatchAction::Advance);
                    }
                    display::show_keybinds();
                    None
                }
                _ => None,
            },
        )?;

        match action {
            WatchAction::Advance => {
                completed[index] = true;
                index += 1;
                progress::save_piscine_checkpoint(&conn, index).ok();
            }
            WatchAction::Skip => {
                index += 1;
                progress::save_piscine_checkpoint(&conn, index).ok();
            }
            WatchAction::Quit => {
                progress::save_piscine_checkpoint(&conn, index).ok();
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

    if done == total {
        progress::clear_piscine_checkpoint(&conn).ok();
    }

    println!();
    if done == total {
        println!(
            "  {} Piscine complétée ! {}/{} en {}h{:02}m",
            "BRAVO !".bold().green(),
            done,
            total,
            hours,
            mins
        );
    } else {
        println!(
            "  {} {}/{} exercices complétés en {}h{:02}m. `kf piscine` pour reprendre.",
            "Session piscine terminée.".bold(),
            done,
            total,
            hours,
            mins
        );
    }

    Ok(())
}

fn show_piscine_header(current: usize, total: usize, start: &Instant) {
    let elapsed = start.elapsed();
    let mins = elapsed.as_secs() / 60;
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
        "  {} {}/{}  ({}%)   {} {}min",
        "Progression:".bold(),
        current,
        total,
        pct,
        "⏱".dimmed(),
        mins,
    );
    println!();
}
