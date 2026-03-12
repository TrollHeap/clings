//! C code compilation and execution engine.
//!
//! Compiles user code with `gcc -Wall -Wextra -std=c11`, writes it to `~/.clings/current.c`,
//! runs it with a 10-second timeout, and validates stdout against expected output.
//! Supports `Output`, `Test`, and `Both` validation modes via `compile_and_run()`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::constants::{
    CLINGS_DIR, CURRENT_C_FILENAME, EXECUTION_TIMEOUT_SECS, GCC_BINARY, GCC_FLAGS,
    MAX_OUTPUT_BYTES, POLL_INTERVAL_MS, REGEX_PREFIX, TEST_SUMMARY_FAILURES, TEST_SUMMARY_IGNORED,
    TEST_SUMMARY_TESTS,
};
use crate::error::KfError;
use crate::models::{Exercise, ValidationMode};

/// Préfixe des messages de timeout — utilisé pour la création du message et les pattern matches.
const TIMEOUT_MSG_PREFIX: &str = "Délai d'exécution dépassé";

/// Nom du fichier de harnais de tests C copié dans le répertoire de travail.
const TEST_H_FILENAME: &str = "test.h";
/// Nom du fichier C généré qui inclut current.c + le code du harnais.
const TEST_C_FILENAME: &str = "test_current.c";

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
    if exercise.files.is_empty() {
        return Ok(());
    }
    let canonical_work = work_dir.canonicalize()?;
    for file in &exercise.files {
        if file.name.contains("..") || file.name.starts_with('/') {
            eprintln!(
                "  Warning: fichier ignoré (chemin invalide) : {}",
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

/// Compile `source_path` with gcc `extra_args`, run the resulting binary from
/// `work_dir`, and collect stdout + stderr within `timeout`.
/// Returns `(stdout, stderr, exit_status)` or a `KfError`.
fn spawn_gcc_and_collect(
    source_path: &Path,
    extra_args: &[&str],
    work_dir: &Path,
    timeout: Duration,
) -> crate::error::Result<(String, String, std::process::ExitStatus)> {
    // Build gcc command: fixed flags + caller-supplied args (output path, source, -I…, linker flags).
    let mut gcc = Command::new(GCC_BINARY);
    gcc.args(GCC_FLAGS);
    for arg in extra_args {
        gcc.arg(arg);
    }

    let compile_result = gcc.output().map_err(|e| {
        KfError::Io(std::io::Error::new(
            e.kind(),
            format!("Impossible de lancer gcc : {e}. gcc est-il installé ?"),
        ))
    })?;

    if !compile_result.status.success() {
        let stderr = String::from_utf8_lossy(&compile_result.stderr).to_string();
        return Err(KfError::Config(stderr));
    }

    // Derive the output binary path: gcc writes it via "-o <output_path>" which is the
    // first extra_arg after "-o". We locate it by scanning extra_args.
    let output_path = extra_args
        .windows(2)
        .find(|w| w[0] == "-o")
        .ok_or_else(|| KfError::Config("extra_args must contain -o <output>".to_string()))
        .map(|w| PathBuf::from(w[1]))?;

    let _ = source_path; // consumed by gcc via extra_args; kept in signature for clarity

    let start = Instant::now();
    let mut child = Command::new(&output_path)
        .current_dir(work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(KfError::Io)?;

    // INVARIANT: Stdio::piped() was requested — take() returns None only if already consumed,
    // which cannot happen here. Propagate as Config error rather than panic.
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| KfError::Config("stdout pipe non disponible".to_owned()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| KfError::Config("stderr pipe non disponible".to_owned()))?;
    let (stdout_thread, stderr_thread) = spawn_drain_threads(stdout, stderr);

    match child.wait_timeout(timeout) {
        Ok(Some(status)) => {
            let stdout = stdout_thread
                .join()
                .map_err(|_| KfError::Config("stdout reader thread paniqué".to_owned()))?;
            let stderr = stderr_thread
                .join()
                .map_err(|_| KfError::Config("stderr reader thread paniqué".to_owned()))?;
            Ok((stdout, stderr, status))
        }
        Ok(None) => {
            kill_and_drain(&child, stdout_thread, stderr_thread);
            let _ = child.wait(); // reap zombie; error already handled by timeout path
            let elapsed = start.elapsed();
            Err(KfError::Config(format!(
                "{TIMEOUT_MSG_PREFIX} ({:.1}s limite)",
                elapsed.as_secs_f64()
            )))
        }
        Err(e) => {
            kill_and_drain(&child, stdout_thread, stderr_thread);
            let _ = child.wait(); // reap zombie; real error propagated below
            Err(KfError::Io(e))
        }
    }
}

/// Compile and run a C source file, validating output against expected.
///
/// Compiles with `gcc -Wall -Wextra -std=c11`, runs with a 10-second timeout,
/// then compares normalized stdout to `exercise.validation.expected_output`
/// (exact or regex).
///
/// Returns a [`RunResult`] with `success`, compiler/runtime `message`, and
/// updated `mastery` score delta.
pub fn compile_and_run(source_path: &Path, exercise: &Exercise) -> RunResult {
    let tmp_fallback2 = std::path::PathBuf::from("/tmp");
    let work_dir = source_path.parent().unwrap_or_else(|| {
        eprintln!("avertissement : répertoire HOME indisponible, utilisation de /tmp");
        tmp_fallback2.as_path()
    });

    match exercise.validation.mode {
        ValidationMode::Test => run_tests(source_path, work_dir, exercise),
        ValidationMode::Both => {
            let output_result = run_output(source_path, work_dir, exercise);
            if !output_result.success {
                return output_result;
            }
            run_tests(source_path, work_dir, exercise)
        }
        ValidationMode::Output => run_output(source_path, work_dir, exercise),
    }
}

/// Run output-validation mode: compile source, run, compare stdout.
fn run_output(source_path: &Path, work_dir: &Path, exercise: &Exercise) -> RunResult {
    let output_path = work_dir.join("kf_run");

    if let Err(e) = write_exercise_files(exercise, work_dir) {
        return make_compile_error(format!("Impossible d'écrire les fichiers d'exercice : {e}"));
    }

    let output_path_str = output_path.to_string_lossy().into_owned();
    let source_path_str = source_path.to_string_lossy().into_owned();
    let include_flag = format!("-I{}", work_dir.display());
    let linker = linker_flags(&exercise.subject);

    let mut extra_args: Vec<&str> = vec!["-o", &output_path_str, &source_path_str, &include_flag];
    for flag in &linker {
        extra_args.push(flag);
    }

    let timeout = exercise
        .validation
        .max_duration_ms
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_secs(EXECUTION_TIMEOUT_SECS));

    let start = Instant::now();
    let gcc_result = spawn_gcc_and_collect(source_path, &extra_args, work_dir, timeout);
    dispatch_gcc_result(
        gcc_result,
        timeout,
        start,
        |stdout, stderr, status, duration_ms| {
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
            let valid = validate_output(&stdout, exercise);
            RunResult {
                success: valid,
                stdout,
                stderr,
                duration_ms,
                compile_error: false,
                timeout: false,
            }
        },
    )
}

/// Run test-harness mode: write test.h + test_current.c, compile, run, parse summary.
fn run_tests(source_path: &Path, work_dir: &Path, exercise: &Exercise) -> RunResult {
    let test_code = match &exercise.validation.test_code {
        Some(c) => c.as_str(),
        None => {
            return make_compile_error(
                "Mode Test : champ 'test_code' manquant dans l'exercice".to_string(),
            );
        }
    };

    // Write test.h from embedded asset
    let test_h_content = include_str!("../assets/test.h");
    let test_h_path = work_dir.join(TEST_H_FILENAME);
    if let Err(e) = std::fs::write(&test_h_path, test_h_content) {
        return make_compile_error(format!("Impossible d'écrire test.h : {e}"));
    }

    // Write test_current.c = #include "current.c" + test_code
    let test_c_path = work_dir.join(TEST_C_FILENAME);
    let source_filename = source_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| CURRENT_C_FILENAME.to_string());
    let test_c_content =
        format!("#include \"{source_filename}\"\n#include \"test.h\"\n\n{test_code}\n");
    if let Err(e) = std::fs::write(&test_c_path, &test_c_content) {
        return make_compile_error(format!("Impossible d'écrire test_current.c : {e}"));
    }

    if let Err(e) = write_exercise_files(exercise, work_dir) {
        return make_compile_error(format!("Impossible d'écrire les fichiers d'exercice : {e}"));
    }

    let output_path = work_dir.join("kf_test");
    let output_path_str = output_path.to_string_lossy().into_owned();
    let test_c_path_str = test_c_path.to_string_lossy().into_owned();
    let include_flag = format!("-I{}", work_dir.display());
    let linker = linker_flags(&exercise.subject);

    let mut extra_args: Vec<&str> = vec!["-o", &output_path_str, &test_c_path_str, &include_flag];
    for flag in &linker {
        extra_args.push(flag);
    }

    let timeout = exercise
        .validation
        .max_duration_ms
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_secs(EXECUTION_TIMEOUT_SECS));

    let start = Instant::now();
    let gcc_result = spawn_gcc_and_collect(&test_c_path, &extra_args, work_dir, timeout);
    let expected_pass = exercise.validation.expected_tests_pass;
    dispatch_gcc_result(
        gcc_result,
        timeout,
        start,
        move |stdout, stderr, status, duration_ms| {
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
            let (success, failures) = parse_test_summary(&stdout);
            let passed = success && failures == 0;
            let result_ok = match expected_pass {
                Some(n) => {
                    // Count OK lines in stdout
                    let ok_count = stdout
                        .lines()
                        .filter(|l| l.trim_start().starts_with("OK"))
                        .count();
                    passed && ok_count >= n
                }
                None => passed,
            };
            RunResult {
                success: result_ok,
                stdout,
                stderr,
                duration_ms,
                compile_error: false,
                timeout: false,
            }
        },
    )
}

/// Parse the test summary line: "N Tests N Failures 0 Ignored".
/// Returns `(found_summary, failures_count)`.
fn parse_test_summary(stdout: &str) -> (bool, usize) {
    for line in stdout.lines().rev() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Format: "<N> Tests <M> Failures 0 Ignored"
        if parts.len() == 6
            && parts[1] == TEST_SUMMARY_TESTS
            && parts[3] == TEST_SUMMARY_FAILURES
            && parts[5] == TEST_SUMMARY_IGNORED
        {
            let failures = parts[2].parse::<usize>().unwrap_or(1);
            return (true, failures);
        }
    }
    (false, 1) // No summary line found → treat as failure
}

/// Dispatch le résultat de `spawn_gcc_and_collect` vers un `RunResult`.
///
/// Le bras `Ok` est délégué à `on_ok(stdout, stderr, status, duration_ms)`.
/// Les bras d'erreur communs (timeout, io, config) sont traités ici.
fn dispatch_gcc_result(
    gcc_result: crate::error::Result<(String, String, std::process::ExitStatus)>,
    timeout: Duration,
    start: Instant,
    on_ok: impl FnOnce(String, String, std::process::ExitStatus, u64) -> RunResult,
) -> RunResult {
    match gcc_result {
        Ok((stdout, stderr, status)) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            on_ok(stdout, stderr, status, duration_ms)
        }
        Err(KfError::Config(msg)) if msg.starts_with(TIMEOUT_MSG_PREFIX) => RunResult {
            success: false,
            stdout: String::new(),
            stderr: msg,
            duration_ms: timeout.as_millis() as u64,
            compile_error: false,
            timeout: true,
        },
        Err(KfError::Config(msg)) => make_compile_error(msg),
        Err(KfError::Io(e)) => RunResult {
            success: false,
            stdout: String::new(),
            stderr: format!("Wait error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
            compile_error: false,
            timeout: false,
        },
        Err(e) => make_compile_error(format!("{e}")),
    }
}

/// Validate program output against expected output.
/// If expected_output starts with "REGEX:" the remainder is compiled as a regex
/// and matched against the full (normalized) stdout.
fn validate_output(stdout: &str, exercise: &Exercise) -> bool {
    if let Some(expected) = &exercise.validation.expected_output {
        let norm_out = normalize(stdout);
        let norm_exp = normalize(expected);
        if let Some(pattern) = norm_exp.strip_prefix(REGEX_PREFIX) {
            let key = pattern.trim().to_string();
            REGEX_CACHE.with(|cache| {
                let mut map = cache.borrow_mut();
                let re = map.entry(key.clone()).or_insert_with(|| {
                    let compiled = regex::Regex::new(&key);
                    if let Err(ref e) = compiled {
                        eprintln!("Avertissement : regex invalide dans l'exercice ({key:?}): {e}");
                    }
                    // .ok(): invalid regex → logged above; None causes is_match to return false
                    compiled.ok()
                });
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
pub fn work_dir() -> crate::error::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        KfError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Variable $HOME non définie — impossible de localiser ~/.clings",
        ))
    })?;
    let dir = PathBuf::from(home).join(CLINGS_DIR);
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
    std::fs::rename(&temp_path, &source_path)?;

    write_exercise_files(exercise, &dir)?;

    Ok(source_path)
}

/// Charge la mastery d'un sujet et écrit le starter code adapté au stage.
/// Retourne (source_path, current_stage).
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

/// Spawn background threads to drain stdout and stderr from a child process.
/// Returns (stdout_thread, stderr_thread) handles so the caller can join them.
/// Taking the handles prevents pipe-buffer deadlock when the child writes > ~64 KB.
fn spawn_drain_threads(
    stdout: std::process::ChildStdout,
    stderr: std::process::ChildStderr,
) -> (
    std::thread::JoinHandle<String>,
    std::thread::JoinHandle<String>,
) {
    let stdout_thread = std::thread::spawn(move || -> String {
        let mut buf = String::new();
        // .ok(): partial read is acceptable — we cap at MAX_OUTPUT_BYTES anyway
        std::io::Read::read_to_string(&mut std::io::Read::take(stdout, MAX_OUTPUT_BYTES), &mut buf)
            .ok();
        buf
    });
    let stderr_thread = std::thread::spawn(move || -> String {
        let mut buf = String::new();
        // .ok(): partial read is acceptable — we cap at MAX_OUTPUT_BYTES anyway
        std::io::Read::read_to_string(&mut std::io::Read::take(stderr, MAX_OUTPUT_BYTES), &mut buf)
            .ok();
        buf
    });
    (stdout_thread, stderr_thread)
}

/// Construct a RunResult representing a compile failure.
fn make_compile_error(stderr: String) -> RunResult {
    RunResult {
        success: false,
        stdout: String::new(),
        stderr,
        duration_ms: 0,
        compile_error: true,
        timeout: false,
    }
}

/// Kill the process group and join drain threads, logging any thread panics.
/// Used in error arms of `spawn_gcc_and_collect` where the collected output is discarded.
fn kill_and_drain(
    child: &std::process::Child,
    stdout_thread: std::thread::JoinHandle<String>,
    stderr_thread: std::thread::JoinHandle<String>,
) {
    kill_process_group(child);
    if let Err(e) = stdout_thread.join() {
        eprintln!("Avertissement : thread lecteur stdout a paniqué : {e:?}");
    }
    if let Err(e) = stderr_thread.join() {
        eprintln!("Avertissement : thread lecteur stderr a paniqué : {e:?}");
    }
}

/// Kill the entire process group of a child to avoid zombie fork-bombs.
fn kill_process_group(child: &std::process::Child) {
    let pid = child.id();
    if pid == 0 {
        return;
    }
    // SAFETY: pid was obtained from Child::id() which guarantees a valid positive PID.
    // Negating it sends SIGKILL to the entire process group, which is intentional
    // to clean up any subprocesses spawned by the C program.
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
    }
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
        // Poll-based busy-wait: std does not expose async wait.
        // POLL_INTERVAL_MS (50 ms) keeps CPU usage negligible while staying
        // responsive enough for a 10-second exercise timeout.
        let start = Instant::now();
        loop {
            match self.try_wait()? {
                Some(status) => return Ok(Some(status)),
                None => {
                    if start.elapsed() >= timeout {
                        return Ok(None);
                    }
                    std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                }
            }
        }
    }
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
        let exercise = make_exercise("pointers", Some("42".to_string()));
        assert!(validate_output("42", &exercise));
    }

    #[test]
    fn test_validate_output_mismatch() {
        let exercise = make_exercise("pointers", Some("42".to_string()));
        assert!(!validate_output("43", &exercise));
    }

    #[test]
    fn test_validate_output_whitespace_normalization() {
        let exercise = make_exercise("pointers", Some("hello\n  world".to_string()));
        assert!(validate_output("hello  \n  world  ", &exercise));
    }

    #[test]
    fn test_validate_output_regex() {
        let exercise = make_exercise("pointers", Some("REGEX:^[0-9]+$".to_string()));
        assert!(validate_output("42", &exercise));
        assert!(!validate_output("abc", &exercise));
    }

    #[test]
    fn test_validate_output_regex_with_whitespace() {
        let exercise = make_exercise("pointers", Some("REGEX:^hello\\s+world$".to_string()));
        assert!(validate_output("hello   world", &exercise));
        assert!(!validate_output("hello world extra", &exercise));
    }

    #[test]
    fn test_validate_output_regex_invalid() {
        let exercise = make_exercise("pointers", Some("REGEX:[invalid(".to_string()));
        assert!(!validate_output("anything", &exercise));
    }

    #[test]
    fn test_validate_output_no_expected() {
        let exercise = make_exercise("pointers", None);
        assert!(validate_output("anything", &exercise));
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
        let mut exercise = make_exercise("pointers", None);
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
        let mut exercise = make_exercise("pointers", None);
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
        let mut exercise = make_exercise("pointers", None);
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
        let mut exercise = make_exercise("pointers", None);
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
        let mut exercise = make_exercise("pointers", None);
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
        let mut exercise = make_exercise("pointers", None);
        exercise.starter_code = "default".to_string();
        exercise.starter_code_stages = vec![];
        assert_eq!(select_starter_code(&exercise, 3.5), "default");
    }

    #[test]
    fn test_select_starter_code_fallback_insufficient_stages() {
        let mut exercise = make_exercise("pointers", None);
        exercise.starter_code = "default".to_string();
        exercise.starter_code_stages = vec!["stage0".to_string(), "stage1".to_string()];
        // mastery 4.5 → stage 4, but only 2 stages available → fall back to default
        assert_eq!(select_starter_code(&exercise, 4.5), "default");
    }

    #[test]
    fn test_select_starter_code_partial_stages() {
        let mut exercise = make_exercise("pointers", None);
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
        assert_eq!(failures, 1); // treated as failure
    }

    #[test]
    fn test_parse_test_summary_empty() {
        let (found, failures) = parse_test_summary("");
        assert!(!found);
        assert_eq!(failures, 1);
    }
}
