use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{KfError, Result};
use crate::models::Exercise;

/// Ensemble d'exercices : liste complète + index par sujet.
pub type ExerciseSet = (Vec<Exercise>, HashMap<String, Vec<Exercise>>);

/// Recursively load all exercise JSON files from a directory.
fn load_exercises_from_dir(dir: &Path) -> Vec<Exercise> {
    let mut exercises = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return exercises,
    };
    for entry in entries.flatten() {
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
                    Err(e) => eprintln!("Warning: failed to parse {}: {}", path.display(), e),
                }
            }
        }
    }
    exercises
}

/// Resolve the exercises directory path.
/// Priority: CLINGS_EXERCISES env var > auto-detect exercises/ relative to binary or CWD
pub fn resolve_exercises_dir() -> Result<PathBuf> {
    if let Ok(env_path) = std::env::var("CLINGS_EXERCISES") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Ok(p);
        }
        return Err(KfError::Config(format!(
            "CLINGS_EXERCISES path does not exist: {}",
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

/// Charge tous les exercices JSON depuis le répertoire résolu, groupés par sujet.
/// Retourne une erreur si aucun exercice n'est trouvé.
pub fn load_all_exercises() -> Result<ExerciseSet> {
    let dir = resolve_exercises_dir()?;
    let exercises = load_exercises_from_dir(&dir);
    if exercises.is_empty() {
        return Err(KfError::Config(format!(
            "No exercises found in {}",
            dir.display()
        )));
    }

    let mut by_subject: HashMap<String, Vec<Exercise>> = HashMap::new();
    for ex in &exercises {
        by_subject
            .entry(ex.subject.clone())
            .or_default()
            .push(ex.clone());
    }

    Ok((exercises, by_subject))
}

/// Recherche un exercice par identifiant exact dans la liste fournie.
pub fn find_exercise<'a>(exercises: &'a [Exercise], id: &str) -> Option<&'a Exercise> {
    exercises.iter().find(|e| e.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_all_exercises_finds_files() {
        let result = load_all_exercises();
        assert!(result.is_ok(), "Should find exercises from project root");
        let (exercises, by_subject) = result.unwrap();
        assert!(!exercises.is_empty(), "Should load at least one exercise");
        assert!(!by_subject.is_empty(), "Should group by subject");
    }

    #[test]
    fn test_find_exercise_exists() {
        let (exercises, _) = load_all_exercises().unwrap();
        let first_id = &exercises[0].id;
        let found = find_exercise(&exercises, first_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, *first_id);
    }

    #[test]
    fn test_find_exercise_missing() {
        let (exercises, _) = load_all_exercises().unwrap();
        assert!(find_exercise(&exercises, "nonexistent-id-999").is_none());
    }

    #[test]
    fn test_exercises_have_required_fields() {
        let (exercises, _) = load_all_exercises().unwrap();
        for ex in &exercises {
            assert!(!ex.id.is_empty(), "Exercise ID must not be empty");
            assert!(!ex.subject.is_empty(), "Subject must not be empty");
            assert!(!ex.title.is_empty(), "Title must not be empty");
            assert!(
                !ex.starter_code.is_empty(),
                "Starter code must not be empty"
            );
        }
    }

    #[test]
    fn test_by_subject_consistency() {
        let (exercises, by_subject) = load_all_exercises().unwrap();
        let total_in_map: usize = by_subject.values().map(|v| v.len()).sum();
        assert_eq!(exercises.len(), total_in_map);
    }
}
