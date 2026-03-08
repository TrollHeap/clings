use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::Exercise;

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
/// Priority: KERNELFORGE_EXERCISES env var > auto-detect exercises/ relative to binary or CWD
fn resolve_exercises_dir() -> Result<PathBuf, String> {
    if let Ok(env_path) = std::env::var("KERNELFORGE_EXERCISES") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Ok(p);
        }
        return Err(format!(
            "KERNELFORGE_EXERCISES path does not exist: {}",
            p.display()
        ));
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

    Err(
        "Cannot find exercises directory. Set KERNELFORGE_EXERCISES or run from project root."
            .to_string(),
    )
}

/// Load all C exercises, grouped by subject.
pub fn load_all_exercises() -> Result<ExerciseSet, String> {
    let dir = resolve_exercises_dir()?;
    let exercises = load_exercises_from_dir(&dir);
    if exercises.is_empty() {
        return Err(format!("No exercises found in {}", dir.display()));
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

/// Find an exercise by ID.
pub fn find_exercise<'a>(exercises: &'a [Exercise], id: &str) -> Option<&'a Exercise> {
    exercises.iter().find(|e| e.id == id)
}
