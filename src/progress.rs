//! SQLite persistence layer — mastery tracking, SRS state, and practice history.

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use serde::Deserialize;

use crate::constants::{
    CLINGS_DIR, DB_BUSY_TIMEOUT_MS, DB_FILENAME, DB_USER_VERSION_CURRENT, EXAM_CHECKPOINT_KEY,
    LAST_EXAM_SESSION_KEY, PISCINE_CHECKPOINT_KEY, SECS_PER_DAY,
};
use crate::error::Result;
use crate::mastery;
use crate::models::{MasteryScore, SrsIntervalDays, Subject};

const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS exercise_scores (
    exercise_id     TEXT PRIMARY KEY,
    subject         TEXT NOT NULL,
    attempts        INTEGER NOT NULL DEFAULT 0,
    successes       INTEGER NOT NULL DEFAULT 0,
    last_tried_at   INTEGER,
    last_success_at INTEGER
);
";

const SCHEMA: &str = "
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
";

/// Open (or create) the progress database.
pub fn open_db() -> Result<Connection> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        crate::error::KfError::Config(
            "Variable $HOME non définie — impossible de localiser ~/.clings".to_string(),
        )
    })?;
    let dir = std::path::PathBuf::from(home).join(CLINGS_DIR);
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
fn migrate_v1(conn: &Connection) -> Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < DB_USER_VERSION_CURRENT {
        conn.execute_batch(SCHEMA_V1)?;
        conn.pragma_update(None, "user_version", DB_USER_VERSION_CURRENT)?;
    }
    Ok(())
}

/// Ensure a subject row exists in the DB (used in tests as setup helper).
#[cfg(test)]
pub fn ensure_subject(conn: &Connection, name: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO subjects (name) VALUES (?1)",
        params![name],
    )?;
    Ok(())
}

/// Batch-ensure all subject rows exist (single transaction).
pub fn ensure_subjects_batch(
    conn: &mut Connection,
    exercises: &[crate::models::Exercise],
) -> Result<()> {
    let unique: std::collections::HashSet<&str> =
        exercises.iter().map(|e| e.subject.as_str()).collect();
    let tx = conn.transaction()?;
    for name in unique {
        tx.execute(
            "INSERT OR IGNORE INTO subjects (name) VALUES (?1)",
            params![name],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Map a rusqlite row (columns 0-7: name, mastery_score, last_practiced_at,
/// attempts_total, attempts_success, difficulty_unlocked, next_review_at,
/// srs_interval_days) to a Subject.
fn row_to_subject(row: &rusqlite::Row) -> rusqlite::Result<Subject> {
    Ok(Subject {
        name: row.get(0)?,
        mastery_score: {
            let v: f64 = row.get(1)?;
            MasteryScore::clamped(v)
        },
        last_practiced_at: row.get(2)?,
        attempts_total: row.get(3)?,
        attempts_success: row.get(4)?,
        difficulty_unlocked: row.get(5)?,
        next_review_at: row.get(6)?,
        srs_interval_days: {
            let v: i64 = row.get::<_, i64>(7)?;
            SrsIntervalDays::clamped(v)
        },
    })
}

/// Get all subjects from DB.
pub fn get_all_subjects(conn: &Connection) -> Result<Vec<Subject>> {
    let mut stmt = conn.prepare_cached(
        "SELECT name, mastery_score, last_practiced_at, attempts_total, attempts_success,
                difficulty_unlocked, next_review_at, srs_interval_days
         FROM subjects ORDER BY name",
    )?;

    let subjects = stmt
        .query_map([], row_to_subject)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(subjects)
}

/// Record a practice attempt and update mastery.
///
/// All three writes (ensure subject, log entry, mastery update) are wrapped in a
/// transaction so a crash mid-way never leaves the DB in an inconsistent state.
pub fn record_attempt(
    conn: &Connection,
    subject: &str,
    exercise_id: &str,
    success: bool,
) -> Result<Subject> {
    let now = Utc::now().timestamp();
    let log_id = uuid::Uuid::new_v4().to_string();

    let tx = conn.unchecked_transaction()?;

    tx.execute(
        "INSERT OR IGNORE INTO subjects (name) VALUES (?1)",
        params![subject],
    )?;

    tx.execute(
        "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![log_id, subject, exercise_id, success as i32, now],
    )?;

    let mut sub = get_subject(&tx, subject)?.unwrap_or_else(|| Subject::new(subject.to_string()));
    mastery::update_mastery(&mut sub, success);

    let (next_review, new_interval) =
        mastery::compute_next_review(sub.srs_interval_days.get(), success, now);
    sub.next_review_at = Some(next_review);
    sub.srs_interval_days = SrsIntervalDays::clamped(new_interval);

    tx.execute(
        "UPDATE subjects SET
            mastery_score = ?2,
            last_practiced_at = ?3,
            attempts_total = ?4,
            attempts_success = ?5,
            difficulty_unlocked = ?6,
            next_review_at = ?7,
            srs_interval_days = ?8
         WHERE name = ?1",
        params![
            sub.name,
            sub.mastery_score.get(),
            sub.last_practiced_at,
            sub.attempts_total,
            sub.attempts_success,
            sub.difficulty_unlocked,
            sub.next_review_at,
            sub.srs_interval_days.get(),
        ],
    )?;

    // Upsert into exercise_scores (same transaction)
    tx.execute(
        "INSERT INTO exercise_scores (exercise_id, subject, attempts, successes, last_tried_at, last_success_at)
         VALUES (?1, ?2, 1, ?3, ?4, ?5)
         ON CONFLICT(exercise_id) DO UPDATE SET
             attempts        = attempts + 1,
             successes       = successes + excluded.successes,
             last_tried_at   = excluded.last_tried_at,
             last_success_at = CASE WHEN excluded.successes > 0 THEN excluded.last_tried_at ELSE last_success_at END",
        params![
            exercise_id,
            subject,
            success as i32,
            now,
            if success { Some(now) } else { None::<i64> },
        ],
    )?;

    tx.commit()?;
    trim_practice_log(conn)?;
    Ok(sub)
}

/// Supprime les lignes de `practice_log` au-delà des 10 000 plus récentes.
/// Sans effet si le nombre de lignes est inférieur à ce seuil.
fn trim_practice_log(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM practice_log
         WHERE practiced_at < (
             SELECT practiced_at FROM practice_log
             ORDER BY practiced_at DESC LIMIT 1 OFFSET 9999
         )",
        [],
    )?;
    Ok(())
}

/// Get a single subject.
pub fn get_subject(conn: &Connection, name: &str) -> Result<Option<Subject>> {
    let mut stmt = conn.prepare_cached(
        "SELECT name, mastery_score, last_practiced_at, attempts_total, attempts_success,
                difficulty_unlocked, next_review_at, srs_interval_days
         FROM subjects WHERE name = ?1",
    )?;

    let subject = stmt.query_row(params![name], row_to_subject).optional()?;

    Ok(subject)
}

/// Get current streak (consecutive days with at least one practice).
pub fn get_streak(conn: &Connection) -> Result<i64> {
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT date(practiced_at, 'unixepoch') as day
         FROM practice_log
         ORDER BY day DESC
         LIMIT 90",
    )?;

    let days: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if days.is_empty() {
        return Ok(0);
    }

    let today = Utc::now().format("%Y-%m-%d").to_string();
    let yesterday = (Utc::now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    // Streak must include today or yesterday
    if days[0] != today && days[0] != yesterday {
        return Ok(0);
    }

    let mut streak = 1i64;
    for window in days.windows(2) {
        if let (Ok(current), Ok(prev)) = (
            chrono::NaiveDate::parse_from_str(&window[0], "%Y-%m-%d"),
            chrono::NaiveDate::parse_from_str(&window[1], "%Y-%m-%d"),
        ) {
            if (current - prev).num_days() == 1 {
                streak += 1;
            } else {
                break;
            }
        }
    }

    Ok(streak)
}

/// Réinitialise la progression d'un seul sujet (mastery + logs), atomiquement.
pub fn reset_subject(conn: &Connection, subject_name: &str) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM subjects WHERE name = ?1",
        params![subject_name],
    )?;
    tx.execute(
        "DELETE FROM practice_log WHERE subject = ?1",
        params![subject_name],
    )?;
    tx.commit()?;
    Ok(())
}

/// Reset all progress: removes every row from `practice_log` and `subjects`.
///
/// This is a destructive, irreversible operation. Both tables are truncated
/// atomically via `execute_batch`. Used by `clings reset` (full reset only;
/// for subject-scoped reset see [`reset_subject`]).
pub fn reset_progress(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM kv", [])?;
    tx.execute("DELETE FROM practice_log", [])?;
    tx.execute("DELETE FROM subjects", [])?;
    tx.commit()?;
    Ok(())
}

/// Open an in-memory database for testing.
#[cfg(test)]
fn open_test_db() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA)?;
    conn.execute_batch(SCHEMA_V1)?;
    Ok(conn)
}

/// Upsert a key-value pair in the `kv` table.
fn kv_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    let mut stmt = conn.prepare_cached("INSERT OR REPLACE INTO kv (key, value) VALUES (?1, ?2)")?;
    stmt.execute(params![key, value])?;
    Ok(())
}

/// Retrieve a value from the `kv` table. Returns `None` if the key does not exist.
fn kv_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare_cached("SELECT value FROM kv WHERE key = ?1")?;
    Ok(stmt.query_row(params![key], |row| row.get(0)).optional()?)
}

/// Delete a key from the `kv` table. Succeeds silently if the key does not exist.
fn kv_del(conn: &Connection, key: &str) -> Result<()> {
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
    Ok(kv_get(conn, PISCINE_CHECKPOINT_KEY)?.and_then(|s| {
        s.parse()
            .map_err(|_| eprintln!("[clings/progress] checkpoint piscine invalide : {s:?}"))
            .ok()
    }))
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
            .and_then(|(_, rest)| {
                rest.parse()
                    .map_err(|_| eprintln!("[clings/progress] checkpoint exam invalide : {s:?}"))
                    .ok()
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

/// Get the number of days until the next SRS review for a subject.
/// Get subjects whose SRS review is due (next_review_at <= now).
pub fn get_due_subjects(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT name FROM subjects
         WHERE next_review_at IS NOT NULL AND next_review_at <= unixepoch()
         ORDER BY mastery_score ASC",
    )?;
    let names = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(names)
}

/// Retourne l'exercice le plus faible (taux de succès le plus bas) par sujet, en une seule requête.
/// Utilisé par `clings review` pour éviter N+1 requêtes par sujet.
pub fn get_all_weakest_exercises(
    conn: &Connection,
) -> Result<std::collections::HashMap<String, String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT subject, exercise_id
         FROM (
             SELECT subject, exercise_id,
                    ROW_NUMBER() OVER (
                        PARTITION BY subject
                        ORDER BY CAST(successes AS REAL) / MAX(attempts, 1) ASC,
                                 attempts DESC
                    ) AS rn
             FROM exercise_scores
         )
         WHERE rn = 1",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<rusqlite::Result<std::collections::HashMap<String, String>>>()
        .map_err(crate::error::KfError::from)
}

/// Retourne (exercise_id, successes, attempts) pour un sujet donné.
pub fn get_exercise_scores(conn: &Connection, subject: &str) -> Result<Vec<(String, u32, u32)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT exercise_id, successes, attempts
         FROM exercise_scores
         WHERE subject = ?1
         ORDER BY exercise_id",
    )?;
    let rows = stmt.query_map(params![subject], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, u32>(1)?,
            row.get::<_, u32>(2)?,
        ))
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::error::KfError::from)
}

/// Retourne (subject, attempts_success, attempts_total) pour tous les sujets pratiqués.
pub fn get_subject_attempts(conn: &Connection) -> Result<Vec<(String, u32, u32)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT subject,
                COUNT(CASE WHEN success = 1 THEN 1 END),
                COUNT(*)
         FROM practice_log
         GROUP BY subject
         ORDER BY subject",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, u32>(1)?,
            row.get::<_, u32>(2)?,
        ))
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::error::KfError::from)
}

/// Retourne (date_iso, count) pour les `days` derniers jours d'activité.
pub fn get_daily_activity(conn: &Connection, days: u32) -> Result<Vec<(String, u32)>> {
    let cutoff = chrono::Utc::now().timestamp() - (days as i64 * SECS_PER_DAY);
    let mut stmt = conn.prepare_cached(
        "SELECT date(practiced_at, 'unixepoch') AS day, COUNT(*)
         FROM practice_log
         WHERE practiced_at >= ?1
         GROUP BY day
         ORDER BY day",
    )?;
    let rows = stmt.query_map([cutoff], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::error::KfError::from)
}

/// Applique la décroissance SRS à tous les sujets avec `mastery > 0`
/// et `elapsed >= decay_days`. Idempotent : sûr d'appeler plusieurs fois.
///
/// Only fetches subjects with `mastery_score > 0`, a known `last_practiced_at`, and enough
/// elapsed time — skipping the rest entirely instead of loading the full table.
///
/// # Errors
/// `KfError::Database` if the transaction or query fails (auto-converted via `#[from]`).
pub fn apply_all_decay(conn: &mut Connection) -> Result<()> {
    let decay_days = crate::config::get().srs.decay_days;
    let mut stmt = conn.prepare_cached(
        "SELECT name, mastery_score, last_practiced_at, attempts_total, attempts_success,
                difficulty_unlocked, next_review_at, srs_interval_days
         FROM subjects
         WHERE mastery_score > 0.0
           AND last_practiced_at IS NOT NULL
           AND last_practiced_at < unixepoch('now') - (?1 * 86400)
         ORDER BY name",
    )?;
    let mut subjects = stmt
        .query_map([decay_days], row_to_subject)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    let tx = conn.transaction()?;
    for sub in &mut subjects {
        let old_score = sub.mastery_score;
        mastery::apply_decay(sub);
        if sub.mastery_score != old_score {
            tx.execute(
                "UPDATE subjects SET mastery_score = ?2, last_practiced_at = ?3 WHERE name = ?1",
                params![sub.name, sub.mastery_score.get(), sub.last_practiced_at],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Sérialise tous les sujets + métadonnées en JSON (pour sauvegarde/transfert).
pub fn export_progress(conn: &Connection) -> Result<String> {
    let subjects = get_all_subjects(conn)?;

    #[derive(serde::Serialize)]
    struct ExportData<'a> {
        version: u32,
        exported_at: String,
        subjects: &'a [Subject],
    }

    let data = ExportData {
        version: 1,
        exported_at: chrono::Utc::now().to_rfc3339(),
        subjects: &subjects,
    };

    serde_json::to_string_pretty(&data)
        .map_err(|e| crate::error::KfError::Config(format!("serialization error: {e}")))
}

/// Importe les sujets depuis un JSON exporté.
/// Si `overwrite` est true, remplace les valeurs existantes.
/// Si false, prend le max(mastery existant, mastery importé).
/// Retourne `(count, warnings)` — le nombre de sujets importés et les avertissements de clamp.
pub fn import_progress(
    conn: &mut Connection,
    json: &str,
    overwrite: bool,
) -> Result<(usize, Vec<String>)> {
    #[derive(Deserialize)]
    struct ImportData {
        subjects: Vec<Subject>,
    }

    let data: ImportData = serde_json::from_str(json)
        .map_err(|e| crate::error::KfError::Config(format!("invalid JSON: {e}")))?;

    let tx = conn.transaction()?;
    let mut count = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    for sub in &data.subjects {
        let clamped_score = sub.mastery_score.get();
        let clamped_difficulty = sub.difficulty_unlocked.clamp(1, 5);
        let clamped_interval = sub.srs_interval_days.get().clamp(
            crate::constants::SRS_BASE_INTERVAL_DAYS,
            crate::constants::SRS_MAX_INTERVAL_DAYS,
        );
        let clamped_total = sub.attempts_total.max(0);
        let clamped_success = sub.attempts_success.max(0).min(clamped_total);

        if clamped_difficulty != sub.difficulty_unlocked {
            warnings.push(format!(
                "'{}': difficulty_unlocked {} → {} (clamped)",
                sub.name, sub.difficulty_unlocked, clamped_difficulty
            ));
        }
        if clamped_interval != sub.srs_interval_days.get() {
            warnings.push(format!(
                "'{}': srs_interval_days {} → {} (clamped)",
                sub.name,
                sub.srs_interval_days.get(),
                clamped_interval
            ));
        }
        if clamped_total != sub.attempts_total {
            warnings.push(format!(
                "'{}': attempts_total {} → {} (clamped)",
                sub.name, sub.attempts_total, clamped_total
            ));
        }
        if clamped_success != sub.attempts_success {
            warnings.push(format!(
                "'{}': attempts_success {} → {} (clamped)",
                sub.name, sub.attempts_success, clamped_success
            ));
        }

        if overwrite {
            tx.execute(
                "INSERT OR REPLACE INTO subjects
                 (name, mastery_score, last_practiced_at, attempts_total, attempts_success,
                  difficulty_unlocked, next_review_at, srs_interval_days)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    sub.name,
                    clamped_score,
                    sub.last_practiced_at,
                    clamped_total,
                    clamped_success,
                    clamped_difficulty,
                    sub.next_review_at,
                    clamped_interval,
                ],
            )?;
        } else {
            tx.execute(
                "INSERT INTO subjects
                 (name, mastery_score, last_practiced_at, attempts_total, attempts_success,
                  difficulty_unlocked, next_review_at, srs_interval_days)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(name) DO UPDATE SET
                   mastery_score = MAX(mastery_score, excluded.mastery_score),
                   difficulty_unlocked = MAX(difficulty_unlocked, excluded.difficulty_unlocked)",
                params![
                    sub.name,
                    clamped_score,
                    sub.last_practiced_at,
                    clamped_total,
                    clamped_success,
                    clamped_difficulty,
                    sub.next_review_at,
                    clamped_interval,
                ],
            )?;
        }
        count += 1;
    }

    tx.commit()?;
    Ok((count, warnings))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_subject_creates_row() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        let sub = get_subject(&conn, "pointers")?;
        assert!(sub.is_some());
        let sub = sub.expect("subject should exist after insert");
        assert_eq!(sub.name, "pointers");
        assert_eq!(sub.mastery_score.get(), 0.0);
        assert_eq!(sub.difficulty_unlocked, 1);
        Ok(())
    }

    #[test]
    fn test_ensure_subject_idempotent() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        ensure_subject(&conn, "pointers")?;
        let subjects = get_all_subjects(&conn)?;
        assert_eq!(subjects.len(), 1);
        Ok(())
    }

    #[test]
    fn test_record_attempt_success() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "structs")?;
        let sub = record_attempt(&conn, "structs", "struct-point-01", true)?;
        assert_eq!(sub.mastery_score.get(), 1.0);
        assert_eq!(sub.attempts_total, 1);
        assert_eq!(sub.attempts_success, 1);
        Ok(())
    }

    #[test]
    fn test_record_attempt_failure() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "structs")?;

        // First succeed to have score > 0
        record_attempt(&conn, "structs", "struct-point-01", true)?;
        let sub = record_attempt(&conn, "structs", "struct-point-01", false)?;
        assert_eq!(sub.mastery_score.get(), 0.5);
        assert_eq!(sub.attempts_total, 2);
        assert_eq!(sub.attempts_success, 1);
        Ok(())
    }

    #[test]
    fn test_reset_progress() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        record_attempt(&conn, "pointers", "ptr-deref-01", true)?;
        reset_progress(&conn)?;
        let subjects = get_all_subjects(&conn)?;
        assert!(subjects.is_empty());
        Ok(())
    }

    #[test]
    fn test_get_subject_missing() -> Result<()> {
        let conn = open_test_db()?;
        let sub = get_subject(&conn, "nonexistent")?;
        assert!(sub.is_none());
        Ok(())
    }

    #[test]
    fn test_reset_subject_isolated() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        ensure_subject(&conn, "structs")?;
        record_attempt(&conn, "pointers", "ptr-deref-01", true)?;
        record_attempt(&conn, "structs", "struct-point-01", true)?;

        reset_subject(&conn, "pointers")?;

        // pointers supprimé
        assert!(get_subject(&conn, "pointers")?.is_none());
        // structs intact
        let s = get_subject(&conn, "structs")?.expect("structs should exist after insert");
        assert_eq!(s.mastery_score.get(), 1.0);
        // log pointers supprimé, log structs intact
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM practice_log WHERE subject = 'structs'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(count, 1);
        let count_ptr: i64 = conn.query_row(
            "SELECT COUNT(*) FROM practice_log WHERE subject = 'pointers'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(count_ptr, 0);
        Ok(())
    }

    #[test]
    fn test_kv_checkpoint_roundtrip() -> Result<()> {
        let conn = open_test_db()?;
        save_piscine_checkpoint(&conn, 42)?;
        let loaded = load_piscine_checkpoint(&conn)?;
        assert_eq!(loaded, Some(42));
        Ok(())
    }

    #[test]
    fn test_kv_checkpoint_missing_returns_none() -> Result<()> {
        let conn = open_test_db()?;
        let loaded = load_piscine_checkpoint(&conn)?;
        assert_eq!(loaded, None);
        Ok(())
    }

    #[test]
    fn test_kv_checkpoint_clear() -> Result<()> {
        let conn = open_test_db()?;
        save_piscine_checkpoint(&conn, 7)?;
        clear_piscine_checkpoint(&conn)?;
        let loaded = load_piscine_checkpoint(&conn)?;
        assert_eq!(loaded, None);
        Ok(())
    }

    #[test]
    fn test_kv_checkpoint_overwrite() -> Result<()> {
        let conn = open_test_db()?;
        save_piscine_checkpoint(&conn, 3)?;
        save_piscine_checkpoint(&conn, 17)?;
        let loaded = load_piscine_checkpoint(&conn)?;
        assert_eq!(loaded, Some(17));
        Ok(())
    }

    #[test]
    fn test_exam_checkpoint_roundtrip() -> Result<()> {
        let conn = open_test_db()?;
        save_exam_checkpoint(&conn, "nsy103-2024", 5)?;
        let loaded = load_exam_checkpoint(&conn, "nsy103-2024")?;
        assert_eq!(loaded, Some(5));
        Ok(())
    }

    #[test]
    fn test_exam_checkpoint_session_isolation() -> Result<()> {
        let conn = open_test_db()?;
        save_exam_checkpoint(&conn, "nsy103-2024", 3)?;
        let other = load_exam_checkpoint(&conn, "utc502-2023")?;
        assert_eq!(other, None);
        Ok(())
    }

    #[test]
    fn test_exam_checkpoint_session_id_with_colon() -> Result<()> {
        let conn = open_test_db()?;
        save_exam_checkpoint(&conn, "utc502:2024", 7)?;
        let loaded = load_exam_checkpoint(&conn, "utc502:2024")?;
        assert_eq!(loaded, Some(7));
        // A session_id that only matches the prefix must not match
        let wrong = load_exam_checkpoint(&conn, "utc502")?;
        assert_eq!(wrong, None);
        Ok(())
    }

    #[test]
    fn test_exam_checkpoint_clear() -> Result<()> {
        let conn = open_test_db()?;
        save_exam_checkpoint(&conn, "nsy103-2024", 2)?;
        clear_exam_checkpoint(&conn)?;
        let loaded = load_exam_checkpoint(&conn, "nsy103-2024")?;
        assert_eq!(loaded, None);
        Ok(())
    }

    #[test]
    fn test_due_subjects_past_review() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        // next_review_at dans le passé → sujet doit apparaître
        let past = Utc::now().timestamp() - 3_600;
        conn.execute(
            "UPDATE subjects SET next_review_at = ?1 WHERE name = 'pointers'",
            params![past],
        )?;
        let due = get_due_subjects(&conn)?;
        assert!(due.contains(&"pointers".to_string()));
        Ok(())
    }

    #[test]
    fn test_due_subjects_future_review() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        // next_review_at dans le futur → sujet absent
        let future = Utc::now().timestamp() + SECS_PER_DAY;
        conn.execute(
            "UPDATE subjects SET next_review_at = ?1 WHERE name = 'pointers'",
            params![future],
        )?;
        let due = get_due_subjects(&conn)?;
        assert!(!due.contains(&"pointers".to_string()));
        Ok(())
    }

    #[test]
    fn test_due_subjects_null_review() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        // next_review_at NULL par défaut → sujet absent
        let due = get_due_subjects(&conn)?;
        assert!(!due.contains(&"pointers".to_string()));
        Ok(())
    }

    #[test]
    fn test_get_streak_empty() -> Result<()> {
        let conn = open_test_db()?;
        assert_eq!(get_streak(&conn)?, 0);
        Ok(())
    }

    #[test]
    fn test_get_streak_today() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at) VALUES ('t1', 'pointers', 'ex1', 1, ?1)",
            params![now],
        )?;
        assert_eq!(get_streak(&conn)?, 1);
        Ok(())
    }

    #[test]
    fn test_get_streak_consecutive_days() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        let now = Utc::now().timestamp();
        for (i, id) in ["c1", "c2", "c3"].iter().enumerate() {
            let ts = now - (i as i64) * SECS_PER_DAY;
            conn.execute(
                "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at) VALUES (?1, 'pointers', 'ex1', 1, ?2)",
                params![id, ts],
            )?;
        }
        assert_eq!(get_streak(&conn)?, 3);
        Ok(())
    }

    #[test]
    fn test_get_streak_broken() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        let now = Utc::now().timestamp();
        // Aujourd'hui et il y a 3 jours (pas hier) → streak = 1
        for (id, offset) in [("b1", 0i64), ("b2", 3)] {
            conn.execute(
                "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at) VALUES (?1, 'pointers', 'ex1', 1, ?2)",
                params![id, now - offset * SECS_PER_DAY],
            )?;
        }
        assert_eq!(get_streak(&conn)?, 1);
        Ok(())
    }

    #[test]
    fn test_apply_all_decay_updates_db() -> Result<()> {
        let mut conn = open_test_db()?;
        ensure_subject(&conn, "structs")?;
        let old_ts = Utc::now().timestamp() - 15 * SECS_PER_DAY;
        conn.execute(
            "UPDATE subjects SET mastery_score = 2.0, last_practiced_at = ?1 WHERE name = 'structs'",
            params![old_ts],
        )?;
        apply_all_decay(&mut conn)?;
        let sub = get_subject(&conn, "structs")?.expect("structs should exist after insert");
        assert_eq!(sub.mastery_score.get(), 1.5);
        Ok(())
    }

    #[test]
    fn test_apply_all_decay_no_change_when_recent() -> Result<()> {
        let mut conn = open_test_db()?;
        ensure_subject(&conn, "pipes")?;
        let recent_ts = Utc::now().timestamp() - 5 * SECS_PER_DAY;
        conn.execute(
            "UPDATE subjects SET mastery_score = 3.0, last_practiced_at = ?1 WHERE name = 'pipes'",
            params![recent_ts],
        )?;
        apply_all_decay(&mut conn)?;
        let sub = get_subject(&conn, "pipes")?.expect("pipes should exist after insert");
        assert_eq!(sub.mastery_score.get(), 3.0);
        Ok(())
    }

    #[test]
    fn test_apply_all_decay_updates_last_practiced_at() -> Result<()> {
        let mut conn = open_test_db()?;
        ensure_subject(&conn, "structs")?;
        let old_ts = Utc::now().timestamp() - 15 * SECS_PER_DAY;
        conn.execute(
            "UPDATE subjects SET mastery_score = 2.0, last_practiced_at = ?1 WHERE name = 'structs'",
            params![old_ts],
        )?;
        apply_all_decay(&mut conn)?;
        let sub = get_subject(&conn, "structs")?.expect("structs should exist after insert");
        assert_eq!(sub.mastery_score.get(), 1.5, "score must decay by 0.5");
        // last_practiced_at must have advanced (not remain at old_ts)
        assert!(
            sub.last_practiced_at
                .expect("last_practiced_at should be set")
                > old_ts,
            "last_practiced_at must advance after decay"
        );
        Ok(())
    }

    #[test]
    fn test_apply_all_decay_idempotent_in_db() -> Result<()> {
        let mut conn = open_test_db()?;
        ensure_subject(&conn, "pipes")?;
        let old_ts = Utc::now().timestamp() - 15 * SECS_PER_DAY;
        conn.execute(
            "UPDATE subjects SET mastery_score = 2.0, last_practiced_at = ?1 WHERE name = 'pipes'",
            params![old_ts],
        )?;
        apply_all_decay(&mut conn)?;
        let sub1 = get_subject(&conn, "pipes")?.expect("pipes should exist after insert");
        apply_all_decay(&mut conn)?;
        let sub2 = get_subject(&conn, "pipes")?.expect("pipes should exist after insert");
        assert_eq!(
            sub1.mastery_score.get(),
            sub2.mastery_score.get(),
            "decay must not compound on second call"
        );
        Ok(())
    }

    #[test]
    fn test_record_attempt_persists_srs_fields() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pipes")?;
        let sub = record_attempt(&conn, "pipes", "pipe-01", true)?;
        // Après un succès, intervalle SRS = round(1 * 2.5) = 3
        assert_eq!(sub.srs_interval_days.get(), 3);
        assert!(sub.next_review_at.is_some());
        // Vérifier la persistance en DB
        let reloaded = get_subject(&conn, "pipes")?.expect("pipes should exist after insert");
        assert_eq!(reloaded.srs_interval_days, sub.srs_interval_days);
        assert_eq!(reloaded.next_review_at, sub.next_review_at);
        Ok(())
    }

    #[test]
    fn test_import_progress_clamps_mastery_score() -> Result<()> {
        let mut conn = open_test_db()?;
        ensure_subject(&conn, "structs")?;

        // Create JSON with mastery_score = 10.0 (exceeds MASTERY_MAX = 5.0)
        let json = r#"
        {
            "subjects": [
                {
                    "name": "structs",
                    "mastery_score": 10.0,
                    "last_practiced_at": null,
                    "attempts_total": 0,
                    "attempts_success": 0,
                    "difficulty_unlocked": 1,
                    "next_review_at": null,
                    "srs_interval_days": 1
                }
            ]
        }
        "#;

        let (count, _warnings) = import_progress(&mut conn, json, true)?;
        assert_eq!(count, 1);

        // Verify the score was clamped to MASTERY_MAX (5.0)
        let sub = get_subject(&conn, "structs")?.expect("structs should exist after insert");
        assert_eq!(sub.mastery_score.get(), crate::constants::MASTERY_MAX);
        Ok(())
    }

    #[test]
    fn test_import_progress_clamps_negative_score() -> Result<()> {
        let mut conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;

        // Create JSON with mastery_score = -2.0 (below MASTERY_MIN = 0.0)
        let json = r#"
        {
            "subjects": [
                {
                    "name": "pointers",
                    "mastery_score": -2.0,
                    "last_practiced_at": null,
                    "attempts_total": 0,
                    "attempts_success": 0,
                    "difficulty_unlocked": 1,
                    "next_review_at": null,
                    "srs_interval_days": 1
                }
            ]
        }
        "#;

        let (count, _warnings) = import_progress(&mut conn, json, true)?;
        assert_eq!(count, 1);

        // Verify the score was clamped to MASTERY_MIN (0.0)
        let sub = get_subject(&conn, "pointers")?.expect("pointers should exist after insert");
        assert_eq!(sub.mastery_score.get(), crate::constants::MASTERY_MIN);
        Ok(())
    }

    #[test]
    fn test_import_progress_preserves_valid_scores() -> Result<()> {
        let mut conn = open_test_db()?;
        ensure_subject(&conn, "memory_allocation")?;

        // Create JSON with a valid mastery_score = 2.5
        let json = r#"
        {
            "subjects": [
                {
                    "name": "memory_allocation",
                    "mastery_score": 2.5,
                    "last_practiced_at": null,
                    "attempts_total": 5,
                    "attempts_success": 3,
                    "difficulty_unlocked": 2,
                    "next_review_at": null,
                    "srs_interval_days": 1
                }
            ]
        }
        "#;

        let (count, _warnings) = import_progress(&mut conn, json, true)?;
        assert_eq!(count, 1);

        // Verify the score was preserved
        let sub = get_subject(&conn, "memory_allocation")?
            .expect("memory_allocation should exist after insert");
        assert_eq!(sub.mastery_score.get(), 2.5);
        assert_eq!(sub.attempts_total, 5);
        Ok(())
    }

    #[test]
    fn test_import_progress_clamps_all_fields() -> Result<()> {
        let mut conn = open_test_db()?;
        ensure_subject(&conn, "signals")?;

        // difficulty_unlocked = 99 (>5), srs_interval_days = 9999 (>60),
        // attempts_total = -3 (<0), attempts_success = 100 (> attempts_total after clamp)
        let json = r#"
        {
            "subjects": [
                {
                    "name": "signals",
                    "mastery_score": 3.0,
                    "last_practiced_at": null,
                    "attempts_total": -3,
                    "attempts_success": 100,
                    "difficulty_unlocked": 99,
                    "next_review_at": null,
                    "srs_interval_days": 9999
                }
            ]
        }
        "#;

        let (count, _warnings) = import_progress(&mut conn, json, true)?;
        assert_eq!(count, 1);

        let sub = get_subject(&conn, "signals")?.expect("signals should exist after insert");
        assert_eq!(sub.difficulty_unlocked, 5, "difficulty clamped to 5");
        assert_eq!(
            sub.srs_interval_days.get(),
            crate::constants::SRS_MAX_INTERVAL_DAYS,
            "srs_interval clamped to SRS_MAX_INTERVAL_DAYS"
        );
        assert_eq!(
            sub.attempts_total, 0,
            "negative attempts_total clamped to 0"
        );
        assert_eq!(
            sub.attempts_success, 0,
            "attempts_success clamped to attempts_total"
        );
        Ok(())
    }

    // ── G2 : last_exam_session KV ─────────────────────────────────────────

    #[test]
    fn test_save_load_last_exam_session_roundtrip() -> Result<()> {
        let conn = open_test_db()?;
        save_last_exam_session(&conn, "nsy103-s1-2024")?;
        let loaded = load_last_exam_session(&conn)?;
        assert_eq!(loaded, Some("nsy103-s1-2024".to_string()));
        Ok(())
    }

    #[test]
    fn test_load_last_exam_session_missing_returns_none() -> Result<()> {
        let conn = open_test_db()?;
        let loaded = load_last_exam_session(&conn)?;
        assert_eq!(loaded, None);
        Ok(())
    }

    // ── G4 : exercise_scores ─────────────────────────────────────────────

    #[test]
    fn test_exercise_scores_upsert() -> Result<()> {
        let conn = open_test_db()?;
        ensure_subject(&conn, "pointers")?;
        record_attempt(&conn, "pointers", "ptr-deref-01", true)?;
        record_attempt(&conn, "pointers", "ptr-deref-01", false)?;
        let scores = get_exercise_scores(&conn, "pointers")?;
        assert_eq!(scores.len(), 1);
        let (id, successes, attempts) = &scores[0];
        assert_eq!(id, "ptr-deref-01");
        assert_eq!(*attempts, 2);
        assert_eq!(*successes, 1);
        Ok(())
    }

    #[test]
    fn test_get_all_weakest_exercises() -> Result<()> {
        let conn = open_test_db()?;
        // Record attempts: subj A has ex_a1 (0/1) and ex_a2 (1/1)
        record_attempt(&conn, "subj_a", "ex_a1", false)?;
        record_attempt(&conn, "subj_a", "ex_a2", true)?;
        record_attempt(&conn, "subj_b", "ex_b1", true)?;
        let map = get_all_weakest_exercises(&conn)?;
        // ex_a1 has lower success rate (0%) than ex_a2 (100%)
        assert_eq!(map.get("subj_a").map(|s| s.as_str()), Some("ex_a1"));
        assert_eq!(map.get("subj_b").map(|s| s.as_str()), Some("ex_b1"));
        Ok(())
    }

    #[test]
    fn test_migrate_v1_idempotent() -> Result<()> {
        let conn = open_test_db()?;
        // open_test_db already runs migrate_v1 via SCHEMA_V1; call again
        migrate_v1(&conn)?;
        migrate_v1(&conn)?;
        // Table still exists and is queryable
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM exercise_scores", [], |r| r.get(0))?;
        assert_eq!(count, 0);
        Ok(())
    }

    #[test]
    fn test_exercise_scores_empty_for_unknown_subject() -> Result<()> {
        let conn = open_test_db()?;
        let scores = get_exercise_scores(&conn, "unknown_subject_xyz")?;
        assert!(scores.is_empty());
        Ok(())
    }

    // ── Last session (launcher "Continue") ──────────────────────────────

    #[test]
    fn test_last_session_roundtrip() -> Result<()> {
        let conn = open_test_db()?;
        save_last_session(&conn, "watch", Some(7), 12)?;
        let loaded = load_last_session(&conn)?;
        assert_eq!(loaded, Some(("watch".to_string(), Some(7), 12)));
        Ok(())
    }

    #[test]
    fn test_last_session_all_chapters() -> Result<()> {
        let conn = open_test_db()?;
        save_last_session(&conn, "piscine", None, 5)?;
        let loaded = load_last_session(&conn)?;
        assert_eq!(loaded, Some(("piscine".to_string(), None, 5)));
        Ok(())
    }

    #[test]
    fn test_last_session_missing_returns_none() -> Result<()> {
        let conn = open_test_db()?;
        let loaded = load_last_session(&conn)?;
        assert_eq!(loaded, None);
        Ok(())
    }
}
