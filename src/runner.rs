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
    CURRENT_C_FILENAME, EXECUTION_TIMEOUT_SECS, GCC_BINARY, GCC_FLAGS, MAX_OUTPUT_BYTES,
    REGEX_PREFIX, TEST_SUMMARY_FAILURES, TEST_SUMMARY_IGNORED, TEST_SUMMARY_TESTS,
};
use crate::error::KfError;
use crate::models::Exercise;

/// Préfixe des messages de timeout — utilisé pour la création du message et les pattern matches.
const TIMEOUT_MSG_PREFIX: &str = "Délai d'exécution dépassé";

// v1.1 : mode test désactivé — fonctions et constantes conservées pour v1.2
#[allow(dead_code)]
/// Nom du fichier de harnais de tests C copié dans le répertoire de travail.
const TEST_H_FILENAME: &str = "test.h";
#[allow(dead_code)]
/// Nom du fichier C généré qui inclut current.c + le code du harnais.
const TEST_C_FILENAME: &str = "test_current.c";

#[allow(dead_code)]
/// Patterns C interdits dans `test_code` — prévient l'injection de code via exercices externes.
const FORBIDDEN_TEST_CODE_PATTERNS: &[&str] = &[
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

/// Supprime la fonction `main()` du code C utilisateur pour éviter la redéfinition
/// lors de la compilation du harness de tests (test_current.c intègre le code utilisateur).
///
/// Trouve le premier `int main`, localise la `{` ouvrante, et retire le bloc entier
/// jusqu'à la `}` correspondante (profondeur d'accolades). Si `main` est absent,
/// retourne le code inchangé.
fn strip_main_function(code: &str) -> String {
    let Some(main_pos) = code.find("int main") else {
        return code.to_string();
    };
    let after_main = &code[main_pos..];
    let Some(brace_offset) = after_main.find('{') else {
        return code.to_string();
    };
    let brace_start = main_pos + brace_offset;
    let mut depth = 0usize;
    let mut end_pos = None;
    for (i, ch) in code[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_pos = Some(brace_start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end_pos.unwrap_or(code.len());
    format!("{}{}", &code[..main_pos], &code[end..])
}

#[allow(dead_code)]
/// Valide `test_code` avant d'écrire le fichier C généré.
/// Retourne `Some(pattern)` si un pattern interdit est trouvé, `None` sinon.
fn validate_test_code(code: &str) -> Option<&'static str> {
    FORBIDDEN_TEST_CODE_PATTERNS
        .iter()
        .copied()
        .find(|&pat| code.contains(pat))
}

// Per-thread cache of compiled regexes: pattern string → compiled Regex (or None if invalid).
thread_local! {
    static REGEX_CACHE: RefCell<HashMap<String, Option<regex::Regex>>> =
        RefCell::new(HashMap::new());
}

/// Résultat de la compilation et de l'exécution d'un exercice C.
#[derive(Clone)]
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
    /// Conseil pédagogique dérivé du stderr gcc (None si pas d'erreur de compilation)
    pub gcc_hint: Option<String>,
}

/// Analyse le stderr gcc et retourne un conseil pédagogique pour les erreurs courantes.
pub fn parse_gcc_hint(stderr: &str) -> Option<String> {
    let patterns: &[(&str, &str)] = &[
        (
            "implicit declaration of function",
            "Fonction utilisée sans déclaration — ajoutez l'#include correspondant",
        ),
        (
            "undefined reference to",
            "Symbole non résolu — vérifiez l'include ou le flag de liaison (-lpthread, -lm…)",
        ),
        (
            "undeclared",
            "Identifiant non déclaré — vérifiez le nom et la portée de la variable/fonction",
        ),
        (
            "expected ';'",
            "Point-virgule manquant — repérez la ligne indiquée par gcc",
        ),
        (
            "too few arguments",
            "Trop peu d'arguments passés à la fonction",
        ),
        (
            "too many arguments",
            "Trop d'arguments passés à la fonction",
        ),
        (
            "incompatible types",
            "Types incompatibles — vérifiez les conversions ou un cast manquant",
        ),
        (
            "dereferencing pointer to incomplete type",
            "Pointeur vers un type incomplet — le struct est-il défini avant cet usage ?",
        ),
        ("unused variable", "Variable déclarée mais jamais utilisée"),
        (
            "control reaches end of non-void function",
            "La fonction doit retourner une valeur sur tous les chemins d'exécution",
        ),
    ];

    for (pattern, hint) in patterns {
        if stderr.contains(pattern) {
            return Some(hint.to_string());
        }
    }
    None
}

/// Returns subject-specific linker flags for gcc.
/// Threads subjects need `-lpthread`, IPC subjects need `-lrt`, others need none.
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

    let mut child = Command::new(&output_path)
        .current_dir(work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(KfError::Io)?;

    // INVARIANT: Stdio::piped() was requested — take() returns None only if already consumed,
    // which cannot happen here. Kill child and propagate as Config error rather than panic.
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            kill_process_group(&child);
            return Err(KfError::Config("stdout pipe non disponible".to_owned()));
        }
    };
    let stderr = match child.stderr.take() {
        Some(s) => s,
        None => {
            kill_process_group(&child);
            return Err(KfError::Config("stderr pipe non disponible".to_owned()));
        }
    };
    let (stdout_thread, stderr_thread) = spawn_drain_threads(stdout, stderr);

    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    kill_and_drain(&child, stdout_thread, stderr_thread);
                    // reap zombie — ECHILD = déjà récolté (attendu après kill), autres erreurs loguées
                    if let Err(e) = child.wait() {
                        if e.raw_os_error() != Some(libc::ECHILD) {
                            eprintln!("[clings/runner] avertissement : reap zombie échoué : {e}");
                        }
                    }
                    return Err(KfError::Config(format!(
                        "{TIMEOUT_MSG_PREFIX} ({:.1}s limite)",
                        timeout.as_secs_f64()
                    )));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => {
                kill_and_drain(&child, stdout_thread, stderr_thread);
                // reap zombie — ECHILD = déjà récolté (attendu après kill), autres erreurs loguées
                if let Err(we) = child.wait() {
                    if we.raw_os_error() != Some(libc::ECHILD) {
                        eprintln!("[clings/runner] avertissement : reap zombie échoué : {we}");
                    }
                }
                return Err(KfError::Io(e));
            }
        }
    };
    let stdout = stdout_thread
        .join()
        .map_err(|_| KfError::Config("stdout reader thread paniqué".to_owned()))?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| KfError::Config("stderr reader thread paniqué".to_owned()))?;
    Ok((stdout, stderr, status))
}

/// Compile and run a C source file, validating output against expected.
///
/// Compiles with `gcc -Wall -Wextra -std=c11`, runs with a 10-second timeout,
/// then compares normalized stdout to `exercise.validation.expected_output`
/// (exact or regex).
///
/// Returns a [`RunResult`] with `success`, compiler/runtime `message`, and
/// updated `mastery` score delta.
///
/// # Never panics
/// Toutes les erreurs (compilation, timeout, output mismatch) sont capturées
/// dans `RunResult` — jamais de panic.
pub fn compile_and_run(source_path: &Path, exercise: &Exercise) -> RunResult {
    let fallback_work_dir = std::path::PathBuf::from("/tmp");
    let work_dir = source_path.parent().unwrap_or_else(|| {
        eprintln!("avertissement : répertoire HOME indisponible, utilisation de /tmp");
        fallback_work_dir.as_path()
    });

    // v1.1 : mode output uniquement — tous les exercices utilisent la comparaison stdout
    run_output(source_path, work_dir, exercise)
}

/// Constructs gcc command-line arguments for output-validation mode.
fn build_output_gcc_args<'a>(
    output_path: &'a str,
    source_path: &'a str,
    include_flag: &'a str,
    linker_flags: &[&'a str],
) -> Vec<&'a str> {
    let mut args = vec!["-o", output_path, source_path, include_flag];
    args.extend_from_slice(linker_flags);
    args
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

    let extra_args =
        build_output_gcc_args(&output_path_str, &source_path_str, &include_flag, &linker);

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
                    gcc_hint: None,
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
                gcc_hint: None,
            }
        },
    )
}

#[allow(dead_code)]
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

    if let Some(forbidden) = validate_test_code(test_code) {
        return make_compile_error(format!(
            "test_code invalide : pattern interdit détecté (`{forbidden}`)"
        ));
    }

    // Write test.h from embedded asset
    let test_h_content = include_str!("../assets/test.h");
    let test_h_path = work_dir.join(TEST_H_FILENAME);
    if let Err(e) = std::fs::write(&test_h_path, test_h_content) {
        return make_compile_error(format!("Impossible d'écrire test.h : {e}"));
    }

    // Write test_current.c = user code (sans main) + test.h + test_code
    // On inline le contenu de current.c plutôt qu'un #include pour pouvoir supprimer
    // le main() utilisateur qui entrerait en conflit avec le main() du harness.
    let test_c_path = work_dir.join(TEST_C_FILENAME);
    let source_filename = source_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| CURRENT_C_FILENAME.to_string());
    let user_code = match std::fs::read_to_string(source_path) {
        Ok(c) => c,
        Err(e) => return make_compile_error(format!("Impossible de lire {source_filename} : {e}")),
    };
    let user_code_no_main = strip_main_function(&user_code);
    // #line préserve les numéros de ligne dans les messages d'erreur gcc
    let test_c_content = format!(
        "#line 1 \"{source_filename}\"\n{user_code_no_main}\n#include \"test.h\"\n\n{test_code}\n"
    );
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
                    gcc_hint: None,
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
                gcc_hint: None,
            }
        },
    )
}

#[allow(dead_code)]
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
            let duration_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
            on_ok(stdout, stderr, status, duration_ms)
        }
        Err(KfError::Config(msg)) if msg.starts_with(TIMEOUT_MSG_PREFIX) => RunResult {
            success: false,
            stdout: String::new(),
            stderr: msg,
            duration_ms: timeout.as_millis().min(u64::MAX as u128) as u64,
            compile_error: false,
            timeout: true,
            gcc_hint: None,
        },
        Err(KfError::Config(msg)) => make_compile_error(msg),
        Err(KfError::Io(e)) => RunResult {
            success: false,
            stdout: String::new(),
            stderr: format!("Wait error: {e}"),
            duration_ms: start.elapsed().as_millis().min(u64::MAX as u128) as u64,
            compile_error: false,
            timeout: false,
            gcc_hint: None,
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
                let re = map.entry(key).or_insert_with_key(|k| {
                    if k.len() > crate::constants::MAX_REGEX_PATTERN_LEN {
                        eprintln!(
                            "Avertissement : pattern regex trop long ({} octets, max {}), ignoré.",
                            k.len(),
                            crate::constants::MAX_REGEX_PATTERN_LEN
                        );
                        return None;
                    }
                    let compiled = regex::Regex::new(k);
                    if let Err(ref e) = compiled {
                        eprintln!("Avertissement : regex invalide dans l'exercice ({k:?}): {e}");
                    }
                    compiled.ok()
                });
                re.as_ref().is_some_and(|r| r.is_match(&norm_out))
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
/// `.lines()` already splits on `\r\n`, `\n`, and `\r` — no pre-replace needed.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line.trim_end());
    }
    out.trim().to_string()
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

/// Select the appropriate starter code stage based on mastery score.
/// Higher mastery → harder stage (less scaffolding).
#[must_use]
pub fn select_starter_code(exercise: &Exercise, mastery: f64) -> &str {
    let stage = mastery_to_stage(mastery) as usize;
    exercise
        .starter_code_stages
        .get(stage)
        .map(|s| s.as_str())
        .unwrap_or(&exercise.starter_code)
}

/// Écrit le code source de l'exercice dans `~/.clings/current.c` via temp-file+rename atomique.
///
/// # Errors
/// `std::io::Error` if `$HOME` is not set or if the write/rename operation fails.
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

    write_exercise_files(exercise, &dir)?;

    Ok(source_path)
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
        // lecture plafonnée à MAX_OUTPUT_BYTES ; erreur I/O loguée pour diagnostic
        if let Err(e) = std::io::Read::read_to_string(
            &mut std::io::Read::take(stdout, MAX_OUTPUT_BYTES),
            &mut buf,
        ) {
            eprintln!("[clings/runner] avertissement : lecture pipe stdout : {e}");
        }
        buf
    });
    let stderr_thread = std::thread::spawn(move || -> String {
        let mut buf = String::new();
        // lecture plafonnée à MAX_OUTPUT_BYTES ; erreur I/O loguée pour diagnostic
        if let Err(e) = std::io::Read::read_to_string(
            &mut std::io::Read::take(stderr, MAX_OUTPUT_BYTES),
            &mut buf,
        ) {
            eprintln!("[clings/runner] avertissement : lecture pipe stderr : {e}");
        }
        buf
    });
    (stdout_thread, stderr_thread)
}

/// Construct a RunResult representing a compile failure.
fn make_compile_error(stderr: String) -> RunResult {
    let gcc_hint = parse_gcc_hint(&stderr);
    RunResult {
        success: false,
        stdout: String::new(),
        stderr,
        duration_ms: 0,
        compile_error: true,
        timeout: false,
        gcc_hint,
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
    // SAFETY: pid was obtained from Child::id() which returns u32 > 0 for valid processes;
    // we checked pid != 0 above. Negating it sends SIGKILL to the entire process group,
    // which is intentional to clean up any subprocesses spawned by the C program.
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
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
    fn test_linker_flags() {
        let cases: &[(&str, &[&str])] = &[
            ("pthreads", &["-lpthread"]),
            ("semaphores", &["-lpthread"]),
            ("sync_concepts", &["-lpthread"]),
            ("sockets", &["-lpthread"]),
            ("capstones", &["-lpthread"]),
            ("message_queues", &["-lrt", "-lpthread"]),
            ("shared_memory", &["-lrt", "-lpthread"]),
            ("file_io", &["-lrt"]),
            ("unknown_subject", &[]),
            ("pointers", &[]),
        ];
        for (subject, expected) in cases {
            assert_eq!(
                linker_flags(subject),
                *expected,
                "linker_flags({subject:?})"
            );
        }
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

    #[test]
    fn test_validate_test_code_detects_forbidden_patterns() {
        let cases = &[
            ("system(\"ls\");", "system("),
            ("popen(cmd, \"r\");", "popen("),
            ("execv(\"/bin/sh\", args);", "execv("),
            ("dlopen(\"lib.so\", RTLD_LAZY);", "dlopen("),
            ("fork();", "fork("),
            ("kill(pid, SIGTERM);", "kill("),
            ("ptrace(PTRACE_ATTACH, pid, 0, 0);", "ptrace("),
            ("setuid(0);", "setuid("),
        ];
        for (code, expected_pattern) in cases {
            let result = validate_test_code(code);
            assert_eq!(
                result,
                Some(*expected_pattern),
                "should detect {expected_pattern:?} in {code:?}"
            );
        }
    }

    #[test]
    fn test_validate_test_code_allows_safe_code() {
        let safe_code = r#"
            void test_add(void) {
                int result = add(2, 3);
                TEST_ASSERT_EQUAL_INT(5, result);
            }
            int main(void) {
                RUN_TEST(test_add);
                TEST_SUMMARY();
                return _clings_failures > 0 ? 1 : 0;
            }
        "#;
        assert_eq!(validate_test_code(safe_code), None);
    }

    #[test]
    fn test_strip_main_function_basic() {
        let code = "void swap(int *a, int *b) { int t = *a; *a = *b; *b = t; }\nint main(void) {\n    return 0;\n}\n";
        let result = strip_main_function(code);
        assert!(!result.contains("int main"), "main() should be removed");
        assert!(result.contains("void swap"), "swap() should remain");
    }

    #[test]
    fn test_strip_main_function_nested_braces() {
        let code = "int foo(void) { return 1; }\nint main(void) {\n    if (1) { int x = 0; }\n    return 0;\n}\n";
        let result = strip_main_function(code);
        assert!(!result.contains("int main"), "main() should be removed");
        assert!(result.contains("int foo"), "foo() should remain");
    }

    #[test]
    fn test_strip_main_function_no_main() {
        let code = "void helper(void) { }\n";
        let result = strip_main_function(code);
        assert_eq!(result, code, "code without main should be unchanged");
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
        // The ".." is caught by the contains("..") check
        assert!(result.is_ok()); // write_exercise_files logs and skips, doesn't error
        assert!(
            !dir.parent().unwrap().join("escape.txt").exists(),
            "path traversal should not create files outside work_dir"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
