//! Exercise file I/O, working directory management, and starter code selection.

use std::path::{Path, PathBuf};

use crate::constants::CURRENT_C_FILENAME;
use crate::error::KfError;
use crate::models::Exercise;

/// Write exercise files (headers etc.) to a temp directory.
pub fn write_exercise_files(exercise: &Exercise, work_dir: &Path) -> std::io::Result<()> {
    if exercise.files.is_empty() {
        return Ok(());
    }
    let canonical_work = work_dir.canonicalize()?;
    for file in &exercise.files {
        if file.name.contains("..") || file.name.starts_with('/') {
            eprintln!(
                "  Avertissement : fichier ignoré (chemin invalide) : {}",
                file.name
            );
            continue;
        }
        let file_path = work_dir.join(&file.name);
        if let Some(parent) = file_path.parent() {
            // Pre-flight lexical check (no I/O): reject path traversal before
            // creating any directories on disk.
            if !parent.starts_with(work_dir) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Fichier hors répertoire de travail : {}", file.name),
                ));
            }
            std::fs::create_dir_all(parent)?;
            // Post-creation canonical check: catch symlink-based traversal.
            let canonical_parent = parent.canonicalize()?;
            if !canonical_parent.starts_with(&canonical_work) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Fichier hors répertoire de travail : {}", file.name),
                ));
            }
        }
        std::fs::write(&file_path, &file.content)?;
    }
    Ok(())
}

/// Get the working directory for exercises.
pub fn work_dir() -> crate::error::Result<PathBuf> {
    let dir = crate::constants::clings_data_dir();
    #[cfg(unix)]
    {
        use std::fs::DirBuilder;
        use std::os::unix::fs::DirBuilderExt;
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(KfError::Io)?;
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(&dir).map_err(KfError::Io)?;
    }
    Ok(dir)
}

/// Map mastery score to stage index (0-4).
#[must_use]
pub fn mastery_to_stage(mastery: f64) -> u8 {
    match mastery {
        m if m < 1.0 => 0,
        m if m < 2.0 => 1,
        m if m < 3.0 => 2,
        m if m < 4.0 => 3,
        _ => 4,
    }
}

/// Select the appropriate starter code stage based on mastery.
///
/// Returns the stage code from `starter_code_stages`, or the default starter code
/// if:
/// - No `starter_code_stages` defined
/// - Mastery-computed stage exceeds available stages
#[must_use]
pub fn select_starter_code(exercise: &Exercise, mastery: f64) -> &str {
    let stage = mastery_to_stage(mastery) as usize;
    exercise
        .starter_code_stages
        .get(stage)
        .map(|s| s.as_str())
        .unwrap_or(&exercise.starter_code)
}

/// Charge la mastery du sujet depuis la DB, sélectionne le stage de code (0–4),
/// et écrit le starter code correspondant.
///
/// # Returns
/// `(source_path, current_stage)` — `current_stage` est `None` si l'exercice
/// n'a pas de staged_code.
///
/// # Errors
/// - `KfError::Database` if database query fails.
/// - `KfError::Io` if `write_starter_code` encounters an I/O error (auto-converted via `#[from]`).
pub fn prepare_exercise_source(
    conn: &rusqlite::Connection,
    exercise: &crate::models::Exercise,
) -> crate::error::Result<(std::path::PathBuf, Option<u8>)> {
    let subject_mastery =
        crate::progress::get_subject(conn, &exercise.subject)?.map(|s| s.mastery_score.get());
    let current_stage = subject_mastery.map(mastery_to_stage);
    let source_path = write_starter_code(exercise, subject_mastery)?;
    Ok((source_path, current_stage))
}

/// Utilise `work_dir()` pour résoudre le répertoire, écrit les fichiers auxiliaires.
///
/// # Errors
/// Retourne `std::io::Error` si :
/// - `work_dir()` échoue (HOME absent ou répertoire inaccessible)
/// - write/rename du fichier temporaire échoue
pub fn write_starter_code(exercise: &Exercise, mastery: Option<f64>) -> std::io::Result<PathBuf> {
    let dir = work_dir().map_err(|e| match e {
        KfError::Io(io) => io,
        other => std::io::Error::other(other.to_string()),
    })?;
    let source_path = dir.join(CURRENT_C_FILENAME);
    let code = match mastery {
        Some(m) => select_starter_code(exercise, m),
        None => &exercise.starter_code,
    };
    // Atomic write: temp file + rename (POSIX guarantee, no corruption window)
    let temp_path = source_path.with_extension("c.tmp");
    std::fs::write(&temp_path, code.as_bytes())?;
    if let Err(e) = std::fs::rename(&temp_path, &source_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }
    Ok(source_path)
}
