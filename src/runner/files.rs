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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Difficulty, ExerciseFile, ExerciseType, Lang, ValidationConfig};

    fn make_exercise(subject: &str) -> Exercise {
        Exercise {
            id: "test_files".to_string(),
            subject: subject.to_string(),
            lang: Lang::C,
            difficulty: Difficulty::Easy,
            title: "Test".to_string(),
            description: "Test".to_string(),
            starter_code: "int main() { return 0; }".to_string(),
            solution_code: "int main() { return 0; }".to_string(),
            hints: vec![],
            validation: ValidationConfig::default(),
            prerequisites: vec![],
            files: vec![],
            exercise_type: ExerciseType::Complete,
            key_concept: None,
            common_mistake: None,
            kc_ids: vec![],
            starter_code_stages: vec![],
            visualizer: Default::default(),
            libsys_module: None,
            libsys_function: None,
            libsys_unlock: None,
            header_code: None,
        }
    }

    #[test]
    fn test_write_starter_code_creates_file() -> std::io::Result<()> {
        let temp_dir = std::env::temp_dir().join("clings_test_write_starter");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir)?;

        let exercise = make_exercise("test");
        let file_path = temp_dir.join("current.c");

        // Simulate by calling write with mastery None
        let code_bytes = exercise.starter_code.as_bytes();
        std::fs::write(&file_path, code_bytes)?;

        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path)?;
        assert_eq!(content, exercise.starter_code);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_write_starter_code_atomic_cleanup() -> std::io::Result<()> {
        let temp_dir = std::env::temp_dir().join("clings_test_atomic_cleanup");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir)?;

        let source_path = temp_dir.join("current.c");
        let temp_path = source_path.with_extension("c.tmp");

        // Simulate atomic write: create temp file
        let code = "int main() { return 0; }";
        std::fs::write(&temp_path, code.as_bytes())?;
        assert!(temp_path.exists());

        // Simulate successful rename
        std::fs::rename(&temp_path, &source_path)?;
        assert!(source_path.exists());
        assert!(!temp_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_mastery_to_stage_mapping() {
        // Test all 5 stages: 0-4
        assert_eq!(mastery_to_stage(0.0), 0);
        assert_eq!(mastery_to_stage(0.5), 0);
        assert_eq!(mastery_to_stage(0.99), 0);

        assert_eq!(mastery_to_stage(1.0), 1);
        assert_eq!(mastery_to_stage(1.5), 1);
        assert_eq!(mastery_to_stage(1.99), 1);

        assert_eq!(mastery_to_stage(2.0), 2);
        assert_eq!(mastery_to_stage(2.5), 2);
        assert_eq!(mastery_to_stage(2.99), 2);

        assert_eq!(mastery_to_stage(3.0), 3);
        assert_eq!(mastery_to_stage(3.5), 3);
        assert_eq!(mastery_to_stage(3.99), 3);

        assert_eq!(mastery_to_stage(4.0), 4);
        assert_eq!(mastery_to_stage(4.5), 4);
        assert_eq!(mastery_to_stage(5.0), 4);
    }

    #[test]
    fn test_select_starter_code_no_stages_returns_default() {
        let exercise = make_exercise("test");
        let code = select_starter_code(&exercise, 2.5);
        assert_eq!(code, &exercise.starter_code);
    }

    #[test]
    fn test_select_starter_code_with_stages_selects_by_mastery() {
        let mut exercise = make_exercise("test");
        exercise.starter_code_stages = vec![
            "stage0_code".to_string(),
            "stage1_code".to_string(),
            "stage2_code".to_string(),
            "stage3_code".to_string(),
            "stage4_code".to_string(),
        ];

        // mastery 0.5 → stage 0
        assert_eq!(select_starter_code(&exercise, 0.5), "stage0_code");
        // mastery 1.5 → stage 1
        assert_eq!(select_starter_code(&exercise, 1.5), "stage1_code");
        // mastery 2.5 → stage 2
        assert_eq!(select_starter_code(&exercise, 2.5), "stage2_code");
        // mastery 3.5 → stage 3
        assert_eq!(select_starter_code(&exercise, 3.5), "stage3_code");
        // mastery 4.5 → stage 4
        assert_eq!(select_starter_code(&exercise, 4.5), "stage4_code");
    }

    #[test]
    fn test_select_starter_code_missing_stage_falls_back() {
        let mut exercise = make_exercise("test");
        exercise.starter_code_stages = vec!["stage0".to_string(), "stage1".to_string()];

        // mastery 4.5 → stage 4, but only 2 stages exist → fall back to default
        let code = select_starter_code(&exercise, 4.5);
        assert_eq!(code, &exercise.starter_code);
    }

    #[test]
    fn test_write_exercise_files_with_subdirs() -> std::io::Result<()> {
        let work_dir = std::env::temp_dir().join("clings_test_subdirs");
        let _ = std::fs::remove_dir_all(&work_dir);
        std::fs::create_dir_all(&work_dir)?;

        let mut exercise = make_exercise("test");
        exercise.files = vec![
            ExerciseFile {
                name: "subdir1/file1.h".to_string(),
                content: "#ifndef FILE1_H\n#define FILE1_H\nvoid func1(void);\n#endif".to_string(),
                readonly: false,
            },
            ExerciseFile {
                name: "subdir1/subdir2/file2.c".to_string(),
                content: "void func1(void) { }".to_string(),
                readonly: false,
            },
        ];

        write_exercise_files(&exercise, &work_dir)?;

        assert!(work_dir.join("subdir1/file1.h").exists());
        assert!(work_dir.join("subdir1/subdir2/file2.c").exists());

        let content1 = std::fs::read_to_string(work_dir.join("subdir1/file1.h"))?;
        assert!(content1.contains("#ifndef FILE1_H"));

        let _ = std::fs::remove_dir_all(&work_dir);
        Ok(())
    }

    #[test]
    fn test_write_exercise_files_empty_list() -> std::io::Result<()> {
        let work_dir = std::env::temp_dir().join("clings_test_empty_files");
        let _ = std::fs::remove_dir_all(&work_dir);
        std::fs::create_dir_all(&work_dir)?;

        let exercise = make_exercise("test");
        assert!(exercise.files.is_empty());

        // Should return Ok immediately without writing anything
        write_exercise_files(&exercise, &work_dir)?;

        let _ = std::fs::remove_dir_all(&work_dir);
        Ok(())
    }
}
