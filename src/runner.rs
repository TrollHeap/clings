use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::models::{Exercise, ValidationMode};

// Per-thread cache of compiled regexes: pattern string → compiled Regex (or None if invalid).
thread_local! {
    static REGEX_CACHE: RefCell<HashMap<String, Option<regex::Regex>>> =
        RefCell::new(HashMap::new());
}

/// Résultat de la compilation et de l'exécution d'un exercice C.
pub struct RunResult {
    /// `true` si la compilation a réussi, l'exécution s'est terminée et la sortie est valide
    pub success: bool,
    /// Sortie standard du programme compilé
    pub stdout: String,
    /// Sortie d'erreur (messages gcc ou stderr du programme)
    pub stderr: String,
    /// Durée d'exécution en millisecondes
    pub duration_ms: u64,
    /// `true` si gcc a échoué (le programme n'a pas été exécuté)
    pub compile_error: bool,
    /// `true` si le programme n'a pas terminé dans la limite de 10 secondes
    pub timeout: bool,
}

/// Determine linker flags based on subject.
fn linker_flags(subject: &str) -> Vec<&'static str> {
    match subject {
        "pthreads" | "semaphores" | "sync_concepts" | "sockets" | "capstones" => {
            vec!["-lpthread"]
        }
        "message_queues" | "shared_memory" => vec!["-lrt", "-lpthread"],
        "file_io" => vec!["-lrt"],
        _ => vec![],
    }
}

/// Write exercise files (headers etc.) to a temp directory.
fn write_exercise_files(exercise: &Exercise, work_dir: &Path) -> std::io::Result<()> {
    for file in &exercise.files {
        let file_path = work_dir.join(&file.name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, &file.content)?;
    }
    Ok(())
}

/// Compile and run a C source file, validating output against expected.
pub fn compile_and_run(source_path: &Path, exercise: &Exercise) -> RunResult {
    let work_dir = source_path.parent().unwrap_or(Path::new("/tmp"));
    let output_path = work_dir.join("kf_run");

    // Write additional files (headers etc.)
    if let Err(e) = write_exercise_files(exercise, work_dir) {
        return RunResult {
            success: false,
            stdout: String::new(),
            stderr: format!("Failed to write exercise files: {e}"),
            duration_ms: 0,
            compile_error: true,
            timeout: false,
        };
    }

    // Compile
    let mut gcc = Command::new("gcc");
    gcc.args(["-Wall", "-Wextra", "-std=c11", "-D_GNU_SOURCE"])
        .arg("-o")
        .arg(&output_path)
        .arg(source_path);

    // Add include path for headers
    gcc.arg(format!("-I{}", work_dir.display()));

    // Add linker flags
    for flag in linker_flags(&exercise.subject) {
        gcc.arg(flag);
    }

    let compile_result = match gcc.output() {
        Ok(r) => r,
        Err(e) => {
            return RunResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to run gcc: {e}. Is gcc installed?"),
                duration_ms: 0,
                compile_error: true,
                timeout: false,
            };
        }
    };

    if !compile_result.status.success() {
        return RunResult {
            success: false,
            stdout: String::new(),
            stderr: String::from_utf8_lossy(&compile_result.stderr).to_string(),
            duration_ms: 0,
            compile_error: true,
            timeout: false,
        };
    }

    // Execute with timeout
    let start = Instant::now();
    let child = Command::new(&output_path)
        .current_dir(work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            return RunResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to execute: {e}"),
                duration_ms: 0,
                compile_error: false,
                timeout: false,
            };
        }
    };

    // Drain stdout/stderr in background threads to prevent pipe buffer deadlock.
    // If the child writes more than ~64 KB without being read, write() blocks
    // and wait() never returns (classic pipe deadlock). Reading concurrently avoids it.
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();
    let stdout_thread = std::thread::spawn(move || -> String {
        stdout_handle
            .map(|mut s| {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut s, &mut buf).ok();
                buf
            })
            .unwrap_or_default()
    });
    let stderr_thread = std::thread::spawn(move || -> String {
        stderr_handle
            .map(|mut s| {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut s, &mut buf).ok();
                buf
            })
            .unwrap_or_default()
    });

    let timeout = exercise
        .validation
        .max_duration_ms
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_secs(10));

    match child.wait_timeout(timeout) {
        Ok(Some(status)) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let stdout = stdout_thread.join().unwrap_or_default();
            let stderr = stderr_thread.join().unwrap_or_default();

            if !status.success() {
                return RunResult {
                    success: false,
                    stdout,
                    stderr: if stderr.is_empty() {
                        format!("Process exited with {status}")
                    } else {
                        stderr
                    },
                    duration_ms,
                    compile_error: false,
                    timeout: false,
                };
            }

            // Validate output
            let valid = validate_output(&stdout, exercise);
            RunResult {
                success: valid,
                stdout,
                stderr,
                duration_ms,
                compile_error: false,
                timeout: false,
            }
        }
        Ok(None) => {
            // Timeout — kill the process, then join readers (pipe close unblocks them)
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            RunResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Execution timed out ({:.1}s limit)", timeout.as_secs_f64()),
                duration_ms: timeout.as_millis() as u64,
                compile_error: false,
                timeout: true,
            }
        }
        Err(e) => {
            let _ = child.kill();
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            RunResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Wait error: {e}"),
                duration_ms: start.elapsed().as_millis() as u64,
                compile_error: false,
                timeout: false,
            }
        }
    }
}

/// Validate program output against expected output.
/// If expected_output starts with "REGEX:" the remainder is compiled as a regex
/// and matched against the full (normalized) stdout.
fn validate_output(stdout: &str, exercise: &Exercise) -> bool {
    match exercise.validation.mode {
        ValidationMode::Output => {
            if let Some(expected) = &exercise.validation.expected_output {
                let norm_out = normalize(stdout);
                let norm_exp = normalize(expected);
                if let Some(pattern) = norm_exp.strip_prefix("REGEX:") {
                    let key = pattern.trim().to_string();
                    REGEX_CACHE.with(|cache| {
                        let mut map = cache.borrow_mut();
                        let re = map
                            .entry(key.clone())
                            .or_insert_with(|| regex::Regex::new(&key).ok());
                        re.as_ref().map(|r| r.is_match(&norm_out)).unwrap_or(false)
                    })
                } else {
                    norm_out == norm_exp
                }
            } else {
                // No expected output defined — just check it compiled and ran
                true
            }
        }
        ValidationMode::Test | ValidationMode::Both => {
            // Test mode not supported in CLI MVP — warn
            false
        }
    }
}

/// Normalize output: trim, normalize newlines, remove trailing whitespace per line.
fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Get the working directory for exercises.
pub fn work_dir() -> PathBuf {
    let dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".clings");
    #[cfg(unix)]
    {
        use std::fs::DirBuilder;
        use std::os::unix::fs::DirBuilderExt;
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .ok();
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(&dir).ok();
    }
    dir
}

/// Map mastery score to stage index (0-4).
pub fn mastery_to_stage(mastery: f64) -> u8 {
    match mastery {
        m if m < 1.0 => 0,
        m if m < 2.0 => 1,
        m if m < 3.0 => 2,
        m if m < 4.0 => 3,
        _ => 4,
    }
}

/// Select the appropriate starter code stage based on mastery score.
/// Higher mastery → harder stage (less scaffolding).
pub fn select_starter_code(exercise: &Exercise, mastery: f64) -> &str {
    let stage = mastery_to_stage(mastery) as usize;
    exercise
        .starter_code_stages
        .get(stage)
        .map(|s| s.as_str())
        .unwrap_or(&exercise.starter_code)
}

/// Write starter code to the current.c file.
/// If mastery is provided, selects the appropriate stage.
pub fn write_starter_code(exercise: &Exercise, mastery: Option<f64>) -> std::io::Result<PathBuf> {
    let dir = work_dir();
    let source_path = dir.join("current.c");
    let code = match mastery {
        Some(m) => select_starter_code(exercise, m),
        None => &exercise.starter_code,
    };
    let mut f = std::fs::File::create(&source_path)?;
    f.write_all(code.as_bytes())?;

    // Write additional files
    write_exercise_files(exercise, &dir)?;

    Ok(source_path)
}

/// Trait to add wait_timeout to Child (not in std).
trait ChildExt {
    fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<std::process::ExitStatus>>;
}

impl ChildExt for std::process::Child {
    fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<std::process::ExitStatus>> {
        let start = Instant::now();
        loop {
            match self.try_wait()? {
                Some(status) => return Ok(Some(status)),
                None => {
                    if start.elapsed() >= timeout {
                        return Ok(None);
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Difficulty, ExerciseType, Lang, ValidationConfig, ValidationMode};

    fn make_exercise(
        subject: &str,
        validation_mode: ValidationMode,
        expected: Option<String>,
    ) -> Exercise {
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
                mode: validation_mode,
                expected_output: expected,
                test_code: None,
                max_duration_ms: None,
            },
            prerequisites: vec![],
            files: vec![],
            exercise_type: ExerciseType::Complete,
            key_concept: None,
            common_mistake: None,
            kc_ids: vec![],
            starter_code_stages: vec![],
            visualizer: Default::default(),
        }
    }

    #[test]
    fn test_normalize_trims_whitespace() {
        let input = "  hello  \n  world  \n  ";
        let result = normalize(input);
        // trim() removes leading/trailing whitespace on the whole string,
        // but preserves internal line indentation
        assert_eq!(result, "hello\n  world");
    }

    #[test]
    fn test_normalize_crlf() {
        let input = "hello\r\nworld\r\n";
        let result = normalize(input);
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn test_normalize_empty() {
        let result = normalize("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_normalize_trailing_whitespace_per_line() {
        let input = "hello   \nworld   \n";
        let result = normalize(input);
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn test_validate_output_exact_match() {
        let exercise = make_exercise("pointers", ValidationMode::Output, Some("42".to_string()));
        assert!(validate_output("42", &exercise));
    }

    #[test]
    fn test_validate_output_mismatch() {
        let exercise = make_exercise("pointers", ValidationMode::Output, Some("42".to_string()));
        assert!(!validate_output("43", &exercise));
    }

    #[test]
    fn test_validate_output_whitespace_normalization() {
        let exercise = make_exercise(
            "pointers",
            ValidationMode::Output,
            Some("hello\n  world".to_string()),
        );
        assert!(validate_output("hello  \n  world  ", &exercise));
    }

    #[test]
    fn test_validate_output_regex() {
        let exercise = make_exercise(
            "pointers",
            ValidationMode::Output,
            Some("REGEX:^[0-9]+$".to_string()),
        );
        assert!(validate_output("42", &exercise));
        assert!(!validate_output("abc", &exercise));
    }

    #[test]
    fn test_validate_output_regex_with_whitespace() {
        let exercise = make_exercise(
            "pointers",
            ValidationMode::Output,
            Some("REGEX:^hello\\s+world$".to_string()),
        );
        assert!(validate_output("hello   world", &exercise));
        assert!(!validate_output("hello world extra", &exercise));
    }

    #[test]
    fn test_validate_output_regex_invalid() {
        let exercise = make_exercise(
            "pointers",
            ValidationMode::Output,
            Some("REGEX:[invalid(".to_string()),
        );
        assert!(!validate_output("anything", &exercise));
    }

    #[test]
    fn test_validate_output_no_expected() {
        let exercise = make_exercise("pointers", ValidationMode::Output, None);
        assert!(validate_output("anything", &exercise));
    }

    #[test]
    fn test_validate_output_test_mode() {
        let exercise = make_exercise("pointers", ValidationMode::Test, Some("42".to_string()));
        assert!(!validate_output("42", &exercise));
    }

    #[test]
    fn test_validate_output_both_mode() {
        let exercise = make_exercise("pointers", ValidationMode::Both, Some("42".to_string()));
        assert!(!validate_output("42", &exercise));
    }

    #[test]
    fn test_linker_flags_pthreads() {
        let flags = linker_flags("pthreads");
        assert_eq!(flags, vec!["-lpthread"]);
    }

    #[test]
    fn test_linker_flags_semaphores() {
        let flags = linker_flags("semaphores");
        assert_eq!(flags, vec!["-lpthread"]);
    }

    #[test]
    fn test_linker_flags_sync_concepts() {
        let flags = linker_flags("sync_concepts");
        assert_eq!(flags, vec!["-lpthread"]);
    }

    #[test]
    fn test_linker_flags_sockets() {
        let flags = linker_flags("sockets");
        assert_eq!(flags, vec!["-lpthread"]);
    }

    #[test]
    fn test_linker_flags_capstones() {
        let flags = linker_flags("capstones");
        assert_eq!(flags, vec!["-lpthread"]);
    }

    #[test]
    fn test_linker_flags_message_queues() {
        let flags = linker_flags("message_queues");
        assert_eq!(flags, vec!["-lrt", "-lpthread"]);
    }

    #[test]
    fn test_linker_flags_shared_memory() {
        let flags = linker_flags("shared_memory");
        assert_eq!(flags, vec!["-lrt", "-lpthread"]);
    }

    #[test]
    fn test_linker_flags_file_io() {
        let flags = linker_flags("file_io");
        assert_eq!(flags, vec!["-lrt"]);
    }

    #[test]
    fn test_linker_flags_unknown() {
        let flags = linker_flags("unknown_subject");
        assert_eq!(flags, Vec::<&'static str>::new());
    }

    #[test]
    fn test_linker_flags_pointers() {
        let flags = linker_flags("pointers");
        let expected: Vec<&'static str> = Vec::new();
        assert_eq!(flags, expected);
    }

    #[test]
    fn test_mastery_to_stage_zero() {
        assert_eq!(mastery_to_stage(0.0), 0);
    }

    #[test]
    fn test_mastery_to_stage_half() {
        assert_eq!(mastery_to_stage(0.5), 0);
    }

    #[test]
    fn test_mastery_to_stage_one() {
        assert_eq!(mastery_to_stage(1.5), 1);
    }

    #[test]
    fn test_mastery_to_stage_two() {
        assert_eq!(mastery_to_stage(2.5), 2);
    }

    #[test]
    fn test_mastery_to_stage_three() {
        assert_eq!(mastery_to_stage(3.5), 3);
    }

    #[test]
    fn test_mastery_to_stage_four() {
        assert_eq!(mastery_to_stage(4.5), 4);
    }

    #[test]
    fn test_mastery_to_stage_max() {
        assert_eq!(mastery_to_stage(5.0), 4);
    }

    #[test]
    fn test_mastery_to_stage_boundary_exactly_one() {
        assert_eq!(mastery_to_stage(1.0), 1);
    }

    #[test]
    fn test_mastery_to_stage_boundary_exactly_two() {
        assert_eq!(mastery_to_stage(2.0), 2);
    }

    #[test]
    fn test_mastery_to_stage_boundary_exactly_three() {
        assert_eq!(mastery_to_stage(3.0), 3);
    }

    #[test]
    fn test_mastery_to_stage_boundary_exactly_four() {
        assert_eq!(mastery_to_stage(4.0), 4);
    }

    #[test]
    fn test_select_starter_code_stage_zero() {
        let mut exercise = make_exercise("pointers", ValidationMode::Output, None);
        exercise.starter_code = "stage0".to_string();
        exercise.starter_code_stages = vec![
            "stage0".to_string(),
            "stage1".to_string(),
            "stage2".to_string(),
            "stage3".to_string(),
            "stage4".to_string(),
        ];
        assert_eq!(select_starter_code(&exercise, 0.5), "stage0");
    }

    #[test]
    fn test_select_starter_code_stage_one() {
        let mut exercise = make_exercise("pointers", ValidationMode::Output, None);
        exercise.starter_code = "stage0".to_string();
        exercise.starter_code_stages = vec![
            "stage0".to_string(),
            "stage1".to_string(),
            "stage2".to_string(),
            "stage3".to_string(),
            "stage4".to_string(),
        ];
        assert_eq!(select_starter_code(&exercise, 1.5), "stage1");
    }

    #[test]
    fn test_select_starter_code_stage_two() {
        let mut exercise = make_exercise("pointers", ValidationMode::Output, None);
        exercise.starter_code = "stage0".to_string();
        exercise.starter_code_stages = vec![
            "stage0".to_string(),
            "stage1".to_string(),
            "stage2".to_string(),
            "stage3".to_string(),
            "stage4".to_string(),
        ];
        assert_eq!(select_starter_code(&exercise, 2.5), "stage2");
    }

    #[test]
    fn test_select_starter_code_stage_three() {
        let mut exercise = make_exercise("pointers", ValidationMode::Output, None);
        exercise.starter_code = "stage0".to_string();
        exercise.starter_code_stages = vec![
            "stage0".to_string(),
            "stage1".to_string(),
            "stage2".to_string(),
            "stage3".to_string(),
            "stage4".to_string(),
        ];
        assert_eq!(select_starter_code(&exercise, 3.5), "stage3");
    }

    #[test]
    fn test_select_starter_code_stage_four() {
        let mut exercise = make_exercise("pointers", ValidationMode::Output, None);
        exercise.starter_code = "stage0".to_string();
        exercise.starter_code_stages = vec![
            "stage0".to_string(),
            "stage1".to_string(),
            "stage2".to_string(),
            "stage3".to_string(),
            "stage4".to_string(),
        ];
        assert_eq!(select_starter_code(&exercise, 5.0), "stage4");
    }

    #[test]
    fn test_select_starter_code_fallback_empty_stages() {
        let mut exercise = make_exercise("pointers", ValidationMode::Output, None);
        exercise.starter_code = "default".to_string();
        exercise.starter_code_stages = vec![];
        assert_eq!(select_starter_code(&exercise, 3.5), "default");
    }

    #[test]
    fn test_select_starter_code_fallback_insufficient_stages() {
        let mut exercise = make_exercise("pointers", ValidationMode::Output, None);
        exercise.starter_code = "default".to_string();
        exercise.starter_code_stages = vec!["stage0".to_string(), "stage1".to_string()];
        // mastery 4.5 → stage 4, but only 2 stages available → fall back to default
        assert_eq!(select_starter_code(&exercise, 4.5), "default");
    }

    #[test]
    fn test_select_starter_code_partial_stages() {
        let mut exercise = make_exercise("pointers", ValidationMode::Output, None);
        exercise.starter_code = "default".to_string();
        exercise.starter_code_stages = vec![
            "stage0".to_string(),
            "stage1".to_string(),
            "stage2".to_string(),
        ];
        // mastery 1.5 → stage 1, which exists
        assert_eq!(select_starter_code(&exercise, 1.5), "stage1");
        // mastery 4.5 → stage 4, but only 3 stages available → fall back to default
        assert_eq!(select_starter_code(&exercise, 4.5), "default");
    }
}
