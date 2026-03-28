//! Unity test framework integration for C exercises.
//!
//! Handles writing Unity framework files, injecting test harnesses, and parsing test results.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::exec::spawn_gcc_and_collect;
use crate::constants::{
    CURRENT_C_FILENAME, TEST_SUMMARY_FAILURES, TEST_SUMMARY_IGNORED, TEST_SUMMARY_TESTS,
};
use crate::models::Exercise;

/// Nom du header Unity copié dans le répertoire de travail.
pub const UNITY_H_FILENAME: &str = "unity.h";
/// Nom du fichier unity_internals.h copié dans le répertoire de travail.
pub const UNITY_INTERNALS_H_FILENAME: &str = "unity_internals.h";
/// Nom du fichier source Unity compilé avec les tests.
pub const UNITY_C_FILENAME: &str = "unity.c";
/// Nom du fichier C généré qui inclut current.c + le code du harnais.
pub const TEST_C_FILENAME: &str = "test_current.c";

/// Patterns C interdits dans `test_code` — prévient l'injection de code via exercices externes.
pub const FORBIDDEN_TEST_CODE_PATTERNS: &[&str] = &[
    "system(",
    "popen(",
    "execv(",
    "execvp(",
    "execve(",
    "execl(",
    "execlp(",
    "execle(",
    "dlopen(",
    "dlsym(",
    "__attribute__((constructor))",
    "#pragma",
    "fork(",
    "kill(",
    "signal(",
    "setuid(",
    "setgid(",
    "chroot(",
    "mount(",
    "ptrace(",
];

/// Find the position of the closing brace `}` matching an opening brace at `open_pos`.
fn find_closing_brace(code: &str, open_pos: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, ch) in code[open_pos..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_pos + i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Supprime la fonction `main()` du code C utilisateur.
fn strip_main_function(code: &str) -> String {
    let Some(main_pos) = code.find("int main") else {
        return code.to_string();
    };
    let after_main = &code[main_pos..];
    let Some(brace_offset) = after_main.find('{') else {
        return code.to_string();
    };
    let brace_start = main_pos + brace_offset;
    let end = find_closing_brace(code, brace_start).unwrap_or(code.len());
    format!("{}{}", &code[..main_pos], &code[end..])
}

/// Valide `test_code` avant d'écrire le fichier C généré.
pub fn validate_test_code(code: &str) -> Option<&'static str> {
    FORBIDDEN_TEST_CODE_PATTERNS
        .iter()
        .copied()
        .find(|&pat| code.contains(pat))
}

/// Write Unity framework files from embedded assets to work directory.
pub fn write_unity_files(work_dir: &Path) -> Result<(), String> {
    let unity_h = include_str!("../../assets/unity/unity.h");
    let unity_int_h = include_str!("../../assets/unity/unity_internals.h");
    let unity_c = include_str!("../../assets/unity/unity.c");
    for (name, content) in [
        (UNITY_H_FILENAME, unity_h),
        (UNITY_INTERNALS_H_FILENAME, unity_int_h),
        (UNITY_C_FILENAME, unity_c),
    ] {
        if let Err(e) = std::fs::write(work_dir.join(name), content) {
            return Err(format!("Impossible d'écrire {name} : {e}"));
        }
    }
    Ok(())
}

/// Compose test_current.c by concatenating user code (without main) + test harness setup + test code.
pub fn compose_test_source(
    source_path: &Path,
    test_code: &str,
    work_dir: &Path,
) -> Result<PathBuf, String> {
    let test_c_path = work_dir.join(TEST_C_FILENAME);
    let source_filename = source_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| CURRENT_C_FILENAME.to_string());
    let user_code = std::fs::read_to_string(source_path)
        .map_err(|e| format!("Impossible de lire {source_filename} : {e}"))?;
    let user_code_no_main = strip_main_function(&user_code);
    let test_c_content = format!(
        "#line 1 \"{source_filename}\"\n{user_code_no_main}\n#include \"unity.h\"\nvoid setUp(void) {{}}\nvoid tearDown(void) {{}}\n\n{test_code}\n"
    );
    std::fs::write(&test_c_path, &test_c_content)
        .map_err(|e| format!("Impossible d'écrire test_current.c : {e}"))?;
    Ok(test_c_path)
}

/// Parse test harness output and determine success based on summary line and expected test count.
pub fn evaluate_test_result(
    stdout: &str,
    stderr: &str,
    status: std::process::ExitStatus,
    expected_pass: Option<usize>,
) -> super::RunResult {
    if !status.success() {
        return super::RunResult {
            success: false,
            stdout: stdout.to_string(),
            stderr: if stderr.is_empty() {
                format!("Process exited with {status}")
            } else {
                stderr.to_string()
            },
            duration_ms: 0,
            compile_error: false,
            timeout: false,
            gcc_hint: None,
        };
    }
    let (success, failures) = parse_test_summary(stdout);
    let passed = success && failures == 0;
    let result_ok = match expected_pass {
        Some(n) => {
            let ok_count = stdout
                .lines()
                .filter(|l| l.trim_start().starts_with("OK"))
                .count();
            passed && ok_count >= n
        }
        None => passed,
    };
    super::RunResult {
        success: result_ok,
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
        duration_ms: 0,
        compile_error: false,
        timeout: false,
        gcc_hint: None,
    }
}

/// Parse the test summary line: "N Tests N Failures 0 Ignored".
#[allow(dead_code)]
pub fn parse_test_summary(stdout: &str) -> (bool, usize) {
    for line in stdout.lines().rev() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 6
            && parts[1] == TEST_SUMMARY_TESTS
            && parts[3] == TEST_SUMMARY_FAILURES
            && parts[5] == TEST_SUMMARY_IGNORED
        {
            let failures = parts[2].parse::<usize>().unwrap_or(1);
            return (true, failures);
        }
    }
    (false, 1)
}

/// Run test-harness mode: write Unity assets + test_current.c, compile, run, parse summary.
pub fn run_tests(source_path: &Path, work_dir: &Path, exercise: &Exercise) -> super::RunResult {
    let test_code = match &exercise.validation.test_code {
        Some(c) => c.as_str(),
        None => {
            return super::make_compile_error(
                "Mode Test : champ 'test_code' manquant dans l'exercice".to_string(),
            );
        }
    };

    if let Some(forbidden) = validate_test_code(test_code) {
        return super::make_compile_error(format!(
            "test_code invalide : pattern interdit détecté (`{forbidden}`)"
        ));
    }

    if let Err(e) = write_unity_files(work_dir) {
        return super::make_compile_error(e);
    }

    let test_c_path = match compose_test_source(source_path, test_code, work_dir) {
        Ok(path) => path,
        Err(e) => return super::make_compile_error(e),
    };

    if let Err(e) = super::write_exercise_files(exercise, work_dir) {
        return super::make_compile_error(format!(
            "Impossible d'écrire les fichiers d'exercice : {e}"
        ));
    }

    let output_path = work_dir.join("kf_test");
    let output_path_str = output_path.to_string_lossy().into_owned();
    let test_c_path_str = test_c_path.to_string_lossy().into_owned();
    let unity_c_path_str = work_dir
        .join(UNITY_C_FILENAME)
        .to_string_lossy()
        .into_owned();
    let include_flag = format!("-I{}", work_dir.display());
    let linker = super::linker_flags(&exercise.subject);

    let mut extra_args: Vec<&str> = vec![
        "-o",
        &output_path_str,
        &test_c_path_str,
        &unity_c_path_str,
        &include_flag,
    ];
    for flag in &linker {
        extra_args.push(flag);
    }

    let timeout = exercise
        .validation
        .max_duration_ms
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_secs(
            crate::constants::EXECUTION_TIMEOUT_SECS,
        ));

    let start = Instant::now();
    let gcc_result = spawn_gcc_and_collect(&test_c_path, &extra_args, work_dir, timeout);
    let expected_pass = exercise.validation.expected_tests_pass;

    super::dispatch_gcc_result(
        gcc_result,
        timeout,
        start,
        move |stdout, stderr, status, duration_ms| {
            let mut result = evaluate_test_result(&stdout, &stderr, status, expected_pass);
            result.duration_ms = duration_ms;
            result
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Difficulty, ExerciseType, Lang, ValidationConfig};

    fn make_exercise(subject: &str, expected: Option<String>) -> Exercise {
        Exercise {
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
    fn test_parse_test_summary_all_pass() {
        let stdout = "  OK    test_foo\n  OK    test_bar\n\n3 Tests 0 Failures 0 Ignored\n";
        let (found, failures) = parse_test_summary(stdout);
        assert!(found);
        assert_eq!(failures, 0);
    }

    #[test]
    fn test_parse_test_summary_with_failures() {
        let stdout = "  OK    test_foo\n  FAIL  test_bar — expected 1 but got 0\n\n2 Tests 1 Failures 0 Ignored\n";
        let (found, failures) = parse_test_summary(stdout);
        assert!(found);
        assert_eq!(failures, 1);
    }

    #[test]
    fn test_parse_test_summary_no_summary_line() {
        let stdout = "some random output\nno summary here\n";
        let (found, failures) = parse_test_summary(stdout);
        assert!(!found);
        assert_eq!(failures, 1);
    }

    #[test]
    fn test_parse_test_summary_empty() {
        let (found, failures) = parse_test_summary("");
        assert!(!found);
        assert_eq!(failures, 1);
    }
}
