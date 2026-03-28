//! C code compilation and execution engine.
//!
//! Compiles user code with `gcc -Wall -Wextra -std=c11`, writes it to `~/.clings/current.c`,
//! runs it with a 10-second timeout, and validates stdout against expected output.
//! Supports `Output` and `Test` (Unity) validation modes via `compile_and_run()`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::constants::{CURRENT_C_FILENAME, EXECUTION_TIMEOUT_SECS, REGEX_PREFIX};
use crate::error::KfError;
use crate::models::{Exercise, ValidationMode};

// Submodules: exec provides process execution primitives, unity provides test framework integration
pub mod exec;
pub mod unity;

#[cfg(test)]
use exec::wait_for_process_with_timeout;

// Re-export submodule items used by other modules in this crate
pub use exec::spawn_gcc_and_collect;
pub use unity::run_tests;

/// Résultat de la compilation et de l'exécution d'un exercice C.
#[derive(Clone, Debug)]
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

/// Retourne les flags de liaison spécifiques au sujet pour gcc.
/// - Sujets threads (pthreads, semaphores, sync_concepts, sockets, capstones) : `-lpthread`
/// - Sujets IPC multi (message_queues, shared_memory) : `-lrt -lpthread`
/// - Sujets file_io : `-lrt`
/// - Autres : aucun flag
pub fn linker_flags(subject: &str) -> Vec<&'static str> {
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

/// Compile et exécute un fichier source C, valide la sortie.
///
/// Compile avec `gcc -Wall -Wextra -std=c11 -D_GNU_SOURCE`, exécute avec timeout 10s,
/// puis compare stdout normalisé à `exercise.validation.expected_output`
/// (comparaison exacte ou regex si préfixe `REGEX:`).
///
/// Retourne [`RunResult`] avec `success`, messages compiler/runtime et durée.
/// La mise à jour du score de maîtrise est effectuée par l'appelant.
///
/// # Never panics
/// Toutes les erreurs (compilation, timeout, mismatch output) sont capturées
/// dans `RunResult` — jamais de panic.
pub fn compile_and_run(source_path: &Path, exercise: &Exercise) -> RunResult {
    let fallback_work_dir = std::path::PathBuf::from("/tmp");
    let work_dir = source_path.parent().unwrap_or_else(|| {
        eprintln!("avertissement : répertoire HOME indisponible, utilisation de /tmp");
        fallback_work_dir.as_path()
    });

    match exercise.validation.mode {
        ValidationMode::Output => run_output(source_path, work_dir, exercise),
        ValidationMode::Test => run_tests(source_path, work_dir, exercise),
    }
}

/// Constructs gcc command-line arguments for compilation.
fn build_gcc_compilation_args<'a>(
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
        build_gcc_compilation_args(&output_path_str, &source_path_str, &include_flag, &linker);

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

thread_local! {
    /// Per-thread cache of compiled regexes: pattern string → compiled Regex (or None if invalid).
    static REGEX_CACHE: RefCell<HashMap<String, Option<regex::Regex>>> =
        RefCell::new(HashMap::new());
}

/// Dispatch the result of a gcc compilation: handle success, timeout, or compilation error.
/// Allows the caller to provide a closure that processes (stdout, stderr, status, duration_ms).
fn dispatch_gcc_result<F>(
    gcc_result: crate::error::Result<(String, String, std::process::ExitStatus)>,
    timeout: Duration,
    start: Instant,
    handler: F,
) -> RunResult
where
    F: FnOnce(String, String, std::process::ExitStatus, u64) -> RunResult,
{
    match gcc_result {
        Ok((stdout, stderr, status)) => {
            let duration_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
            handler(stdout, stderr, status, duration_ms)
        }
        Err(KfError::Config(msg)) if msg.contains(exec::TIMEOUT_MSG_PREFIX) => RunResult {
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

/// Construct a RunResult representing a compile failure.
pub fn make_compile_error(stderr: String) -> RunResult {
    let hint = parse_gcc_hint(&stderr);
    RunResult {
        success: false,
        stdout: String::new(),
        stderr,
        duration_ms: 0,
        compile_error: true,
        timeout: false,
        gcc_hint: hint,
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
            libsys_module: None,
            libsys_function: None,
            libsys_unlock: None,
            header_code: None,
        }
    }

    #[test]
    fn test_normalize_removes_whitespace() {
        assert_eq!(normalize("hello"), "hello");
        assert_eq!(normalize("  hello  "), "hello");
        assert_eq!(normalize("hello\n\n"), "hello");
        assert_eq!(normalize("hello   \nworld"), "hello\nworld");
    }

    #[test]
    fn test_normalize_preserves_internal_structure() {
        let input = "line 1  \nline 2  \nline 3";
        let expected = "line 1\nline 2\nline 3";
        assert_eq!(normalize(input), expected);
    }

    #[test]
    fn test_validate_output_exact_match() {
        let mut exercise = make_exercise("test", Some("expected".to_string()));
        exercise.validation.expected_output = Some("expected".to_string());
        assert!(validate_output("expected", &exercise));
        assert!(!validate_output("wrong", &exercise));
    }

    #[test]
    fn test_validate_output_regex_match() {
        let mut exercise = make_exercise("test", Some("REGEX:\\d+".to_string()));
        exercise.validation.expected_output = Some("REGEX:\\d+".to_string());
        assert!(validate_output("42", &exercise));
        assert!(!validate_output("abc", &exercise));
    }

    #[test]
    fn test_validate_output_no_expected() {
        let exercise = make_exercise("test", None);
        assert!(validate_output("anything", &exercise));
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
        // The ".." is caught by the contains("..") check
        assert!(result.is_ok()); // write_exercise_files logs and skips, doesn't error
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
        // process_group(0) makes the child its own group leader so that
        // kill_process_group (which sends SIGKILL to -pgid) actually reaches it.
        let mut child = std::process::Command::new("sleep")
            .arg("100")
            .process_group(0)
            .spawn()
            .expect("sleep must be available on Linux");

        let timeout = std::time::Duration::from_millis(100);
        let result = wait_for_process_with_timeout(&mut child, timeout);

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
        // pthreads, semaphores, sync_concepts, sockets, capstones all need -lpthread
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
        // message_queues and shared_memory need both -lrt and -lpthread
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
        // file_io needs -lrt only
        let flags = linker_flags("file_io");
        assert_eq!(flags, vec!["-lrt"], "file_io should only have -lrt");
    }

    #[test]
    fn test_linker_flags_no_special_flags() {
        // pointers, structs, processes, etc. should have no special flags
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
        // Absolute paths should be skipped (eprintln warning) but function returns Ok
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
        // work_dir() should create the directory if it doesn't exist
        // and return Ok with a valid path
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
