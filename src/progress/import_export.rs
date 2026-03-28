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
