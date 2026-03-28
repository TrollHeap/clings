//! GCC compilation pipeline, output validation, and RunResult.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::constants::{EXECUTION_TIMEOUT_SECS, REGEX_PREFIX};
use crate::error::KfError;
use crate::models::{Exercise, ValidationMode};

use super::exec;
use super::exec::spawn_gcc_and_collect;
use super::files::write_exercise_files;
use super::unity::run_tests;

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

thread_local! {
    /// Per-thread cache of compiled regexes: pattern string → compiled Regex (or None if invalid).
    static REGEX_CACHE: RefCell<HashMap<String, Option<regex::Regex>>> =
        RefCell::new(HashMap::new());
}

/// Dispatch the result of a gcc compilation: handle success, timeout, or compilation error.
/// Allows the caller to provide a closure that processes (stdout, stderr, status, duration_ms).
pub(super) fn dispatch_gcc_result<F>(
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
}
