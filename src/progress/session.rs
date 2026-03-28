//! Last session state persistence — saves and restores the launcher "Continue" context.

use rusqlite::Connection;

use super::checkpoints::{kv_get, kv_set};
use crate::error::Result;

/// Save last session state (mode + chapter + exercise index) for the launcher "Continue" option.
pub fn save_last_session(
    conn: &Connection,
    mode: &str,
    chapter: Option<u8>,
    index: usize,
) -> Result<()> {
    kv_set(conn, "last_mode", mode)?;
    kv_set(
        conn,
        "last_chapter",
        &chapter.map_or("0".to_string(), |c| c.to_string()),
    )?;
    kv_set(conn, "last_exercise_index", &index.to_string())?;
    Ok(())
}

/// Load last session state. Returns None if no session was saved.
pub fn load_last_session(conn: &Connection) -> Result<Option<(String, Option<u8>, usize)>> {
    let mode = kv_get(conn, "last_mode")?;
    let chapter = kv_get(conn, "last_chapter")?;
    let index = kv_get(conn, "last_exercise_index")?;
    match (mode, chapter, index) {
        (Some(m), Some(c), Some(i)) => {
            let ch = c.parse::<u8>().ok().filter(|&n| n > 0);
            let idx = i.parse::<usize>().unwrap_or(0);
            Ok(Some((m, ch, idx)))
        }
        _ => Ok(None),
    }
}
