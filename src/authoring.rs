//! Authoring tools: skeleton generator and exercise validator.
//!
//! Used by `clings new`.

use std::path::Path;

use crate::error::{KfError, Result};
use crate::models::{Difficulty, Exercise, Lang, ValidationConfig, ValidationMode};

/// An error found while validating an exercise JSON file.
#[derive(Debug, PartialEq)]
pub struct ValidationError(pub String);

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Generate a skeleton `Exercise` with visible placeholders.
///
/// The skeleton is ready to be serialised to JSON and opened in an editor.
pub fn generate_skeleton(subject: &str, difficulty: u8, mode: &str) -> Result<Exercise> {
    let difficulty = Difficulty::try_from(difficulty)
        .map_err(|e| KfError::Config(format!("difficulty invalide : {e}")))?;

    let validation_mode = parse_mode(mode)?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let id = format!("{subject}-new-{ts}");

    let (expected_output, test_code, expected_tests_pass) = match validation_mode {
        ValidationMode::Output => (Some("__EXPECTED_OUTPUT__".to_owned()), None, None),
        ValidationMode::Test => (
            None,
            Some(
                "#include \"unity.h\"\n\
                 \n\
                 void test_placeholder(void) {\n\
                 \tTEST_ASSERT_EQUAL_INT(1, 1);\n\
                 }\n\
                 \n\
                 int main(void) {\n\
                 \tUNITY_BEGIN();\n\
                 \tRUN_TEST(test_placeholder);\n\
                 \treturn UNITY_END();\n\
                 }"
                .to_string(),
            ),
            Some(1usize),
        ),
    };

    let exercise = Exercise {
        id: id.clone(),
        subject: subject.to_string(),
        lang: Lang::C,
        difficulty,
        title: format!("__TITLE__ ({})", id),
        description: "__DESCRIPTION__\n\nRemplacez ce texte par l'énoncé de l'exercice."
            .to_string(),
        starter_code:
            "#include <stdio.h>\n\nint main(void) {\n    // __STARTER_CODE__\n    return 0;\n}\n"
                .to_string(),
        solution_code:
            "#include <stdio.h>\n\nint main(void) {\n    // __SOLUTION_CODE__\n    return 0;\n}\n"
                .to_string(),
        hints: vec!["__HINT_1__".to_owned(), "__HINT_2__".to_owned()],
        validation: ValidationConfig {
            mode: validation_mode,
            expected_output,
            test_code,
            expected_tests_pass,
            max_duration_ms: None,
        },
        prerequisites: vec![],
        files: vec![],
        exercise_type: Default::default(),
        key_concept: Some("__KEY_CONCEPT__".to_string()),
        common_mistake: Some("__COMMON_MISTAKE__".to_string()),
        kc_ids: vec![],
        starter_code_stages: vec![],
        visualizer: Default::default(),
    };

    Ok(exercise)
}

/// Parse a mode string to `ValidationMode`.
fn parse_mode(mode: &str) -> Result<ValidationMode> {
    match mode.to_lowercase().as_str() {
        "output" => Ok(ValidationMode::Output),
        "test" => Ok(ValidationMode::Test),
        other => Err(KfError::Config(format!(
            "mode invalide : '{other}' (attendu : output, test)"
        ))),
    }
}

/// Validate an exercise JSON file and return a list of errors.
///
/// An empty list means the file is structurally valid.
/// This does **not** compile the C code (no `gcc` invocation).
pub fn validate_exercise(path: &Path) -> Vec<ValidationError> {
    let mut errors: Vec<ValidationError> = Vec::new();

    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            errors.push(ValidationError(format!(
                "Impossible de lire le fichier : {e}"
            )));
            return errors;
        }
    };

    let exercise: Exercise = match serde_json::from_str(&raw) {
        Ok(e) => e,
        Err(e) => {
            errors.push(ValidationError(format!("JSON invalide : {e}")));
            return errors;
        }
    };

    // Required non-empty fields
    if exercise.id.trim().is_empty() {
        errors.push(ValidationError("`id` est vide".to_string()));
    }
    if exercise.title.trim().is_empty() || exercise.title.contains("__TITLE__") {
        errors.push(ValidationError(
            "`title` contient un placeholder ou est vide".to_string(),
        ));
    }
    if exercise.description.trim().is_empty() || exercise.description.contains("__DESCRIPTION__") {
        errors.push(ValidationError(
            "`description` contient un placeholder ou est vide".to_string(),
        ));
    }
    if exercise.starter_code.trim().is_empty() || exercise.starter_code.contains("__STARTER_CODE__")
    {
        errors.push(ValidationError(
            "`starter_code` contient un placeholder ou est vide".to_string(),
        ));
    }
    if exercise.solution_code.trim().is_empty()
        || exercise.solution_code.contains("__SOLUTION_CODE__")
    {
        errors.push(ValidationError(
            "`solution_code` contient un placeholder ou est vide".to_string(),
        ));
    }
    if exercise.hints.is_empty() {
        errors.push(ValidationError(
            "`hints` est vide (au moins 1 indice requis)".to_string(),
        ));
    }

    // Validation mode checks
    match exercise.validation.mode {
        ValidationMode::Output => match &exercise.validation.expected_output {
            None => errors.push(ValidationError(
                "`validation.expected_output` manquant pour mode output".to_string(),
            )),
            Some(s) if s.contains("__EXPECTED_OUTPUT__") => errors.push(ValidationError(
                "`validation.expected_output` contient un placeholder".to_string(),
            )),
            _ => {}
        },
        ValidationMode::Test => {}
    }
    match exercise.validation.mode {
        ValidationMode::Test => match &exercise.validation.test_code {
            None => errors.push(ValidationError(
                "`validation.test_code` manquant pour mode test".to_string(),
            )),
            Some(s) if s.trim().is_empty() => errors.push(ValidationError(
                "`validation.test_code` est vide".to_string(),
            )),
            _ => {}
        },
        ValidationMode::Output => {}
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(name: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        let mut f = std::fs::File::create(&path).expect("failed to create temp file for test");
        f.write_all(content.as_bytes())
            .expect("failed to write temp file content for test");
        path
    }

    #[test]
    fn test_generate_skeleton_output_mode() {
        let ex = generate_skeleton("pointers", 2, "output").unwrap();
        assert_eq!(ex.subject, "pointers");
        assert!(ex.id.starts_with("pointers-new-"));
        assert!(ex.validation.expected_output.is_some());
        assert!(ex.validation.test_code.is_none());
        assert_eq!(ex.validation.mode, ValidationMode::Output);
    }

    #[test]
    fn test_generate_skeleton_test_mode() {
        let ex = generate_skeleton("semaphores", 1, "test").unwrap();
        assert!(ex.validation.test_code.is_some());
        assert!(ex.validation.expected_output.is_none());
        assert!(ex.validation.expected_tests_pass.is_some());
        assert_eq!(ex.validation.mode, ValidationMode::Test);
    }

    #[test]
    fn test_generate_skeleton_invalid_difficulty() {
        assert!(generate_skeleton("pointers", 6, "output").is_err());
        assert!(generate_skeleton("pointers", 0, "output").is_err());
    }

    #[test]
    fn test_validate_exercise_missing_fields() {
        let json =
            "{\"id\":\"ptr-test-99\",\"subject\":\"pointers\",\"lang\":\"c\",\"difficulty\":1,\
                    \"title\":\"\",\"description\":\"desc\",\
                    \"starter_code\":\"#include <stdio.h>\\nint main(){return 0;}\",\
                    \"solution_code\":\"#include <stdio.h>\\nint main(){return 0;}\",\
                    \"hints\":[\"hint1\"],\
                    \"validation\":{\"mode\":\"output\",\"expected_output\":\"ok\"}}";
        let path = write_tmp("clings_test_missing_title.json", json);
        let errors = validate_exercise(&path);
        assert!(
            errors.iter().any(|e| e.0.contains("title")),
            "expected title error, got: {errors:?}"
        );
    }

    #[test]
    fn test_validate_exercise_valid_json() {
        let json = "{\"id\":\"ptr-test-99\",\"subject\":\"pointers\",\"lang\":\"c\",\"difficulty\":1,\
                    \"title\":\"Test exercise\",\"description\":\"A real description\",\
                    \"starter_code\":\"#include <stdio.h>\\nint main(){return 0;}\",\
                    \"solution_code\":\"#include <stdio.h>\\nint main(){printf(\\\"ok\\\");return 0;}\",\
                    \"hints\":[\"check the pointer\"],\
                    \"validation\":{\"mode\":\"output\",\"expected_output\":\"ok\"}}";
        let path = write_tmp("clings_test_valid.json", json);
        let errors = validate_exercise(&path);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }
}
