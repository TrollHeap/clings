//! C code compilation and execution engine.
//!
//! Compiles user code with `gcc -Wall -Wextra -std=c11`, writes it to `~/.clings/current.c`,
//! runs it with a 10-second timeout, and validates stdout against expected output.
//! Supports `Output` and `Test` (Unity) validation modes via `compile_and_run()`.

pub mod compiler;
pub mod exec;
pub mod files;
pub mod unity;

// Public API — used by modules outside runner/
pub use compiler::{compile_and_run, RunResult};
pub use files::{prepare_exercise_source, write_starter_code};

// Test helpers — only compiled in test mode
#[cfg(test)]
pub use compiler::{linker_flags, parse_gcc_hint};
#[cfg(test)]
pub use files::{mastery_to_stage, select_starter_code, work_dir, write_exercise_files};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Difficulty, ExerciseType, Lang, ValidationConfig, ValidationMode};

    fn make_exercise(subject: &str, expected: Option<String>) -> crate::models::Exercise {
        crate::models::Exercise {
            id: "test".to_string(),
            subject: subject.to_string(),
            lang: Lang::C,
            difficulty: Difficulty::Easy,
            title: "Test".to_string(),
            description: "Test".to_string(),
            starter_code: "int main() { return 0; }".to_string(),
            solution_code: "int main() { return 0; }".to_string(),
            hints: vec![],
            validation: ValidationConfig {
                expected_output: expected,
                ..Default::default()
            },
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
    fn test_mastery_to_stage() {
        assert_eq!(mastery_to_stage(0.5), 0);
        assert_eq!(mastery_to_stage(1.5), 1);
        assert_eq!(mastery_to_stage(2.5), 2);
        assert_eq!(mastery_to_stage(3.5), 3);
        assert_eq!(mastery_to_stage(4.5), 4);
    }

    #[test]
    fn test_select_starter_code_no_stages() {
        let exercise = make_exercise("test", None);
        assert_eq!(select_starter_code(&exercise, 2.5), &exercise.starter_code);
    }

    #[test]
    fn test_select_starter_code_with_stages() {
        let mut exercise = make_exercise("test", None);
        exercise.starter_code_stages = vec![
            "stage0".to_string(),
            "stage1".to_string(),
            "stage2".to_string(),
        ];
        // mastery 1.5 → stage 1, which exists
        assert_eq!(select_starter_code(&exercise, 1.5), "stage1");
        // mastery 4.5 → stage 4, but only 3 stages available → fall back to default
        assert_eq!(select_starter_code(&exercise, 4.5), &exercise.starter_code);
    }

    #[test]
    fn test_write_exercise_files_rejects_path_traversal() {
        use crate::models::ExerciseFile;
        let dir = std::env::temp_dir().join("clings_test_traversal");
        let _ = std::fs::create_dir_all(&dir);

        let mut exercise = make_exercise("test", None);
        exercise.files = vec![ExerciseFile {
            name: "../escape.txt".to_string(),
            content: "pwned".to_string(),
            readonly: false,
        }];

        let result = write_exercise_files(&exercise, &dir);
        assert!(result.is_ok());
        assert!(
            !dir.parent().unwrap().join("escape.txt").exists(),
            "path traversal should not create files outside work_dir"
        );

        if let Err(e) = std::fs::remove_dir_all(&dir) {
            eprintln!("test cleanup warning: {e}");
        }
    }

    #[test]
    fn test_timeout_kills_process() {
        use std::os::unix::process::CommandExt;
        let mut child = std::process::Command::new("sleep")
            .arg("100")
            .process_group(0)
            .spawn()
            .expect("sleep must be available on Linux");

        let timeout = std::time::Duration::from_millis(100);
        let result = exec::wait_for_process_with_timeout(&mut child, timeout);

        assert!(result.is_err(), "should return Err on timeout");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(exec::TIMEOUT_MSG_PREFIX),
            "expected TIMEOUT_MSG_PREFIX in error, got: {msg}"
        );
    }

    #[test]
    fn test_compile_and_run_syntax_error_returns_compile_error() {
        let dir = std::env::temp_dir().join("clings_test_compile_err");
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("syntax_error.c");
        std::fs::write(&src, "int main(void) { invalid syntax HERE !!! }").unwrap();

        let mut exercise = make_exercise("test_compile_err", Some("ok".to_string()));
        exercise.validation.mode = ValidationMode::Output;

        let result = compile_and_run(&src, &exercise);

        let _ = std::fs::remove_dir_all(&dir);

        assert!(!result.success, "syntaxe invalide doit échouer");
        assert!(result.compile_error, "doit signaler compile_error=true");
        assert!(
            !result.stderr.is_empty(),
            "stderr doit contenir le message d'erreur gcc"
        );
    }

    #[test]
    fn test_compile_and_run_valid_c_succeeds() {
        let dir = std::env::temp_dir().join("clings_test_compile_ok");
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("hello.c");
        std::fs::write(
            &src,
            "#include <stdio.h>\nint main(void){printf(\"ok\\n\");return 0;}\n",
        )
        .unwrap();

        let mut exercise = make_exercise("test_compile_ok", Some("ok".to_string()));
        exercise.validation.mode = ValidationMode::Output;
        exercise.validation.expected_output = Some("ok".to_string());

        let result = compile_and_run(&src, &exercise);

        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            result.success,
            "C valide avec output correct doit réussir; stderr: {}",
            result.stderr
        );
        assert!(
            !result.compile_error,
            "pas d'erreur de compilation attendue"
        );
    }

    #[test]
    fn test_compile_and_run_output_mismatch_fails() {
        let dir = std::env::temp_dir().join("clings_test_mismatch");
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("mismatch.c");
        std::fs::write(
            &src,
            "#include <stdio.h>\nint main(void){printf(\"wrong\\n\");return 0;}\n",
        )
        .unwrap();

        let mut exercise = make_exercise("test_mismatch", Some("expected_output".to_string()));
        exercise.validation.mode = ValidationMode::Output;
        exercise.validation.expected_output = Some("expected_output".to_string());

        let result = compile_and_run(&src, &exercise);

        let _ = std::fs::remove_dir_all(&dir);

        assert!(!result.success, "output incorrect doit échouer");
        assert!(
            !result.compile_error,
            "ce n'est pas une erreur de compilation"
        );
    }

    #[test]
    fn test_linker_flags_pthreads_modules() {
        let pthread_subjects = vec![
            "pthreads",
            "semaphores",
            "sync_concepts",
            "sockets",
            "capstones",
        ];
        for subject in pthread_subjects {
            let flags = linker_flags(subject);
            assert!(
                flags.contains(&"-lpthread"),
                "{subject} should include -lpthread, got: {flags:?}"
            );
        }
    }

    #[test]
    fn test_linker_flags_ipc_subjects() {
        let ipc_subjects = vec!["message_queues", "shared_memory"];
        for subject in ipc_subjects {
            let flags = linker_flags(subject);
            assert!(
                flags.contains(&"-lrt"),
                "{subject} should include -lrt, got: {flags:?}"
            );
            assert!(
                flags.contains(&"-lpthread"),
                "{subject} should include -lpthread, got: {flags:?}"
            );
        }
    }

    #[test]
    fn test_linker_flags_file_io() {
        let flags = linker_flags("file_io");
        assert_eq!(flags, vec!["-lrt"], "file_io should only have -lrt");
    }

    #[test]
    fn test_linker_flags_no_special_flags() {
        let no_flag_subjects = vec![
            "pointers",
            "structs",
            "processes",
            "bitwise_ops",
            "unknown_subject",
        ];
        for subject in no_flag_subjects {
            let flags = linker_flags(subject);
            assert!(
                flags.is_empty(),
                "{subject} should have no special flags, got: {flags:?}"
            );
        }
    }

    #[test]
    fn test_write_exercise_files_rejects_absolute_paths() {
        use crate::models::ExerciseFile;
        let dir = std::env::temp_dir().join("clings_test_abs_path");
        let _ = std::fs::create_dir_all(&dir);

        let mut exercise = make_exercise("test", None);
        exercise.files = vec![ExerciseFile {
            name: "/etc/passwd".to_string(),
            content: "should_be_rejected".to_string(),
            readonly: false,
        }];

        let result = write_exercise_files(&exercise, &dir);
        assert!(
            result.is_ok(),
            "write_exercise_files should handle absolute paths gracefully"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_exercise_files_normal_files() {
        use crate::models::ExerciseFile;
        let dir = std::env::temp_dir().join("clings_test_normal_files");
        let _ = std::fs::create_dir_all(&dir);

        let mut exercise = make_exercise("test", None);
        exercise.files = vec![
            ExerciseFile {
                name: "header.h".to_string(),
                content: "#ifndef HEADER_H\n#define HEADER_H\nvoid func(void);\n#endif".to_string(),
                readonly: false,
            },
            ExerciseFile {
                name: "subdir/data.txt".to_string(),
                content: "test data".to_string(),
                readonly: false,
            },
        ];

        let result = write_exercise_files(&exercise, &dir);
        assert!(result.is_ok(), "writing normal files should succeed");

        assert!(dir.join("header.h").exists(), "header.h should exist");
        assert!(
            dir.join("subdir/data.txt").exists(),
            "subdir/data.txt should exist"
        );

        let header_content = std::fs::read_to_string(dir.join("header.h")).unwrap();
        assert!(
            header_content.contains("#ifndef HEADER_H"),
            "header.h content should match"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_work_dir_creates_directory() {
        let result = work_dir();
        assert!(result.is_ok(), "work_dir() should return Ok");
        let path = result.unwrap();
        assert!(path.exists(), "work_dir() should create the directory");
        assert!(path.is_dir(), "work_dir() should return a directory path");
    }

    #[test]
    fn test_parse_gcc_hint_missing_semicolon() {
        let stderr = "error: expected ';' before 'return' at line 5";
        let hint = parse_gcc_hint(stderr);
        assert!(hint.is_some(), "should detect missing semicolon pattern");
        assert!(
            hint.unwrap().contains("Point-virgule"),
            "hint should mention semicolon in French"
        );
    }

    #[test]
    fn test_parse_gcc_hint_implicit_function() {
        let stderr = "warning: implicit declaration of function 'strlen'";
        let hint = parse_gcc_hint(stderr);
        assert!(hint.is_some(), "should detect implicit declaration pattern");
        assert!(
            hint.unwrap().contains("déclaration"),
            "hint should mention declaration"
        );
    }

    #[test]
    fn test_parse_gcc_hint_undefined_reference() {
        let stderr = "undefined reference to `pthread_create'";
        let hint = parse_gcc_hint(stderr);
        assert!(hint.is_some(), "should detect undefined reference pattern");
        assert!(
            hint.unwrap().contains("liaison"),
            "hint should mention linker flags"
        );
    }

    #[test]
    fn test_parse_gcc_hint_no_match() {
        let stderr = "some random error message with no pattern match";
        let hint = parse_gcc_hint(stderr);
        assert!(hint.is_none(), "should return None for unknown errors");
    }
}
