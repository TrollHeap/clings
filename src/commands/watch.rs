//! Mode watch — progression SRS par chapitre.
//!
//! v3.0 : architecture TEA Ratatui. Logique métier inchangée.

use colored::Colorize;

use crate::constants::SECS_PER_DAY;
use crate::error::Result;
use crate::tui::app::{App, AppMode};
use crate::{chapters, exercises, progress, tmux};

pub fn cmd_watch(filter_chapter: Option<u8>) -> Result<()> {
    // ── 1. Chargement exercices + données SRS ──────────────────────────
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let mut conn = progress::open_db()?;

    progress::apply_all_decay(&mut conn)?;
    progress::ensure_subjects_batch(&mut conn, &all_exercises)?;

    let subjects = progress::get_all_subjects(&conn)?;

    // Gate par difficulté déverrouillée
    let subject_map: std::collections::HashMap<&str, i32> = subjects
        .iter()
        .map(|s| (s.name.as_str(), s.difficulty_unlocked))
        .collect();
    let mastery_map: std::collections::HashMap<String, f64> = subjects
        .iter()
        .map(|s| (s.name.clone(), s.mastery_score.get()))
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

    // SRS review map
    let now_ts = chrono::Utc::now().timestamp();
    let review_map: std::collections::HashMap<String, Option<i64>> = subjects
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                s.next_review_at.map(|ts| (ts - now_ts) / SECS_PER_DAY),
            )
        })
        .collect();

    // ── 2. Prépare AppState ────────────────────────────────────────────
    let mut app = App::new(AppMode::Watch {
        chapter: filter_chapter,
    });
    app.state.exercises = exercise_order.into_iter().cloned().collect();
    app.state.completed = vec![false; total];
    app.state.review_map = review_map;
    app.state.mastery_map = mastery_map;

    // Build subject_order cache (unique subjects in appearance order)
    let mut seen = std::collections::HashSet::new();
    app.state.subject_order = app
        .state
        .exercises
        .iter()
        .filter_map(|ex| {
            if seen.insert(ex.subject.clone()) {
                Some(ex.subject.clone())
            } else {
                None
            }
        })
        .collect();

    // ── 3. Ratatui init ────────────────────────────────────────────────
    let mut terminal = ratatui::init();

    let result = app.run_watch(&mut terminal, &conn);

    ratatui::restore();

    // ── 4. Cleanup tmux ────────────────────────────────────────────────
    if let Some(pane) = &app.state.editor_pane {
        tmux::kill_pane(pane);
    }

    result?;

    // ── 5. Summary post-session ────────────────────────────────────────
    let done = app.state.completed.iter().filter(|&&c| c).count();
    if done == total && total > 0 {
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
