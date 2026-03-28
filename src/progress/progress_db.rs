//! Database schema initialization and migrations for progress tracking.

use rusqlite::Connection;

use crate::constants::{clings_data_dir, DB_BUSY_TIMEOUT_MS, DB_FILENAME, DB_USER_VERSION_CURRENT};
use crate::error::Result;

/// Initial schema (v0), contains exercise_scores table.
pub(crate) const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS exercise_scores (
    exercise_id     TEXT PRIMARY KEY,
    subject         TEXT NOT NULL,
    attempts        INTEGER NOT NULL DEFAULT 0,
    successes       INTEGER NOT NULL DEFAULT 0,
    last_tried_at   INTEGER,
    last_success_at INTEGER
);
";

/// Current schema: subjects, practice_log, and key-value store tables with indexes.
pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS subjects (
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
);

CREATE INDEX IF NOT EXISTS idx_practice_log_practiced_at ON practice_log(practiced_at DESC);
CREATE INDEX IF NOT EXISTS idx_subjects_next_review ON subjects(next_review_at ASC, mastery_score ASC);
";

/// Open (or create) the progress database.
///
/// Sets up the SQLite connection with WAL mode, foreign keys enabled,
/// and applies the schema and migrations.
pub fn open_db() -> Result<Connection> {
    let dir = clings_data_dir();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(&dir)?;

    let db_path = dir.join(DB_FILENAME);
    let conn = Connection::open(&db_path)?;

    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "busy_timeout", DB_BUSY_TIMEOUT_MS)?;
    conn.execute_batch(SCHEMA)?;
    migrate_v1(&conn)?;

    Ok(conn)
}

/// Additive migration: add `exercise_scores` table (user_version 0 → 1).
pub(crate) fn migrate_v1(conn: &Connection) -> Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < DB_USER_VERSION_CURRENT {
        conn.execute_batch(SCHEMA_V1)?;
        conn.pragma_update(None, "user_version", DB_USER_VERSION_CURRENT)?;
    }

    // Additive migration v2: add optional reporting columns to practice_log.
    // Safe expand: check existence first, only add if missing.
    add_practice_log_columns_if_missing(conn)?;

    Ok(())
}

/// Add practice_log columns for reporting if they don't exist yet.
/// Columns: error_type TEXT, duration_ms INTEGER, hint_count_used INTEGER DEFAULT 0
pub(crate) fn add_practice_log_columns_if_missing(conn: &Connection) -> Result<()> {
    // Check if error_type column exists.
    let has_error_type: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('practice_log') WHERE name = 'error_type'",
        [],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    )?;

    if !has_error_type {
        conn.execute("ALTER TABLE practice_log ADD COLUMN error_type TEXT", [])?;
    }

    // Check if duration_ms column exists.
    let has_duration_ms: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('practice_log') WHERE name = 'duration_ms'",
        [],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    )?;

    if !has_duration_ms {
        conn.execute(
            "ALTER TABLE practice_log ADD COLUMN duration_ms INTEGER",
            [],
        )?;
    }

    // Check if hint_count_used column exists.
    let has_hint_count: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('practice_log') WHERE name = 'hint_count_used'",
        [],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    )?;

    if !has_hint_count {
        conn.execute(
            "ALTER TABLE practice_log ADD COLUMN hint_count_used INTEGER DEFAULT 0",
            [],
        )?;
    }

    Ok(())
}
