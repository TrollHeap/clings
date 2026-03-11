use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use serde::Deserialize;

use crate::constants::{
    CLINGS_DIR, DB_BUSY_TIMEOUT_MS, DB_FILENAME, EXAM_CHECKPOINT_KEY, PISCINE_CHECKPOINT_KEY,
};
use crate::error::Result;
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

    conn.execute_batch(&format!(
        "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout={DB_BUSY_TIMEOUT_MS};"
    ))?;
    conn.execute_batch(SCHEMA)?;

    Ok(conn)
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
        mastery_score: row.get(1)?,
        last_practiced_at: row.get(2)?,
        attempts_total: row.get(3)?,
        attempts_success: row.get(4)?,
        difficulty_unlocked: row.get(5)?,
        next_review_at: row.get(6)?,
        srs_interval_days: row.get::<_, i64>(7)?,
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
        mastery::compute_next_review(sub.srs_interval_days, success, now);
    sub.next_review_at = Some(next_review);
    sub.srs_interval_days = new_interval;

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
            sub.mastery_score,
            sub.last_practiced_at,
            sub.attempts_total,
            sub.attempts_success,
            sub.difficulty_unlocked,
            sub.next_review_at,
            sub.srs_interval_days,
        ],
    )?;

    tx.commit()?;
    Ok(sub)
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
         ORDER BY day DESC",
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
    conn.execute_batch(
        "DELETE FROM kv;
         DELETE FROM practice_log;
         DELETE FROM subjects;",
    )?;
    Ok(())
}

/// Open an in-memory database for testing.
#[cfg(test)]
fn open_test_db() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA)?;
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
    Ok(kv_get(conn, PISCINE_CHECKPOINT_KEY)?.and_then(|s| s.parse().ok()))
}

/// Clear piscine checkpoint (called when piscine is fully completed).
pub fn clear_piscine_checkpoint(conn: &Connection) -> Result<()> {
    kv_del(conn, PISCINE_CHECKPOINT_KEY)
}

/// Save exam checkpoint: stores "{session_id}:{index}" under exam_checkpoint key.
pub fn save_exam_checkpoint(conn: &Connection, session_id: &str, index: usize) -> Result<()> {
    kv_set(conn, EXAM_CHECKPOINT_KEY, &format!("{session_id}:{index}"))
}

/// Load exam checkpoint for the given session_id. Returns None if no checkpoint exists or if the
/// stored session differs (i.e. the user switched to a different exam session).
pub fn load_exam_checkpoint(conn: &Connection, session_id: &str) -> Result<Option<usize>> {
    Ok(kv_get(conn, EXAM_CHECKPOINT_KEY)?.and_then(|s| {
        s.rsplit_once(':')
            .filter(|(sid, _)| *sid == session_id)
            .and_then(|(_, rest)| rest.parse().ok())
    }))
}

/// Clear exam checkpoint (called when exam session is fully completed).
pub fn clear_exam_checkpoint(conn: &Connection) -> Result<()> {
    kv_del(conn, EXAM_CHECKPOINT_KEY)
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

/// Retourne l'exercise_id raté le plus récemment pour un sujet donné.
/// Utilisé par `clings review` pour cibler les exercices échoués.
/// Retourne None si aucun échec n'est enregistré pour ce sujet.
pub fn get_failed_exercise(conn: &Connection, subject: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT exercise_id FROM practice_log
         WHERE subject = ?1 AND success = 0
         ORDER BY practiced_at DESC
         LIMIT 1",
    )?;
    let result = stmt
        .query_row([subject], |row| row.get::<_, String>(0))
        .optional()?;
    Ok(result)
}

/// Apply decay to all subjects (batched in single transaction).
pub fn apply_all_decay(conn: &mut Connection) -> Result<()> {
    let mut subjects = get_all_subjects(conn)?;
    let tx = conn.transaction()?;
    for sub in &mut subjects {
        let old_score = sub.mastery_score;
        mastery::apply_decay(sub);
        if sub.mastery_score != old_score {
            tx.execute(
                "UPDATE subjects SET mastery_score = ?2 WHERE name = ?1",
                params![sub.name, sub.mastery_score],
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
/// Retourne le nombre de sujets importés/mis à jour.
pub fn import_progress(conn: &mut Connection, json: &str, overwrite: bool) -> Result<usize> {
    #[derive(Deserialize)]
    struct ImportData {
        subjects: Vec<Subject>,
    }

    let data: ImportData = serde_json::from_str(json)
        .map_err(|e| crate::error::KfError::Config(format!("invalid JSON: {e}")))?;

    let tx = conn.transaction()?;
    let mut count = 0usize;

    for sub in &data.subjects {
        if overwrite {
            tx.execute(
                "INSERT OR REPLACE INTO subjects
                 (name, mastery_score, last_practiced_at, attempts_total, attempts_success,
                  difficulty_unlocked, next_review_at, srs_interval_days)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
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
                    sub.mastery_score,
                    sub.last_practiced_at,
                    sub.attempts_total,
                    sub.attempts_success,
                    sub.difficulty_unlocked,
                    sub.next_review_at,
                    sub.srs_interval_days,
                ],
            )?;
        }
        count += 1;
    }

    tx.commit()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_subject_creates_row() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        let sub = get_subject(&conn, "pointers").unwrap();
        assert!(sub.is_some());
        let sub = sub.unwrap();
        assert_eq!(sub.name, "pointers");
        assert_eq!(sub.mastery_score, 0.0);
        assert_eq!(sub.difficulty_unlocked, 1);
    }

    #[test]
    fn test_ensure_subject_idempotent() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        let subjects = get_all_subjects(&conn).unwrap();
        assert_eq!(subjects.len(), 1);
    }

    #[test]
    fn test_record_attempt_success() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "structs").unwrap();
        let sub = record_attempt(&conn, "structs", "struct-point-01", true).unwrap();
        assert_eq!(sub.mastery_score, 1.0);
        assert_eq!(sub.attempts_total, 1);
        assert_eq!(sub.attempts_success, 1);
    }

    #[test]
    fn test_record_attempt_failure() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "structs").unwrap();

        // First succeed to have score > 0
        record_attempt(&conn, "structs", "struct-point-01", true).unwrap();
        let sub = record_attempt(&conn, "structs", "struct-point-01", false).unwrap();
        assert_eq!(sub.mastery_score, 0.5);
        assert_eq!(sub.attempts_total, 2);
        assert_eq!(sub.attempts_success, 1);
    }

    #[test]
    fn test_reset_progress() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        record_attempt(&conn, "pointers", "ptr-deref-01", true).unwrap();
        reset_progress(&conn).unwrap();
        let subjects = get_all_subjects(&conn).unwrap();
        assert!(subjects.is_empty());
    }

    #[test]
    fn test_get_subject_missing() {
        let conn = open_test_db().unwrap();
        let sub = get_subject(&conn, "nonexistent").unwrap();
        assert!(sub.is_none());
    }

    #[test]
    fn test_reset_subject_isolated() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        ensure_subject(&conn, "structs").unwrap();
        record_attempt(&conn, "pointers", "ptr-deref-01", true).unwrap();
        record_attempt(&conn, "structs", "struct-point-01", true).unwrap();

        reset_subject(&conn, "pointers").unwrap();

        // pointers supprimé
        assert!(get_subject(&conn, "pointers").unwrap().is_none());
        // structs intact
        let s = get_subject(&conn, "structs").unwrap().unwrap();
        assert_eq!(s.mastery_score, 1.0);
        // log pointers supprimé, log structs intact
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM practice_log WHERE subject = 'structs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let count_ptr: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM practice_log WHERE subject = 'pointers'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_ptr, 0);
    }

    #[test]
    fn test_kv_checkpoint_roundtrip() {
        let conn = open_test_db().unwrap();
        save_piscine_checkpoint(&conn, 42).unwrap();
        let loaded = load_piscine_checkpoint(&conn).unwrap();
        assert_eq!(loaded, Some(42));
    }

    #[test]
    fn test_kv_checkpoint_missing_returns_none() {
        let conn = open_test_db().unwrap();
        let loaded = load_piscine_checkpoint(&conn).unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn test_kv_checkpoint_clear() {
        let conn = open_test_db().unwrap();
        save_piscine_checkpoint(&conn, 7).unwrap();
        clear_piscine_checkpoint(&conn).unwrap();
        let loaded = load_piscine_checkpoint(&conn).unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn test_kv_checkpoint_overwrite() {
        let conn = open_test_db().unwrap();
        save_piscine_checkpoint(&conn, 3).unwrap();
        save_piscine_checkpoint(&conn, 17).unwrap();
        let loaded = load_piscine_checkpoint(&conn).unwrap();
        assert_eq!(loaded, Some(17));
    }

    #[test]
    fn test_exam_checkpoint_roundtrip() {
        let conn = open_test_db().unwrap();
        save_exam_checkpoint(&conn, "nsy103-2024", 5).unwrap();
        let loaded = load_exam_checkpoint(&conn, "nsy103-2024").unwrap();
        assert_eq!(loaded, Some(5));
    }

    #[test]
    fn test_exam_checkpoint_session_isolation() {
        let conn = open_test_db().unwrap();
        save_exam_checkpoint(&conn, "nsy103-2024", 3).unwrap();
        let other = load_exam_checkpoint(&conn, "utc502-2023").unwrap();
        assert_eq!(other, None);
    }

    #[test]
    fn test_exam_checkpoint_session_id_with_colon() {
        let conn = open_test_db().unwrap();
        save_exam_checkpoint(&conn, "utc502:2024", 7).unwrap();
        let loaded = load_exam_checkpoint(&conn, "utc502:2024").unwrap();
        assert_eq!(loaded, Some(7));
        // A session_id that only matches the prefix must not match
        let wrong = load_exam_checkpoint(&conn, "utc502").unwrap();
        assert_eq!(wrong, None);
    }

    #[test]
    fn test_exam_checkpoint_clear() {
        let conn = open_test_db().unwrap();
        save_exam_checkpoint(&conn, "nsy103-2024", 2).unwrap();
        clear_exam_checkpoint(&conn).unwrap();
        let loaded = load_exam_checkpoint(&conn, "nsy103-2024").unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn test_due_subjects_past_review() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        // next_review_at dans le passé → sujet doit apparaître
        let past = Utc::now().timestamp() - 3_600;
        conn.execute(
            "UPDATE subjects SET next_review_at = ?1 WHERE name = 'pointers'",
            params![past],
        )
        .unwrap();
        let due = get_due_subjects(&conn).unwrap();
        assert!(due.contains(&"pointers".to_string()));
    }

    #[test]
    fn test_due_subjects_future_review() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        // next_review_at dans le futur → sujet absent
        let future = Utc::now().timestamp() + 86_400;
        conn.execute(
            "UPDATE subjects SET next_review_at = ?1 WHERE name = 'pointers'",
            params![future],
        )
        .unwrap();
        let due = get_due_subjects(&conn).unwrap();
        assert!(!due.contains(&"pointers".to_string()));
    }

    #[test]
    fn test_due_subjects_null_review() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        // next_review_at NULL par défaut → sujet absent
        let due = get_due_subjects(&conn).unwrap();
        assert!(!due.contains(&"pointers".to_string()));
    }

    #[test]
    fn test_get_streak_empty() {
        let conn = open_test_db().unwrap();
        assert_eq!(get_streak(&conn).unwrap(), 0);
    }

    #[test]
    fn test_get_streak_today() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at) VALUES ('t1', 'pointers', 'ex1', 1, ?1)",
            params![now],
        ).unwrap();
        assert_eq!(get_streak(&conn).unwrap(), 1);
    }

    #[test]
    fn test_get_streak_consecutive_days() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        let now = Utc::now().timestamp();
        for (i, id) in ["c1", "c2", "c3"].iter().enumerate() {
            let ts = now - (i as i64) * 86_400;
            conn.execute(
                "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at) VALUES (?1, 'pointers', 'ex1', 1, ?2)",
                params![id, ts],
            ).unwrap();
        }
        assert_eq!(get_streak(&conn).unwrap(), 3);
    }

    #[test]
    fn test_get_streak_broken() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pointers").unwrap();
        let now = Utc::now().timestamp();
        // Aujourd'hui et il y a 3 jours (pas hier) → streak = 1
        for (id, offset) in [("b1", 0i64), ("b2", 3)] {
            conn.execute(
                "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at) VALUES (?1, 'pointers', 'ex1', 1, ?2)",
                params![id, now - offset * 86_400],
            ).unwrap();
        }
        assert_eq!(get_streak(&conn).unwrap(), 1);
    }

    #[test]
    fn test_apply_all_decay_updates_db() {
        let mut conn = open_test_db().unwrap();
        ensure_subject(&conn, "structs").unwrap();
        let old_ts = Utc::now().timestamp() - 15 * 86_400;
        conn.execute(
            "UPDATE subjects SET mastery_score = 2.0, last_practiced_at = ?1 WHERE name = 'structs'",
            params![old_ts],
        ).unwrap();
        apply_all_decay(&mut conn).unwrap();
        let sub = get_subject(&conn, "structs").unwrap().unwrap();
        assert_eq!(sub.mastery_score, 1.5);
    }

    #[test]
    fn test_apply_all_decay_no_change_when_recent() {
        let mut conn = open_test_db().unwrap();
        ensure_subject(&conn, "pipes").unwrap();
        let recent_ts = Utc::now().timestamp() - 5 * 86_400;
        conn.execute(
            "UPDATE subjects SET mastery_score = 3.0, last_practiced_at = ?1 WHERE name = 'pipes'",
            params![recent_ts],
        )
        .unwrap();
        apply_all_decay(&mut conn).unwrap();
        let sub = get_subject(&conn, "pipes").unwrap().unwrap();
        assert_eq!(sub.mastery_score, 3.0);
    }

    #[test]
    fn test_record_attempt_persists_srs_fields() {
        let conn = open_test_db().unwrap();
        ensure_subject(&conn, "pipes").unwrap();
        let sub = record_attempt(&conn, "pipes", "pipe-01", true).unwrap();
        // Après un succès, intervalle SRS = round(1 * 2.5) = 3
        assert_eq!(sub.srs_interval_days, 3);
        assert!(sub.next_review_at.is_some());
        // Vérifier la persistance en DB
        let reloaded = get_subject(&conn, "pipes").unwrap().unwrap();
        assert_eq!(reloaded.srs_interval_days, sub.srs_interval_days);
        assert_eq!(reloaded.next_review_at, sub.next_review_at);
    }
}
