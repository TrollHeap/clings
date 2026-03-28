//! Checkpoint persistence (piscine, exam) and kv-store helpers.
//!
//! Provides a lightweight key-value store on top of the `kv` SQLite table,
//! plus typed checkpoint read/write operations for piscine and exam modes.

use rusqlite::{params, Connection, OptionalExtension};

use crate::constants::{EXAM_CHECKPOINT_KEY, LAST_EXAM_SESSION_KEY, PISCINE_CHECKPOINT_KEY};
use crate::error::Result;

/// Upsert a key-value pair in the `kv` table.
pub(super) fn kv_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    let mut stmt = conn.prepare_cached("INSERT OR REPLACE INTO kv (key, value) VALUES (?1, ?2)")?;
    stmt.execute(params![key, value])?;
    Ok(())
}

/// Retrieve a value from the `kv` table. Returns `None` if the key does not exist.
pub(super) fn kv_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare_cached("SELECT value FROM kv WHERE key = ?1")?;
    Ok(stmt.query_row(params![key], |row| row.get(0)).optional()?)
}

/// Delete a key from the `kv` table. Succeeds silently if the key does not exist.
pub(super) fn kv_del(conn: &Connection, key: &str) -> Result<()> {
    let mut stmt = conn.prepare_cached("DELETE FROM kv WHERE key = ?1")?;
    stmt.execute(params![key])?;
    Ok(())
}

/// Save piscine checkpoint (current exercise index).
pub fn save_piscine_checkpoint(conn: &Connection, index: usize) -> Result<()> {
    kv_set(conn, PISCINE_CHECKPOINT_KEY, &index.to_string())
}

/// Load piscine checkpoint, returns None if no checkpoint saved.
pub fn load_piscine_checkpoint(conn: &Connection) -> Result<Option<usize>> {
    Ok(
        kv_get(conn, PISCINE_CHECKPOINT_KEY)?.and_then(|s| match s.parse::<usize>() {
            Ok(idx) => Some(idx),
            Err(_) => {
                eprintln!("[clings/progress] checkpoint piscine invalide : {s:?}");
                None
            }
        }),
    )
}

/// Clear piscine checkpoint (called when piscine is fully completed).
pub fn clear_piscine_checkpoint(conn: &Connection) -> Result<()> {
    kv_del(conn, PISCINE_CHECKPOINT_KEY)
}

/// Save exam checkpoint: stores "{session_id}:{index}" under exam_checkpoint key.
/// `session_id` is an annale ID (e.g. "nsy103_2023_juin") — no colons allowed, parsed with `rsplit_once(':')`.
pub fn save_exam_checkpoint(conn: &Connection, session_id: &str, index: usize) -> Result<()> {
    kv_set(conn, EXAM_CHECKPOINT_KEY, &format!("{session_id}:{index}"))
}

/// Load exam checkpoint for the given session_id. Returns None if no checkpoint exists or if the
/// stored session differs (i.e. the user switched to a different exam session).
pub fn load_exam_checkpoint(conn: &Connection, session_id: &str) -> Result<Option<usize>> {
    Ok(kv_get(conn, EXAM_CHECKPOINT_KEY)?.and_then(|s| {
        s.rsplit_once(':')
            .filter(|(sid, _)| *sid == session_id)
            .and_then(|(_, rest)| match rest.parse::<usize>() {
                Ok(idx) => Some(idx),
                Err(_) => {
                    eprintln!("[clings/progress] checkpoint exam invalide : {s:?}");
                    None
                }
            })
    }))
}

/// Clear exam checkpoint (called when exam session is fully completed).
pub fn clear_exam_checkpoint(conn: &Connection) -> Result<()> {
    kv_del(conn, EXAM_CHECKPOINT_KEY)
}

/// Save the ID of the last selected exam session (for TUI sélecteur).
pub fn save_last_exam_session(conn: &Connection, session_id: &str) -> Result<()> {
    kv_set(conn, LAST_EXAM_SESSION_KEY, session_id)
}

/// Load the ID of the last selected exam session. Returns None if never set.
pub fn load_last_exam_session(conn: &Connection) -> Result<Option<String>> {
    kv_get(conn, LAST_EXAM_SESSION_KEY)
}
