//! Progress import and export — JSON serialization/deserialization for cross-machine sync.

use rusqlite::{params, Connection};
use serde::Deserialize;

use super::get_all_subjects;
use crate::error::Result;
use crate::models::Subject;

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

/// Valide et clamp un sujet importé, retournant les valeurs clamped et les avertissements.
fn clamp_and_warn_import_subject(sub: &Subject) -> (f64, i32, i64, i64, i64, Vec<String>) {
    let clamped_score = sub.mastery_score.get();
    let clamped_difficulty = sub.difficulty_unlocked.clamp(1, 5);
    let clamped_interval = sub.srs_interval_days.get().clamp(
        crate::constants::SRS_BASE_INTERVAL_DAYS,
        crate::constants::SRS_MAX_INTERVAL_DAYS,
    );
    let clamped_total = sub.attempts_total.max(0);
    let clamped_success = sub.attempts_success.max(0).min(clamped_total);

    let mut warnings: Vec<String> = Vec::new();

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

    (
        clamped_score,
        clamped_difficulty,
        clamped_total,
        clamped_success,
        clamped_interval,
        warnings,
    )
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
        let (
            clamped_score,
            clamped_difficulty,
            clamped_total,
            clamped_success,
            clamped_interval,
            sub_warnings,
        ) = clamp_and_warn_import_subject(sub);
        warnings.extend(sub_warnings);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_db() -> Result<Connection> {
        crate::progress::open_db_for_test()
    }

    #[test]
    fn test_export_empty_db() -> Result<()> {
        let conn = open_test_db()?;
        let json = export_progress(&conn)?;
        assert!(json.contains("subjects") && json.contains("[]"));
        assert!(json.contains("version") && json.contains("1"));
        Ok(())
    }

    #[test]
    fn test_export_with_subjects() -> Result<()> {
        let conn = open_test_db()?;
        crate::progress::ensure_subject_for_test(&conn, "pointers")?;
        crate::progress::record_attempt(&conn, "pointers", "ptr-01", true)?;

        let json = export_progress(&conn)?;
        assert!(json.contains("pointers"));
        assert!(json.contains("version"));

        // Validate it's valid JSON
        let _: serde_json::Value = serde_json::from_str(&json)?;
        Ok(())
    }

    #[test]
    fn test_roundtrip_export_import() -> Result<()> {
        let mut conn = open_test_db()?;
        crate::progress::ensure_subject_for_test(&conn, "structs")?;
        crate::progress::record_attempt(&conn, "structs", "struct-01", true)?;
        crate::progress::record_attempt(&conn, "structs", "struct-02", false)?;

        // Export
        let json = export_progress(&conn)?;

        // Import into fresh DB (overwrite mode)
        let mut conn2 = open_test_db()?;
        let (count, warnings) = import_progress(&mut conn2, &json, true)?;

        assert_eq!(count, 1, "should import 1 subject");
        assert!(warnings.is_empty(), "no warnings for valid data");

        // Verify the imported subject
        let sub = crate::progress::get_subject(&conn2, "structs")?;
        assert!(sub.is_some());
        let sub = sub.unwrap();
        assert_eq!(sub.name, "structs");
        assert_eq!(sub.mastery_score.get(), 0.5); // 1 success, 1 failure
        assert_eq!(sub.attempts_total, 2);
        assert_eq!(sub.attempts_success, 1);
        Ok(())
    }

    #[test]
    fn test_import_malformed_json_error() -> Result<()> {
        let mut conn = open_test_db()?;
        let invalid_json = r#"{ "subjects": [not valid json ] }"#;

        let result = import_progress(&mut conn, invalid_json, false);
        assert!(result.is_err(), "should reject malformed JSON");

        let err = result.unwrap_err();
        assert!(err.to_string().contains("invalid JSON"));
        Ok(())
    }

    #[test]
    fn test_import_missing_subjects_field() -> Result<()> {
        let mut conn = open_test_db()?;
        let no_subjects = r#"{ "version": 1 }"#;

        let result = import_progress(&mut conn, no_subjects, false);
        assert!(result.is_err(), "should reject JSON without subjects field");
        Ok(())
    }

    #[test]
    fn test_import_clamping_warnings() -> Result<()> {
        let mut conn = open_test_db()?;

        // Manually construct a Subject with out-of-bounds values
        let json = r#"{
            "subjects": [
                {
                    "name": "test_clamp",
                    "mastery_score": 2.5,
                    "last_practiced_at": null,
                    "attempts_total": -10,
                    "attempts_success": 20,
                    "difficulty_unlocked": 100,
                    "next_review_at": null,
                    "srs_interval_days": 999999
                }
            ]
        }"#;

        let (_count, warnings) = import_progress(&mut conn, json, false)?;
        // Should warn about clamped values
        let warnings_str = format!("{:?}", warnings);
        assert!(
            warnings_str.contains("difficulty_unlocked") || warnings_str.contains("attempts"),
            "should warn about out-of-bounds values: {}",
            warnings_str
        );
        Ok(())
    }

    #[test]
    fn test_import_no_overwrite_merges_mastery() -> Result<()> {
        let mut conn = open_test_db()?;

        // Insert initial subject with mastery 2.0
        crate::progress::ensure_subject_for_test(&conn, "merge_test")?;
        {
            let mut stmt =
                conn.prepare("UPDATE subjects SET mastery_score = ?1 WHERE name = ?2")?;
            stmt.execute(rusqlite::params![2.0, "merge_test"])?;
        } // Drop statement before mutable borrow

        // Import with lower mastery (1.0), no overwrite
        let json = r#"{
            "subjects": [
                {
                    "name": "merge_test",
                    "mastery_score": 1.0,
                    "last_practiced_at": null,
                    "attempts_total": 0,
                    "attempts_success": 0,
                    "difficulty_unlocked": 1,
                    "next_review_at": null,
                    "srs_interval_days": 1
                }
            ]
        }"#;

        import_progress(&mut conn, json, false)?;

        // Should keep max (2.0)
        let sub = crate::progress::get_subject(&conn, "merge_test")?.expect("subject should exist");
        assert_eq!(sub.mastery_score.get(), 2.0);
        Ok(())
    }
}
