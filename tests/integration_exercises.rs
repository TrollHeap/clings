//! Tests d'intégration exercices — vérifie que solution_code compile
//! et produit le expected_output attendu pour chaque exercice output/both.
//!
//! Exécuter avec : `make test-exercises`
//! (ou `cargo test --test integration_exercises -- --nocapture`)

use std::process::Command;

use clings::models::ValidationMode;

/// Normalise la sortie : trim global + trim trailing whitespace par ligne.
/// Identique à `runner::normalize` pour garantir la cohérence.
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

/// Flags de linker par sujet (miroir de runner::linker_flags).
fn linker_flags(subject: &str) -> &'static [&'static str] {
    match subject {
        "pthreads" | "semaphores" | "sync_concepts" | "sockets" | "capstones" => &["-lpthread"],
        "message_queues" | "shared_memory" => &["-lrt", "-lpthread"],
        "file_io" => &["-lrt"],
        _ => &[],
    }
}

/// Vérifie que solution_code de chaque exercice output/both compile
/// et produit exactement le expected_output déclaré.
///
/// Les exercices `mode: "test"` sont ignorés (pas de expected_output stdout).
#[test]
fn solution_code_matches_expected_output() {
    // Pointe CLINGS_EXERCISES sur exercises/ dans le répertoire projet
    // pour que load_all_exercises utilise les fichiers courants.
    let exercises_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/exercises");
    // SAFETY: set_var est process-wide. Ce test est seul dans son binaire
    // (--test integration_exercises) donc pas de course avec d'autres tests.
    unsafe {
        std::env::set_var(clings::constants::EXERCISES_ENV_VAR, exercises_dir);
    }

    let (exercises, _) = clings::exercises::load_all_exercises().expect("chargement des exercices");

    let mut failures: Vec<String> = Vec::new();
    let mut skipped = 0usize;

    for exercise in &exercises {
        // Ignorer les exercices test-only : solution_code n'a pas de main()
        if matches!(exercise.validation.mode, ValidationMode::Test) {
            skipped += 1;
            continue;
        }

        // Ignorer les exercices sans expected_output ou avec validation regex
        let expected = match &exercise.validation.expected_output {
            Some(e) if !e.starts_with("REGEX:") => e,
            _ => {
                skipped += 1;
                continue;
            }
        };

        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("current.c");
        let bin = dir.path().join("solution_out");

        std::fs::write(&src, &exercise.solution_code).expect("écriture solution_code");

        // Fichiers auxiliaires (headers custom)
        for f in &exercise.files {
            if f.name.contains("..") || f.name.starts_with('/') {
                continue;
            }
            std::fs::write(dir.path().join(&f.name), &f.content)
                .expect("écriture fichier auxiliaire");
        }

        // Compilation
        let mut gcc = Command::new("gcc");
        gcc.args(["-Wall", "-Wextra", "-std=c11", "-D_GNU_SOURCE"])
            .arg("-o")
            .arg(&bin)
            .arg(&src)
            .arg(format!("-I{}", dir.path().display()));
        for flag in linker_flags(&exercise.subject) {
            gcc.arg(flag);
        }

        let compile_out = gcc.output().expect("lancement gcc");
        if !compile_out.status.success() {
            let stderr = String::from_utf8_lossy(&compile_out.stderr);
            failures.push(format!(
                "[{}] ERREUR DE COMPILATION\n  {}",
                exercise.id,
                stderr.trim()
            ));
            continue;
        }

        // Exécution + comparaison stdout
        let run_out = Command::new(&bin)
            .current_dir(dir.path())
            .output()
            .expect("exécution du binaire");

        let actual = normalize(&String::from_utf8_lossy(&run_out.stdout));
        let exp = normalize(expected);

        if actual != exp {
            failures.push(format!(
                "[{}] OUTPUT INCORRECT\n  attendu : {:?}\n  obtenu  : {:?}",
                exercise.id, exp, actual
            ));
        }
    }

    if skipped > 0 {
        println!("  ({skipped} exercice(s) ignoré(s) : test-mode, REGEX ou sans expected_output)");
    }

    if !failures.is_empty() {
        panic!(
            "\n{} exercice(s) échoué(s) :\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    println!(
        "  {} exercice(s) validé(s) avec succès.",
        exercises.len() - skipped
    );
}
