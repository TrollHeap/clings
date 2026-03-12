//! Commandes run et review — exécution et renforcement d'exercices.

use std::io::Write;

use colored::Colorize;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use crate::error::{KfError, Result};
use crate::watcher::WatchAction;
use crate::{display, exercises, progress, runner, watcher};

pub fn cmd_run(exercise_id: &str) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| KfError::ExerciseNotFound(exercise_id.to_string()))?;

    display::show_exercise(exercise, 0, 1);

    let conn = progress::open_db()?;
    let subject_mastery =
        progress::get_subject(&conn, &exercise.subject)?.map(|s| s.mastery_score.get());
    let source_path = runner::write_starter_code(exercise, subject_mastery)?;

    display::show_edit_instructions(&source_path);
    display::show_keybinds_with_vis(!exercise.visualizer.steps.is_empty(), false, false);
    let exercise = exercise.clone();
    let source_for_change = source_path.clone();

    let mut vis_active = false;
    let mut vis_step: usize = 0;
    let mut vis_lines: usize = 0;

    let action = watcher::watch_file_interactive(
        &source_path,
        || {
            let result = runner::compile_and_run(&source_for_change, &exercise);
            display::show_result(&result, &exercise);

            if result.success {
                crate::record_and_show(&conn, &exercise.subject, &exercise.id, true);
                println!("  {}", "Exercice résolu !".bold().green());
                return WatchAction::Advance;
            }

            if !result.compile_error {
                crate::record_and_show(&conn, &exercise.subject, &exercise.id, false);
            }

            println!("  {}", "En attente de la prochaine sauvegarde...".dimmed());
            WatchAction::Continue
        },
        |key_event| {
            if key_event.kind != KeyEventKind::Press {
                return None;
            }

            if vis_active {
                match key_event.code {
                    KeyCode::Right => {
                        vis_step =
                            display::vis_step_forward(vis_step, exercise.visualizer.steps.len());
                        print!("\x1b[{}A\x1b[J", vis_lines);
                        let _ = std::io::stdout().flush(); // best-effort flush
                        vis_lines = display::show_visualizer(&exercise, vis_step);
                        return None;
                    }
                    KeyCode::Left => {
                        vis_step = display::vis_step_back(vis_step);
                        print!("\x1b[{}A\x1b[J", vis_lines);
                        let _ = std::io::stdout().flush(); // best-effort flush
                        vis_lines = display::show_visualizer(&exercise, vis_step);
                        return None;
                    }
                    _ => {
                        vis_active = false;
                        display::show_exercise(&exercise, 0, 1);
                        display::show_keybinds_with_vis(
                            !exercise.visualizer.steps.is_empty(),
                            false,
                            false,
                        );
                        return None;
                    }
                }
            }

            match key_event.code {
                KeyCode::Char('v') | KeyCode::Char('V')
                    if !exercise.visualizer.steps.is_empty() =>
                {
                    vis_step = 0;
                    vis_active = true;
                    vis_lines = display::show_visualizer(&exercise, vis_step);
                    None
                }
                KeyCode::Char('q') | KeyCode::Char('Q') => Some(WatchAction::Quit),
                KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(WatchAction::Quit)
                }
                KeyCode::Char('z') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(WatchAction::Quit)
                }
                _ => None,
            }
        },
    )?;

    if matches!(action, WatchAction::Advance) {
        println!("  {}", "Terminé !".bold().green());
    }

    Ok(())
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
