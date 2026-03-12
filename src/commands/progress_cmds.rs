//! Commandes de progression — progress et stats.

use crate::error::Result;
use crate::{display, progress};

pub fn cmd_progress(subject: Option<&str>) -> Result<()> {
    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;
    if let Some(s) = subject {
        let scores = progress::get_exercise_scores(&conn, s)?;
        display::show_exercise_scores(s, &scores);
    } else {
        let subjects = progress::get_all_subjects(&conn)?;
        let streak = progress::get_streak(&conn)?;
        display::show_progress(&subjects, streak);
    }
    Ok(())
}

pub fn cmd_stats(detailed: bool) -> Result<()> {
    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;
    let subjects = progress::get_all_subjects(&conn)?;
    let streak = progress::get_streak(&conn)?;
    if detailed {
        let attempts = progress::get_subject_attempts(&conn)?;
        let daily = progress::get_daily_activity(&conn, 30)?;
        crate::tui::ui_stats::run_stats(&subjects, streak as u32, Some(&attempts), Some(&daily))
    } else {
        crate::tui::ui_stats::run_stats(&subjects, streak as u32, None, None)
    }
}
