//! Exercise loader — discovers and parses JSON exercise files.
//!
//! Resolution order: `CLINGS_EXERCISES` env var → embedded binary data.
//! Each exercise is a JSON file under `exercises/<subject>/`.
//! Also loads `annales_map.json` for past exam mappings.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::constants::EXERCISES_ENV_VAR;
use crate::error::{KfError, Result};
use crate::models::Exercise;

/// Exercises embedded at compile time from the `exercises/` directory.
#[derive(rust_embed::RustEmbed)]
#[folder = "exercises/"]
struct EmbeddedExercises;

/// Ensemble d'exercices : liste complète + index par sujet (indices dans le Vec).
pub type ExerciseSet = (Vec<Exercise>, HashMap<String, Vec<usize>>);

/// Recursively load all exercise JSON files from a directory.
fn load_exercises_from_dir(dir: &Path) -> Vec<Exercise> {
    let mut exercises = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!(
                "Avertissement : impossible de lire le répertoire {} : {e}",
                dir.display()
            );
            return exercises;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("Avertissement : entrée de répertoire ignorée : {err}");
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            exercises.extend(load_exercises_from_dir(&path));
        } else if path.extension().is_some_and(|e| e == "json")
            && path.file_name() != Some(std::ffi::OsStr::new("kc_error_map.json"))
            && path.file_name() != Some(std::ffi::OsStr::new("annales_map.json"))
        {
            if let Ok(content) = std::fs::read_to_string(&path) {
                match serde_json::from_str::<Exercise>(&content) {
                    Ok(exercise) => exercises.push(exercise),
                    Err(e) => eprintln!(
                        "Avertissement : échec d'analyse de {} : {}",
                        path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                        e
                    ),
                }
            }
        }
    }
    exercises
}

/// Resolve the exercises directory path.
/// Priority: CLINGS_EXERCISES env var > auto-detect exercises/ relative to binary or CWD
pub fn resolve_exercises_dir() -> Result<PathBuf> {
    if let Ok(env_path) = std::env::var(EXERCISES_ENV_VAR) {
        let p = PathBuf::from(env_path);
        if p.is_dir() {
            return Ok(p);
        }
        return Err(KfError::Config(format!(
            "{EXERCISES_ENV_VAR} path does not exist: {}",
            p.display()
        )));
    }

    // Try relative to the binary location
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for ancestor in exe_dir.ancestors() {
                let candidate = ancestor.join("exercises");
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
    }

    // Try relative to CWD
    let candidates = [PathBuf::from("exercises"), PathBuf::from("../exercises")];
    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }

    Err(KfError::Config(
        "Cannot find exercises directory. Set CLINGS_EXERCISES or run from project root."
            .to_string(),
    ))
}

/// Build a subject → indices map from a slice of exercises.
fn build_subject_index(exercises: &[Exercise]) -> HashMap<String, Vec<usize>> {
    let mut by_subject: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, ex) in exercises.iter().enumerate() {
        by_subject.entry(ex.subject.clone()).or_default().push(i);
    }
    by_subject
}

/// Charge tous les exercices JSON.
/// - Si `CLINGS_EXERCISES` est défini : charge depuis le système de fichiers (comportement authoring).
/// - Sinon : charge depuis les données embarquées dans le binaire.
pub fn load_all_exercises() -> Result<ExerciseSet> {
    if std::env::var(EXERCISES_ENV_VAR).is_ok() {
        let dir = resolve_exercises_dir()?;
        let exercises = load_exercises_from_dir(&dir);
        if exercises.is_empty() {
            return Err(KfError::Config(format!(
                "No exercises found in {}",
                dir.display()
            )));
        }
        let by_subject = build_subject_index(&exercises);
        return Ok((exercises, by_subject));
    }

    let mut exercises = Vec::new();
    for path in EmbeddedExercises::iter() {
        let p = path.as_ref();
        if !p.ends_with(".json") {
            continue;
        }
        let filename = std::path::Path::new(p)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        if filename == "annales_map.json" || filename == "kc_error_map.json" {
            continue;
        }
        let file = EmbeddedExercises::get(p)
            .ok_or_else(|| KfError::Config(format!("embedded file missing: {p}")))?;
        let text = std::str::from_utf8(file.data.as_ref())
            .map_err(|e| KfError::Config(format!("UTF-8 invalide {p}: {e}")))?;
        match serde_json::from_str::<Exercise>(text) {
            Ok(ex) => exercises.push(ex),
            Err(e) => eprintln!("Avertissement : {p} ignoré ({e})"),
        }
    }

    if exercises.is_empty() {
        return Err(KfError::Config(
            "Aucun exercice embarqué trouvé dans le binaire.".to_string(),
        ));
    }
    let by_subject = build_subject_index(&exercises);
    Ok((exercises, by_subject))
}

/// Charge le fichier `annales_map.json`.
/// - Si `CLINGS_EXERCISES` est défini : lit depuis le système de fichiers.
/// - Sinon : charge depuis les données embarquées.
pub fn load_annales_map() -> Result<Vec<crate::models::AnnaleSession>> {
    if std::env::var(EXERCISES_ENV_VAR).is_ok() {
        let dir = resolve_exercises_dir()?;
        let raw = std::fs::read_to_string(dir.join("annales_map.json"))?;
        return serde_json::from_str(&raw)
            .map_err(|e| KfError::Config(format!("annales_map.json: {e}")));
    }
    let file = EmbeddedExercises::get("annales_map.json")
        .ok_or_else(|| KfError::Config("annales_map.json non trouvé dans le binaire".into()))?;
    let text = std::str::from_utf8(file.data.as_ref())
        .map_err(|e| KfError::Config(format!("annales_map.json UTF-8: {e}")))?;
    serde_json::from_str(text).map_err(|e| KfError::Config(format!("annales_map.json: {e}")))
}

/// Recherche un exercice par identifiant exact dans la liste fournie.
pub fn find_exercise<'a>(exercises: &'a [Exercise], id: &str) -> Option<&'a Exercise> {
    exercises.iter().find(|e| e.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_all_exercises_finds_files() -> crate::error::Result<()> {
        let (exercises, by_subject) = load_all_exercises()?;
        assert!(!exercises.is_empty(), "Should load at least one exercise");
        assert!(!by_subject.is_empty(), "Should group by subject");
        // Verify indices are in bounds
        for indices in by_subject.values() {
            for &i in indices {
                assert!(i < exercises.len());
            }
        }
        Ok(())
    }

    #[test]
    fn test_find_exercise_exists() -> crate::error::Result<()> {
        let (exercises, _) = load_all_exercises()?;
        let first_id = &exercises[0].id;
        let found = find_exercise(&exercises, first_id);
        assert!(found.is_some());
        let ex = found.expect("should find exercise");
        assert_eq!(ex.id, *first_id);
        Ok(())
    }

    #[test]
    fn test_find_exercise_missing() -> crate::error::Result<()> {
        let (exercises, _) = load_all_exercises()?;
        assert!(find_exercise(&exercises, "nonexistent-id-999").is_none());
        Ok(())
    }

    #[test]
    fn test_exercises_have_required_fields() -> crate::error::Result<()> {
        let (exercises, _) = load_all_exercises()?;
        for ex in &exercises {
            assert!(!ex.id.is_empty(), "Exercise ID must not be empty");
            assert!(!ex.subject.is_empty(), "Subject must not be empty");
            assert!(!ex.title.is_empty(), "Title must not be empty");
            assert!(
                !ex.starter_code.is_empty(),
                "Starter code must not be empty"
            );
        }
        Ok(())
    }

    #[test]
    fn test_by_subject_consistency() -> crate::error::Result<()> {
        let (exercises, by_subject) = load_all_exercises()?;
        let total_in_map: usize = by_subject.values().map(|v| v.len()).sum();
        assert_eq!(exercises.len(), total_in_map);
        // Verify subjects in map match the exercises they index
        for (subject, indices) in &by_subject {
            for &i in indices {
                assert_eq!(&exercises[i].subject, subject);
            }
        }
        Ok(())
    }

    #[test]
    fn test_exercise_ids_unique() -> crate::error::Result<()> {
        let (exercises, _) = load_all_exercises()?;
        let ids: std::collections::HashSet<&str> =
            exercises.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            exercises.len(),
            ids.len(),
            "Exercise IDs must be unique ({} exercises, {} unique IDs)",
            exercises.len(),
            ids.len()
        );
        Ok(())
    }

    #[test]
    fn test_exercises_fields_complete() -> crate::error::Result<()> {
        let (exercises, _) = load_all_exercises()?;
        for ex in &exercises {
            assert!(!ex.title.is_empty(), "Exercise {} has empty title", ex.id);
            assert!(
                !ex.description.is_empty(),
                "Exercise {} has empty description",
                ex.id
            );
            assert!(
                !ex.starter_code.is_empty(),
                "Exercise {} has empty starter_code",
                ex.id
            );
            assert!(
                !ex.solution_code.is_empty(),
                "Exercise {} has empty solution_code",
                ex.id
            );
            assert!(!ex.hints.is_empty(), "Exercise {} has no hints", ex.id);
        }
        Ok(())
    }

    #[test]
    fn test_starter_code_stages_count() -> crate::error::Result<()> {
        let (exercises, _) = load_all_exercises()?;
        for ex in &exercises {
            if !ex.starter_code_stages.is_empty() {
                assert_eq!(
                    ex.starter_code_stages.len(),
                    5,
                    "Exercise {} has {} stages, expected 5",
                    ex.id,
                    ex.starter_code_stages.len()
                );
                for (i, stage) in ex.starter_code_stages.iter().enumerate() {
                    assert!(
                        !stage.is_empty(),
                        "Exercise {} stage S{} is empty",
                        ex.id,
                        i
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_output_validation_has_expected() -> crate::error::Result<()> {
        use crate::models::ValidationMode;
        let (exercises, _) = load_all_exercises()?;
        for ex in &exercises {
            // Test-mode exercises validate via test_code, not expected_output
            if matches!(ex.validation.mode, ValidationMode::Test) {
                continue;
            }
            assert!(
                ex.validation.expected_output.is_some(),
                "Exercise {} has no expected_output",
                ex.id
            );
            let expected = ex
                .validation
                .expected_output
                .as_ref()
                .expect("expected_output should exist");
            assert!(
                !expected.is_empty(),
                "Exercise {} has empty expected_output",
                ex.id
            );
        }
        Ok(())
    }

    #[test]
    fn test_difficulty_range() -> crate::error::Result<()> {
        let (exercises, _) = load_all_exercises()?;
        for ex in &exercises {
            let d = ex.difficulty as u8;
            assert!(
                (1..=5).contains(&d),
                "Exercise {} has invalid difficulty {}",
                ex.id,
                d
            );
        }
        Ok(())
    }
}
