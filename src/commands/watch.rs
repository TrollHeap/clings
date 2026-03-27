//! Mode watch — progression SRS par chapitre.
//!
//! v3.0 : architecture TEA Ratatui. Logique métier inchangée.

use colored::Colorize;

use crate::constants::{clings_data_dir, SECS_PER_DAY};
use crate::error::Result;
use crate::tui::app::App;
use crate::{chapters, config, exercises, progress, sync, tmux};

/// Subjects exclusive to UTC502 — excluded from NSY103-only mode.
const NSY103_EXCLUDED: &[&str] = &["scheduling", "virtual_memory"];

/// Start watch mode — interactive SRS-based exercise progression by chapter.
/// Launches Ratatui TUI. Optionally filters by chapter; enables file-watching auto-compile and SRS decay.
/// Calls sync pull/push if configured. Saves session state for "Continue" in launcher.
/// If `nsy103_only` is true, filters out UTC502-specific subjects (scheduling, virtual_memory).
pub fn cmd_watch(filter_chapter: Option<u8>, nsy103_only: bool) -> Result<()> {
    // ── 0. Sync pull (si activé) ───────────────────────────────────────
    let clings_dir = clings_data_dir();
    let sync_cfg = config::get().sync.clone();

    // ── 1. Chargement exercices + données SRS ──────────────────────────
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let mut conn = progress::open_db()?;

    if sync_cfg.enabled {
        match sync::pull_and_merge(&clings_dir, &mut conn) {
            Ok(Some(n)) => println!("  {} {n} sujet(s) mis à jour depuis le remote.", "↪".bold()),
            Ok(None) => {}
            Err(e) => eprintln!("  {} sync pull: {e} (mode hors-ligne)", "⚠".yellow()),
        }
    }

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
    // Minimum difficulty per subject — ensures subjects with no D1 exercises
    // (e.g. capstones starting at D3) are never fully locked out.
    let min_difficulty_per_subject: std::collections::HashMap<&str, i32> = all_exercises
        .iter()
        .fold(std::collections::HashMap::new(), |mut acc, ex| {
            let entry = acc.entry(ex.subject.as_str()).or_insert(5);
            *entry = (*entry).min(ex.difficulty as i32);
            acc
        });
    let gated_exercises: Vec<crate::models::Exercise> = all_exercises
        .iter()
        .filter(|ex| {
            let unlocked = subject_map.get(ex.subject.as_str()).copied().unwrap_or(1);
            let min_d = min_difficulty_per_subject
                .get(ex.subject.as_str())
                .copied()
                .unwrap_or(1);
            (ex.difficulty as i32) <= unlocked.max(min_d)
        })
        .cloned()
        .collect();

    let gated_exercises: Vec<crate::models::Exercise> = if nsy103_only {
        gated_exercises
            .into_iter()
            .filter(|ex| !NSY103_EXCLUDED.contains(&ex.subject.as_str()))
            .collect()
    } else {
        gated_exercises
    };

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
    let mut app = App::new();
    app.state.exercises = exercise_order.into_iter().cloned().collect();
    app.state.completed = vec![false; total];
    app.state.review_map = review_map;
    app.state.mastery_map = mastery_map;

    // Build subject_order cache (unique subjects in appearance order)
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    app.state.subject_order = app
        .state
        .exercises
        .iter()
        .filter_map(|ex| {
            if seen.insert(ex.subject.as_str()) {
                Some(ex.subject.clone())
            } else {
                None
            }
        })
        .collect();

    // ── 3. Ratatui init ────────────────────────────────────────────────
    let mut terminal = ratatui::init();

    let result = app.run_watch(&mut terminal, &conn);

    // Save session state for "Continue" in launcher
    if let Err(e) =
        progress::save_last_session(&conn, "watch", filter_chapter, app.state.current_index)
    {
        eprintln!("[clings] erreur sauvegarde session: {e}");
    }

    ratatui::restore();

    // ── 4. Cleanup tmux ────────────────────────────────────────────────
    if let Some(pane) = &app.state.editor_pane {
        tmux::kill_pane(pane);
    }

    result?;

    // ── 5. Sync push (si activé) — export synchrone, commit+push en background ─
    if sync_cfg.enabled {
        match progress::export_progress(&conn) {
            Ok(json_str) => {
                let snapshot = clings_dir.join(crate::constants::SYNC_SNAPSHOT_FILENAME);
                if let Err(e) = std::fs::write(&snapshot, &json_str) {
                    eprintln!("  {} écriture snapshot: {e}", "⚠".yellow());
                } else {
                    let dir = clings_dir.clone();
                    let cfg = sync_cfg.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = sync::commit_and_push(&dir, &cfg) {
                            eprintln!("  {} sync push: {e}", "⚠".yellow());
                        }
                    });
                }
            }
            Err(e) => eprintln!("  {} export progression: {e}", "⚠".yellow()),
        }
    }

    // ── 6. Summary post-session ────────────────────────────────────────
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
