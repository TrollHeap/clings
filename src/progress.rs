use chrono::Utc;
use rusqlite::{params, Connection};

use crate::mastery;
use crate::models::Subject;

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
";

/// Open (or create) the progress database.
pub fn open_db() -> Result<Connection, String> {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".kernelforge");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create dir: {e}"))?;

    let db_path = dir.join("progress.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("Cannot open DB: {e}"))?;

    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(|e| format!("Pragma error: {e}"))?;

    conn.execute_batch(SCHEMA)
        .map_err(|e| format!("Schema error: {e}"))?;

    Ok(conn)
}

/// Ensure a subject row exists in the DB.
pub fn ensure_subject(conn: &Connection, name: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO subjects (name) VALUES (?1)",
        params![name],
    )
    .map_err(|e| format!("Insert subject error: {e}"))?;
    Ok(())
}

/// Get all subjects from DB.
pub fn get_all_subjects(conn: &Connection) -> Result<Vec<Subject>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT name, mastery_score, last_practiced_at, attempts_total, attempts_success,
                    difficulty_unlocked, next_review_at, srs_interval_days
             FROM subjects ORDER BY name",
        )
        .map_err(|e| format!("Query error: {e}"))?;

    let subjects = stmt
        .query_map([], |row| {
            Ok(Subject {
                name: row.get(0)?,
                mastery_score: row.get(1)?,
                last_practiced_at: row.get(2)?,
                attempts_total: row.get(3)?,
                attempts_success: row.get(4)?,
                difficulty_unlocked: row.get(5)?,
                next_review_at: row.get(6)?,
                srs_interval_days: row.get::<_, Option<i64>>(7)?.unwrap_or(1),
            })
        })
        .map_err(|e| format!("Query map error: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Collect error: {e}"))?;

    Ok(subjects)
}

/// Record a practice attempt and update mastery.
pub fn record_attempt(
    conn: &Connection,
    subject: &str,
    exercise_id: &str,
    success: bool,
) -> Result<Subject, String> {
    let now = Utc::now().timestamp();

    ensure_subject(conn, subject)?;

    // Log the attempt
    let log_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![log_id, subject, exercise_id, success as i32, now],
    )
    .map_err(|e| format!("Log insert error: {e}"))?;

    // Load current subject
    let mut sub = get_subject(conn, subject)?.unwrap_or_else(|| Subject::new(subject.to_string()));

    // Apply mastery update
    mastery::update_mastery(&mut sub, success);

    // Compute SRS
    let (next_review, new_interval) =
        mastery::compute_next_review(sub.srs_interval_days, success, now);
    sub.next_review_at = Some(next_review);
    sub.srs_interval_days = new_interval;

    // Persist
    conn.execute(
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
            sub.mastery_score,
            sub.last_practiced_at,
            sub.attempts_total,
            sub.attempts_success,
            sub.difficulty_unlocked,
            sub.next_review_at,
            sub.srs_interval_days,
        ],
    )
    .map_err(|e| format!("Update subject error: {e}"))?;

    Ok(sub)
}

/// Get a single subject.
pub fn get_subject(conn: &Connection, name: &str) -> Result<Option<Subject>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT name, mastery_score, last_practiced_at, attempts_total, attempts_success,
                    difficulty_unlocked, next_review_at, srs_interval_days
             FROM subjects WHERE name = ?1",
        )
        .map_err(|e| format!("Query error: {e}"))?;

    let subject = stmt
        .query_row(params![name], |row| {
            Ok(Subject {
                name: row.get(0)?,
                mastery_score: row.get(1)?,
                last_practiced_at: row.get(2)?,
                attempts_total: row.get(3)?,
                attempts_success: row.get(4)?,
                difficulty_unlocked: row.get(5)?,
                next_review_at: row.get(6)?,
                srs_interval_days: row.get::<_, Option<i64>>(7)?.unwrap_or(1),
            })
        })
        .ok();

    Ok(subject)
}

/// Get current streak (consecutive days with at least one practice).
pub fn get_streak(conn: &Connection) -> Result<i64, String> {
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT date(practiced_at, 'unixepoch') as day
             FROM practice_log
             ORDER BY day DESC",
        )
        .map_err(|e| format!("Streak query error: {e}"))?;

    let days: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| format!("Streak map error: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Streak collect error: {e}"))?;

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

/// Reset all progress (with confirmation handled by caller).
pub fn reset_progress(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "DELETE FROM practice_log;
         DELETE FROM subjects;",
    )
    .map_err(|e| format!("Reset error: {e}"))?;
    Ok(())
}

/// Apply decay to all subjects.
pub fn apply_all_decay(conn: &Connection) -> Result<(), String> {
    let mut subjects = get_all_subjects(conn)?;
    for sub in &mut subjects {
        let old_score = sub.mastery_score;
        mastery::apply_decay(sub);
        if sub.mastery_score != old_score {
            conn.execute(
                "UPDATE subjects SET mastery_score = ?2 WHERE name = ?1",
                params![sub.name, sub.mastery_score],
            )
            .map_err(|e| format!("Decay update error: {e}"))?;
        }
    }
    Ok(())
}
