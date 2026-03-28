//! SQLite persistence layer — mastery tracking, SRS state, and practice history.

mod checkpoints;
mod import_export;
mod mastery_calc;
mod progress_db;
mod session;
mod subjects;

pub use checkpoints::{
    clear_exam_checkpoint, clear_piscine_checkpoint, load_exam_checkpoint, load_last_exam_session,
    load_piscine_checkpoint, save_exam_checkpoint, save_last_exam_session, save_piscine_checkpoint,
};
pub use import_export::{export_progress, import_progress};
pub use progress_db::open_db;
pub use session::{load_last_session, save_last_session};
pub use subjects::{
    apply_all_decay, ensure_subjects_batch, get_all_subjects, get_all_weakest_exercises,
    get_daily_activity, get_due_subjects, get_exercise_scores, get_streak, get_subject,
    get_subject_attempts, record_attempt, reset_progress, reset_subject,
};

// ── Test infrastructure (integration/unit tests) ────────────────────────────────
// Note: These helpers use internal test-only module functions. They're only meant
// to be used in tests and should never be called from library code.

use progress_db::{add_practice_log_columns_if_missing, SCHEMA, SCHEMA_V1};

/// Open an in-memory database for testing (used only by integration/unit tests).
pub fn open_db_for_test() -> crate::error::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA)?;
    conn.execute_batch(SCHEMA_V1)?;
    add_practice_log_columns_if_missing(&conn)?;
    Ok(conn)
}

/// Ensure a subject exists in the database (used only by tests).
pub fn ensure_subject_for_test(
    conn: &rusqlite::Connection,
    name: &str,
) -> crate::error::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO subjects (name) VALUES (?1)",
        rusqlite::params![name],
    )?;
    Ok(())
}

/// Migrate to v1 schema (used only by tests).
pub fn migrate_v1_for_test(conn: &rusqlite::Connection) -> crate::error::Result<()> {
    progress_db::migrate_v1(conn)
}
