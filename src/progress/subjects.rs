//! Subject mastery CRUD, SRS queries, and practice log management.

use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::constants::SECS_PER_DAY;
use crate::error::Result;
use crate::mastery;
use crate::models::{MasteryScore, SrsIntervalDays, Subject};

/// Monotonic counter for unique practice_log IDs within a process.
static LOG_SEQ: AtomicU64 = AtomicU64::new(0);

const PRACTICE_LOG_MAX_ENTRIES: usize = 10_000;

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

/// Metadata for a practice log entry.
#[derive(Default)]
struct PracticeLogMeta {
    error_type: Option<String>,
    duration_ms: Option<u64>,
    hint_count_used: u32,
}

/// Insert a practice log entry into the database.
fn insert_practice_log(
    tx: &rusqlite::Transaction,
    log_id: &str,
    subject: &str,
    exercise_id: &str,
    success: bool,
    practiced_at: i64,
    meta: &PracticeLogMeta,
) -> Result<()> {
    tx.execute(
        "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at, error_type, duration_ms, hint_count_used)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            log_id,
            subject,
            exercise_id,
            success as i32,
            practiced_at,
            &meta.error_type,
            meta.duration_ms.map(|d| d as i64),
            meta.hint_count_used as i32
        ],
    )?;
    Ok(())
}

/// Upsert subject mastery record with updated SRS state.
fn upsert_subject_mastery(tx: &rusqlite::Transaction, sub: &Subject) -> Result<()> {
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
    Ok(())
}

/// Applique une mise à jour de mastery et SRS après un tentative de pratique.
/// Retourne le score de mastery mis à jour.
fn apply_mastery_update(
    tx: &rusqlite::Transaction,
    subject: &str,
    success: bool,
    now: i64,
) -> Result<f64> {
    let mut sub = get_subject(tx, subject)?.unwrap_or_else(|| Subject::new(subject.to_string()));
    mastery::update_mastery(&mut sub, success);

    let (next_review, new_interval) =
        mastery::compute_next_review(sub.srs_interval_days.get(), success, now);
    sub.next_review_at = Some(next_review);
    sub.srs_interval_days = SrsIntervalDays::clamped(new_interval);

    upsert_subject_mastery(tx, &sub)?;
    Ok(sub.mastery_score.get())
}

/// Record a practice attempt and update mastery.
///
/// All writes (ensure subject, log entry, mastery update) are wrapped in a
/// transaction so a crash mid-way never leaves the DB in an inconsistent state.
pub fn record_attempt(
    conn: &Connection,
    subject: &str,
    exercise_id: &str,
    success: bool,
) -> Result<Subject> {
    let now = Utc::now().timestamp();
    let seq = LOG_SEQ.fetch_add(1, Ordering::Relaxed);
    let log_id = format!("{}-{}-{}", now, std::process::id(), seq);

    let tx = conn.unchecked_transaction()?;

    tx.execute(
        "INSERT OR IGNORE INTO subjects (name) VALUES (?1)",
        params![subject],
    )?;

    insert_practice_log(
        &tx,
        &log_id,
        subject,
        exercise_id,
        success,
        now,
        &PracticeLogMeta::default(),
    )?;

    apply_mastery_update(&tx, subject, success, now)?;
    let sub = get_subject(&tx, subject)?.unwrap_or_else(|| Subject::new(subject.to_string()));

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
    truncate_practice_log_to_max_entries(conn)?;
    Ok(sub)
}

/// Supprime les lignes de `practice_log` au-delà des PRACTICE_LOG_MAX_ENTRIES plus récentes.
/// Sans effet si le nombre de lignes est inférieur à ce seuil.
fn truncate_practice_log_to_max_entries(conn: &Connection) -> Result<()> {
    let offset = PRACTICE_LOG_MAX_ENTRIES - 1;
    conn.execute(
        &format!(
            "DELETE FROM practice_log
             WHERE practiced_at < (
                 SELECT practiced_at FROM practice_log
                 ORDER BY practiced_at DESC LIMIT 1 OFFSET {}
             )",
            offset
        ),
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

    let days: Vec<chrono::NaiveDate> = stmt
        .query_map([], |row| {
            let s: String = row.get(0)?;
            chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if days.is_empty() {
        return Ok(0);
    }

    let today = Utc::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);

    if days[0] != today && days[0] != yesterday {
        return Ok(0);
    }

    let mut streak = 1i64;
    for window in days.windows(2) {
        if (window[0] - window[1]).num_days() == 1 {
            streak += 1;
        } else {
            break;
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
