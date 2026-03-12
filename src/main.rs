//! CLI entry point — parses subcommands and dispatches to clings modules.

mod authoring;
mod chapters;
pub mod config;
pub mod constants;
mod display;
mod error;
mod exam;
mod exercises;
mod mastery;
mod models;
mod piscine;
mod progress;
mod runner;
mod tmux;
mod watcher;

use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::constants::{
    CONSECUTIVE_FAILURE_THRESHOLD, CTRL_C, CTRL_Z, MSG_EXERCISE_SOLVED_ADVANCING,
    MSG_PRESS_KEY_RETURN, SECS_PER_DAY, SUCCESS_PAUSE_SECS,
};
use crate::error::{KfError, Result};
use crate::watcher::WatchAction;

#[derive(Parser)]
#[command(
    name = "clings",
    version,
    propagate_version = true,
    about = "clings — C Systems Programming Trainer",
    long_about = "clings — Entraîneur de programmation système C (NSY103/UTC502)\n\nSans sous-commande, démarre le mode watch (progression SRS par défaut).\n\nVariables d'environnement :\n  CLINGS_EXERCISES  chemin vers le répertoire des exercices"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Mode watch : progression SRS par chapitre
    Watch {
        /// Restreindre à un seul chapitre (1-16)
        #[arg(long, short = 'c')]
        chapter: Option<u8>,
    },
    /// Lister tous les exercices (filtrable par sujet)
    List {
        #[arg(long)]
        subject: Option<String>,
        /// Afficher uniquement les exercices dont la révision SRS est due
        #[arg(long)]
        due: bool,
    },
    /// Lancer un exercice par identifiant
    Run {
        /// Exercise ID (e.g. "ptr-deref-01")
        exercise_id: String,
    },
    /// Afficher un résumé de la progression
    Progress {
        /// Détail par exercice pour un sujet donné (ex: pointers)
        #[arg(long, short = 's')]
        subject: Option<String>,
    },
    /// Afficher les indices d'un exercice
    Hint {
        /// Exercise ID
        exercise_id: String,
    },
    /// Afficher la solution d'un exercice (nécessite au moins 1 tentative)
    Solution {
        /// Exercise ID
        exercise_id: String,
    },
    /// Réinitialiser la progression (tout ou un seul sujet)
    Reset {
        /// Réinitialiser uniquement ce sujet (ex: pointers)
        #[arg(long, short = 's')]
        subject: Option<String>,
    },
    /// Mode piscine: intensive linear progression (all exercises unlocked)
    Piscine {
        /// Restreindre à un seul chapitre (1-16)
        #[arg(long, short = 'c')]
        chapter: Option<u8>,
        /// Durée limite en minutes (mode exam simulé, ex: 150 pour 2h30)
        #[arg(long, short = 't')]
        timed: Option<u64>,
    },
    /// Réviser les sujets dus selon le calendrier SRS
    Review,
    /// Afficher les statistiques globales
    Stats {
        /// Affichage détaillé : sparkline d'activité + breakdown par sujet
        #[arg(long, short = 'd')]
        detailed: bool,
    },
    /// Afficher les annales NSY103 et leur correspondance avec les exercices
    Annales,
    /// Mode exam simulé : reproduit une annale NSY103/UTC502 avec timer
    Exam {
        /// ID de session (ex: nsy103-s1-2022-2023). Laisser vide pour lister.
        #[arg(long, short = 's')]
        session: Option<String>,
        /// Lister les sessions disponibles
        #[arg(long, short = 'l')]
        list: bool,
    },
    /// Exporter la progression en JSON
    Export {
        /// Fichier de sortie (défaut : stdout)
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Importer une progression JSON exportée
    Import {
        /// Fichier JSON à importer
        input: PathBuf,
        /// Écraser avec les valeurs importées (défaut : fusion max)
        #[arg(long)]
        overwrite: bool,
    },
    /// Modifier la configuration utilisateur (~/.clings/clings.toml)
    Config {
        /// Clé au format section.champ (ex: srs.decay_days)
        key: String,
        /// Nouvelle valeur
        value: String,
    },
    /// Générer un squelette d'exercice ou valider un fichier JSON existant
    New {
        /// Sujet de l'exercice (ex: pointers, signals)
        #[arg(long, short = 's')]
        subject: Option<String>,
        /// Niveau de difficulté 1–5
        #[arg(long, short = 'd', default_value = "1")]
        difficulty: u8,
        /// Mode de validation : output, test, both
        #[arg(long, short = 'm', default_value = "output")]
        mode: String,
        /// Fichier de sortie (défaut : ./exercises/<subject>/<id>.json)
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        /// Valider uniquement un fichier JSON existant sans en créer un nouveau
        #[arg(long, short = 'v')]
        validate_only: Option<PathBuf>,
    },
}

fn main() {
    config::init(config::load());

    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Watch { chapter }) => cmd_watch(chapter),
        Some(Commands::List { subject, due }) => cmd_list(subject.as_deref(), due),
        Some(Commands::Run { exercise_id }) => cmd_run(&exercise_id),
        Some(Commands::Progress { subject }) => cmd_progress(subject.as_deref()),
        Some(Commands::Hint { exercise_id }) => cmd_hint(&exercise_id),
        Some(Commands::Solution { exercise_id }) => cmd_solution(&exercise_id),
        Some(Commands::Reset { subject }) => cmd_reset(subject.as_deref()),
        Some(Commands::Piscine { chapter, timed }) => piscine::cmd_piscine(chapter, timed),
        Some(Commands::Review) => cmd_review(),
        Some(Commands::Stats { detailed }) => cmd_stats(detailed),
        Some(Commands::Annales) => cmd_annales(),
        Some(Commands::Exam { session, list }) => exam::cmd_exam(session.as_deref(), list),
        Some(Commands::Export { output }) => cmd_export(output.as_deref()),
        Some(Commands::Import { input, overwrite }) => cmd_import(&input, overwrite),
        Some(Commands::Config { key, value }) => cmd_config(&key, &value),
        Some(Commands::New {
            subject,
            difficulty,
            mode,
            output,
            validate_only,
        }) => cmd_new(
            subject.as_deref(),
            difficulty,
            &mode,
            output.as_deref(),
            validate_only.as_deref(),
        ),
        None => cmd_watch(None),
    };

    if let Err(e) = result {
        eprintln!("{} {e}", "Erreur:".bold().red());
        std::process::exit(1);
    }
}

fn cmd_watch(filter_chapter: Option<u8>) -> Result<()> {
    install_ctrlc_handler();

    let (all_exercises, _) = exercises::load_all_exercises()?;
    let mut conn = progress::open_db()?;

    progress::apply_all_decay(&mut conn)?;

    progress::ensure_subjects_batch(&mut conn, &all_exercises)?;

    let subjects = progress::get_all_subjects(&conn)?;

    // Filter out exercises above unlocked difficulty for each subject
    let subject_map: std::collections::HashMap<&str, i32> = subjects
        .iter()
        .map(|s| (s.name.as_str(), s.difficulty_unlocked))
        .collect();
    let mastery_map: std::collections::HashMap<&str, f64> = subjects
        .iter()
        .map(|s| (s.name.as_str(), s.mastery_score.get()))
        .collect();
    let gated_exercises: Vec<crate::models::Exercise> = all_exercises
        .iter()
        .filter(|ex| {
            let unlocked = subject_map.get(ex.subject.as_str()).copied().unwrap_or(1);
            (ex.difficulty as i32) <= unlocked
        })
        .cloned()
        .collect();

    let mut chapter_blocks = chapters::order_by_chapters(&gated_exercises, &subjects);
    if !chapters::filter_by_chapter(&mut chapter_blocks, filter_chapter) {
        println!(
            "  {} Chapitre {} introuvable ou aucun exercice disponible.",
            "⚠".yellow(),
            filter_chapter.unwrap_or(0)
        );
        return Ok(());
    }
    let exercise_order = chapters::flatten_chapters(&chapter_blocks);

    if exercise_order.is_empty() {
        println!("{}", "  Aucun exercice disponible.".dimmed());
        return Ok(());
    }

    let total = exercise_order.len();
    let mut completed = vec![false; total];
    let mut editor_pane: Option<String> = None;

    // Enable raw mode for keyboard input if possible
    let _raw_guard = enable_raw_mode();

    // Pre-compute next_review_days from already-loaded subjects to avoid N+1 DB queries.
    let now_ts = chrono::Utc::now().timestamp();
    let review_map: std::collections::HashMap<&str, Option<i64>> = subjects
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.next_review_at.map(|ts| (ts - now_ts) / SECS_PER_DAY),
            )
        })
        .collect();

    // Pre-build id→exercise map for O(1) prerequisite lookups inside the watch loop.
    let exercise_by_id: std::collections::HashMap<&str, &crate::models::Exercise> =
        all_exercises.iter().map(|e| (e.id.as_str(), e)).collect();

    let mut index = 0;
    while index < total {
        let exercise = exercise_order[index];

        // Select starter code stage based on subject mastery
        let (source_path, current_stage) = runner::prepare_exercise_source(&conn, exercise)?;

        // Display exercise
        let ch_ctx = chapters::chapter_context_at(&chapter_blocks, index);
        let next_review = review_map.get(exercise.subject.as_str()).copied().flatten();
        let unmet_prereqs: Vec<String> = exercise
            .prerequisites
            .iter()
            .filter_map(|pid| {
                let subj = exercise_by_id.get(pid.as_str())?.subject.as_str();
                let mastery = *mastery_map.get(subj).unwrap_or(&0.0);
                (mastery < 2.0).then(|| pid.clone())
            })
            .collect();
        let watch_meta = display::WatchMeta {
            stage: current_stage,
            next_review_days: next_review,
            unmet_prereqs,
        };
        display::show_exercise_watch(
            exercise,
            index,
            total,
            &completed,
            Some(&ch_ctx),
            &watch_meta,
        );
        display::show_watching(&source_path);
        display::show_keybinds_with_vis(!exercise.visualizer.steps.is_empty(), false, true);

        // Open/update neovim pane in tmux
        editor_pane = tmux::update_editor_pane(editor_pane.as_deref(), &source_path);

        let source_for_change = source_path.clone();
        let mut hint_shown = false;
        let mut vis_active = false;
        let mut vis_step: usize = 0;
        let mut vis_lines: usize = 0;
        let mut escape_buf: Vec<u8> = Vec::new();
        let mut already_recorded = false;
        let mut consecutive_failures: u32 = 0;

        let action = watcher::watch_file_interactive(
            &source_path,
            // On file change: notify only, no auto-compile
            || {
                display::show_file_saved();
                display::show_keybinds_with_vis(!exercise.visualizer.steps.is_empty(), false, true);
                WatchAction::Continue
            },
            // On keyboard input
            |key| {
                // Accumulate escape sequences for arrow keys (3-byte: ESC [ C/D)
                if display::handle_esc_sequence(
                    key,
                    &mut escape_buf,
                    vis_active,
                    &mut vis_step,
                    &mut vis_lines,
                    exercise.visualizer.steps.len(),
                    &mut |step| display::show_visualizer(exercise, step),
                )
                .is_some()
                {
                    return None;
                }

                // Any non-arrow key closes the visualizer
                if vis_active {
                    vis_active = false;
                    display::show_exercise_watch(
                        exercise,
                        index,
                        total,
                        &completed,
                        None,
                        &watch_meta,
                    );
                    display::show_keybinds_with_vis(
                        !exercise.visualizer.steps.is_empty(),
                        false,
                        true,
                    );
                    return None;
                }

                match key {
                    b'v' | b'V' => {
                        if !exercise.visualizer.steps.is_empty() {
                            vis_step = 0;
                            vis_active = true;
                            vis_lines = display::show_visualizer(exercise, vis_step);
                        }
                        None
                    }
                    b'h' | b'H' => {
                        if !hint_shown {
                            println!();
                            display::show_hints(exercise);
                            hint_shown = true;
                        }
                        None
                    }
                    b'n' | b'N' => Some(WatchAction::Skip),
                    b'j' | b'J' => Some(WatchAction::Next),
                    b'k' | b'K' => Some(WatchAction::Prev),
                    b'q' | b'Q' | CTRL_C | CTRL_Z => Some(WatchAction::Quit),
                    b'l' | b'L' => {
                        // Quick exercise list — reuse subjects already loaded at session start
                        display::show_exercise_list(&all_exercises, &subjects, None, None);
                        println!("  {}", MSG_PRESS_KEY_RETURN.dimmed());
                        None
                    }
                    b'r' | b'R' => {
                        // Explicit run: compile and check now
                        let result = runner::compile_and_run(&source_for_change, exercise);
                        display::show_result(&result, exercise);
                        if result.success {
                            consecutive_failures = 0;
                            if !already_recorded {
                                already_recorded = true;
                                record_and_show(&conn, &exercise.subject, &exercise.id, true);
                            }
                            println!("  {}", MSG_EXERCISE_SOLVED_ADVANCING.bold().green());
                            std::thread::sleep(std::time::Duration::from_secs(SUCCESS_PAUSE_SECS));
                            return Some(WatchAction::Advance);
                        }
                        consecutive_failures += 1;
                        if consecutive_failures >= CONSECUTIVE_FAILURE_THRESHOLD as u32
                            && !exercise.hints.is_empty()
                        {
                            println!();
                            println!(
                                "  {}",
                                "Indice automatique après 3 tentatives :".dimmed().yellow()
                            );
                            display::show_hints(exercise);
                        }
                        display::show_keybinds_with_vis(
                            !exercise.visualizer.steps.is_empty(),
                            false,
                            true,
                        );
                        None
                    }
                    _ => None,
                }
            },
        )?;

        match action {
            WatchAction::Advance => {
                completed[index] = true;
                index += 1;
            }
            WatchAction::Skip | WatchAction::Next => {
                if index + 1 < total {
                    index += 1;
                }
            }
            WatchAction::Prev => {
                index = index.saturating_sub(1);
            }
            WatchAction::Quit => {
                break;
            }
            WatchAction::Continue => {}
        }
    }

    // Cleanup (raw mode restored automatically by _raw_guard drop)
    drop(_raw_guard);
    if let Some(pane) = &editor_pane {
        tmux::kill_pane(pane);
    }

    let done = completed.iter().filter(|&&c| c).count();
    if done == total {
        println!(
            "\n  {} Tous les exercices complétés ! Lancez `clings progress` pour voir vos stats.",
            "Félicitations !".bold().green()
        );
    } else {
        println!(
            "\n  {} {}/{} exercices complétés. Lancez `clings watch` pour continuer.",
            "Session terminée.".bold(),
            done,
            total
        );
    }

    Ok(())
}

/// RAII guard that restores libc raw mode on drop.
struct RawModeGuard;

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        disable_raw_mode();
    }
}

/// Enable terminal raw mode for single-key input.
/// Returns a guard that restores the terminal on drop.
pub(crate) fn enable_raw_mode() -> Option<RawModeGuard> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = std::io::stdin().as_raw_fd();
        // SAFETY: fd est le stdin du processus (as_raw_fd() sur un stdin valide).
        // libc::termios est un type POD, zeroed() est une valeur initiale valide.
        // tcgetattr/tcsetattr sont thread-safe pour le terminal de contrôle du processus courant.
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut termios) == 0 {
                let original = termios;
                // SAFETY: lock poison is acceptable — Option<termios> is always
                // structurally valid regardless of which thread last held the lock.
                ORIGINAL_TERMIOS
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .replace(original);
                termios.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
                termios.c_cc[libc::VMIN] = 1;
                termios.c_cc[libc::VTIME] = 0;
                if libc::tcsetattr(fd, libc::TCSANOW, &termios) == 0 {
                    return Some(RawModeGuard);
                }
            }
        }
        None
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn disable_raw_mode() {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = std::io::stdin().as_raw_fd();
        // SAFETY: original est une valeur tcgetattr valide conservée depuis enable_raw_mode.
        // fd est le même stdin que lors de l'appel à tcgetattr.
        // SAFETY: lock poison is acceptable — Option<termios> is always
        // structurally valid regardless of which thread last held the lock.
        if let Some(original) = ORIGINAL_TERMIOS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            unsafe {
                libc::tcsetattr(fd, libc::TCSANOW, &original);
            }
        }
    }
}

#[cfg(unix)]
static ORIGINAL_TERMIOS: std::sync::Mutex<Option<libc::termios>> = std::sync::Mutex::new(None);

/// Install Ctrl+C handler to restore terminal and clean up tmux panes.
pub(crate) fn install_ctrlc_handler() {
    if let Err(e) = ctrlc::set_handler(move || {
        disable_raw_mode();
        println!();
        // process::exit bypasses Drop of all RAII guards (RawModeGuard, tmux panes).
        // This is intentional: the handler runs asynchronously and cannot safely
        // unwind the call stack. Terminal cleanup is done explicitly above.
        std::process::exit(0);
    }) {
        eprintln!("Avertissement : échec de l'installation du gestionnaire Ctrl-C : {e}");
    }
}

/// Record a practice attempt and display the mastery update.
/// On failure, only logs the attempt (no mastery display).
pub(crate) fn record_and_show(
    conn: &rusqlite::Connection,
    subject: &str,
    exercise_id: &str,
    success: bool,
) {
    if success {
        match progress::record_attempt(conn, subject, exercise_id, true) {
            Ok(sub) => display::show_mastery_update(&sub, true),
            Err(e) => eprintln!("  {} {e}", "Erreur BD :".red()),
        }
    } else if let Err(e) = progress::record_attempt(conn, subject, exercise_id, false) {
        eprintln!("  {} {e}", "Erreur BD :".red());
    }
}

fn cmd_list(filter_subject: Option<&str>, filter_due: bool) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let conn = progress::open_db()?;
    let subjects = progress::get_all_subjects(&conn)?;

    let due_subjects: Option<Vec<String>> = if filter_due {
        Some(progress::get_due_subjects(&conn)?)
    } else {
        None
    };

    display::show_exercise_list(
        &all_exercises,
        &subjects,
        filter_subject,
        due_subjects.as_deref(),
    );
    Ok(())
}

fn cmd_run(exercise_id: &str) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| KfError::ExerciseNotFound(exercise_id.to_string()))?;

    display::show_exercise(exercise, 0, 1);

    let conn = progress::open_db()?;
    let subject_mastery =
        progress::get_subject(&conn, &exercise.subject)?.map(|s| s.mastery_score.get());
    let source_path = runner::write_starter_code(exercise, subject_mastery)?;

    display::show_edit_instructions(&source_path);
    display::show_keybinds_with_vis(!exercise.visualizer.steps.is_empty(), false, false);
    let exercise = exercise.clone();
    let source_for_change = source_path.clone();

    let mut vis_active = false;
    let mut vis_step: usize = 0;
    let mut vis_lines: usize = 0;
    let mut escape_buf: Vec<u8> = Vec::new();

    let action = watcher::watch_file_interactive(
        &source_path,
        || {
            let result = runner::compile_and_run(&source_for_change, &exercise);
            display::show_result(&result, &exercise);

            if result.success {
                record_and_show(&conn, &exercise.subject, &exercise.id, true);
                println!("  {}", "Exercice résolu !".bold().green());
                return WatchAction::Advance;
            }

            if !result.compile_error {
                record_and_show(&conn, &exercise.subject, &exercise.id, false);
            }

            println!("  {}", "En attente de la prochaine sauvegarde...".dimmed());
            WatchAction::Continue
        },
        |key| {
            if display::handle_esc_sequence(
                key,
                &mut escape_buf,
                vis_active,
                &mut vis_step,
                &mut vis_lines,
                exercise.visualizer.steps.len(),
                &mut |step| display::show_visualizer(&exercise, step),
            )
            .is_some()
            {
                return None;
            }

            if vis_active {
                vis_active = false;
                display::show_exercise(&exercise, 0, 1);
                display::show_keybinds_with_vis(
                    !exercise.visualizer.steps.is_empty(),
                    false,
                    false,
                );
                return None;
            }

            match key {
                b'v' | b'V' if !exercise.visualizer.steps.is_empty() => {
                    vis_step = 0;
                    vis_active = true;
                    vis_lines = display::show_visualizer(&exercise, vis_step);
                    None
                }
                b'q' | b'Q' | CTRL_C | CTRL_Z => Some(WatchAction::Quit),
                _ => None,
            }
        },
    )?;

    if matches!(action, WatchAction::Advance) {
        println!("  {}", "Terminé !".bold().green());
    }

    Ok(())
}

fn cmd_progress(subject: Option<&str>) -> Result<()> {
    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;
    if let Some(s) = subject {
        let scores = progress::get_exercise_scores(&conn, s)?;
        display::show_exercise_scores(s, &scores);
    } else {
        let subjects = progress::get_all_subjects(&conn)?;
        let streak = progress::get_streak(&conn)?;
        display::show_progress(&subjects, streak);
    }
    Ok(())
}

fn cmd_hint(exercise_id: &str) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| KfError::ExerciseNotFound(exercise_id.to_string()))?;
    display::show_hints(exercise);
    Ok(())
}

fn cmd_solution(exercise_id: &str) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| KfError::ExerciseNotFound(exercise_id.to_string()))?;

    let conn = progress::open_db()?;
    let mut stmt =
        conn.prepare_cached("SELECT COUNT(*) FROM practice_log WHERE exercise_id = ?1")?;
    let count: i64 = stmt.query_row([exercise_id], |row| row.get(0))?;

    if count == 0 {
        println!(
            "  {} Vous devez tenter l'exercice au moins une fois avant de voir la solution.",
            "Verrouillé:".bold().yellow()
        );
        println!("  Lancer : clings run {exercise_id}");
        return Ok(());
    }

    display::show_solution(exercise);
    Ok(())
}

fn cmd_review() -> Result<()> {
    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;

    let due = progress::get_due_subjects(&conn)?;
    if due.is_empty() {
        println!(
            "  {}",
            "Aucun sujet à renforcer pour l'instant. Revenez plus tard !".dimmed()
        );
        return Ok(());
    }

    let (all_exercises, _) = exercises::load_all_exercises()?;

    // Build subject map: name → Subject (mastery_score + next_review_at)
    let subjects = progress::get_all_subjects(&conn)?;
    let subject_map: std::collections::HashMap<&str, &crate::models::Subject> =
        subjects.iter().map(|s| (s.name.as_str(), s)).collect();

    // Pre-build O(1) lookup maps for the review loop.
    let exercise_by_id: std::collections::HashMap<&str, &crate::models::Exercise> =
        all_exercises.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut exercise_by_subject: std::collections::HashMap<&str, &crate::models::Exercise> =
        std::collections::HashMap::new();
    for e in &all_exercises {
        exercise_by_subject.entry(e.subject.as_str()).or_insert(e);
    }

    // For each due subject, prefer the weakest exercise (by exercise_scores); fallback to first
    let weakest_by_subject = progress::get_all_weakest_exercises(&conn).unwrap_or_else(|e| {
        eprintln!("  [clings] avertissement : weakest_exercises indisponible : {e}");
        std::collections::HashMap::new()
    });
    let mut due_exercises: Vec<&crate::models::Exercise> = due
        .iter()
        .filter_map(|subject_name| {
            // Prioritise the exercise with the lowest success rate for this subject.
            if let Some(id) = weakest_by_subject.get(subject_name.as_str()) {
                if let Some(ex) = exercise_by_id.get(id.as_str()) {
                    return Some(*ex);
                }
            }
            // Fallback: first exercise belonging to this subject
            exercise_by_subject.get(subject_name.as_str()).copied()
        })
        .collect();

    // Sort by (mastery_score ASC, next_review_at ASC) — weakest and most-overdue first
    due_exercises.sort_by(|a, b| {
        let sa = subject_map.get(a.subject.as_str());
        let sb = subject_map.get(b.subject.as_str());
        let ma = sa.map_or(0.0, |s| s.mastery_score.get());
        let mb = sb.map_or(0.0, |s| s.mastery_score.get());
        ma.partial_cmp(&mb)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let ra = sa.and_then(|s| s.next_review_at).unwrap_or(i64::MAX);
                let rb = sb.and_then(|s| s.next_review_at).unwrap_or(i64::MAX);
                ra.cmp(&rb)
            })
    });

    let total = due_exercises.len();
    println!(
        "  {} {} sujet(s) à renforcer",
        "Renforcement mastery —".bold().cyan(),
        total.to_string().bold()
    );
    println!();

    for (i, exercise) in due_exercises.iter().enumerate() {
        println!(
            "  {} [{}/{}] {}",
            "Exercice".bold().cyan(),
            i + 1,
            total,
            exercise.title.bold().green()
        );
        println!("  {} {}", "Sujet:".dimmed(), exercise.subject.dimmed());
        println!();
        match cmd_run(&exercise.id) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("  {} {e}", "Erreur:".bold().red());
            }
        }
        println!();
    }

    println!(
        "  {} Session de renforcement terminée ({} exercices).",
        "✓".bold().green(),
        total
    );

    Ok(())
}

fn cmd_stats(detailed: bool) -> Result<()> {
    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;
    let subjects = progress::get_all_subjects(&conn)?;
    let streak = progress::get_streak(&conn)?;
    if detailed {
        let attempts = progress::get_subject_attempts(&conn)?;
        let daily = progress::get_daily_activity(&conn, 30)?;
        display::show_stats_detailed(&subjects, streak as u32, &attempts, &daily);
    } else {
        display::show_stats(&subjects, streak as u32);
    }
    Ok(())
}

fn cmd_annales() -> Result<()> {
    let exercises_dir = exercises::resolve_exercises_dir()?;
    let map_path = exercises_dir.join("annales_map.json");
    let raw = std::fs::read_to_string(&map_path)?;
    let annales: Vec<display::AnnaleSession> = serde_json::from_str(&raw)
        .map_err(|e| KfError::Config(format!("annales_map.json: {e}")))?;
    let (all_exercises, _) = exercises::load_all_exercises()?;
    display::show_annales(&annales, &all_exercises);
    Ok(())
}

fn cmd_export(output: Option<&std::path::Path>) -> Result<()> {
    let conn = progress::open_db()?;
    let subjects = progress::get_all_subjects(&conn)?;
    let count = subjects.len();
    let json = progress::export_progress(&conn)?;
    match output {
        Some(path) => {
            std::fs::write(path, &json)?;
            display::show_export_done(Some(path), count);
        }
        None => {
            print!("{json}");
            display::show_export_done(None, count);
        }
    }
    Ok(())
}

fn cmd_import(input: &std::path::Path, overwrite: bool) -> Result<()> {
    let json = std::fs::read_to_string(input)?;
    let mut conn = progress::open_db()?;
    let (count, warnings) = progress::import_progress(&mut conn, &json, overwrite)?;
    for w in &warnings {
        eprintln!("  {} {}", "⚠".yellow(), w);
    }
    display::show_import_done(count, overwrite);
    Ok(())
}

fn cmd_reset(subject: Option<&str>) -> Result<()> {
    if let Some(name) = subject {
        print!(
            "  {} Supprimer la progression de '{}'. Taper 'yes' pour confirmer : ",
            "Attention !".bold().red(),
            name
        );
        let _ = io::stdout().flush();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim() == "yes" {
            let conn = progress::open_db()?;
            progress::reset_subject(&conn, name)?;
            println!("  {} Progression de '{}' réinitialisée.", "✓".green(), name);
        } else {
            println!("  {}", "Annulé.".dimmed());
        }
    } else {
        print!(
            "  {} Ceci supprimera TOUTE la progression. Tapez 'yes' pour confirmer : ",
            "Attention !".bold().red()
        );
        let _ = io::stdout().flush();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim() == "yes" {
            let conn = progress::open_db()?;
            progress::reset_progress(&conn)?;
            println!("  {}", "Progression réinitialisée.".green());
        } else {
            println!("  {}", "Annulé.".dimmed());
        }
    }
    Ok(())
}

fn cmd_config(key: &str, value: &str) -> Result<()> {
    let (section, field) = key.split_once('.').ok_or_else(|| {
        KfError::Config(format!(
            "Format de clé invalide : '{key}' — attendu 'section.champ' (ex: srs.decay_days)"
        ))
    })?;
    config::set_value(section, field, value).map_err(KfError::Config)?;
    println!(
        "  {} {key} = {value}",
        "Config mise à jour :".bold().green()
    );
    Ok(())
}

fn cmd_new(
    subject: Option<&str>,
    difficulty: u8,
    mode: &str,
    output: Option<&std::path::Path>,
    validate_only: Option<&std::path::Path>,
) -> Result<()> {
    // ── Mode --validate-only ──────────────────────────────────────────────
    if let Some(path) = validate_only {
        let errors = authoring::validate_exercise(path);
        display::show_authoring_result(path, &errors);
        if !errors.is_empty() {
            return Err(KfError::Config("validation échouée".to_string()));
        }
        return Ok(());
    }

    // ── Mode génération ───────────────────────────────────────────────────
    let subject = subject.ok_or_else(|| {
        KfError::Config(
            "--subject requis pour générer un squelette (ex: --subject pointers)".to_string(),
        )
    })?;

    let exercise = authoring::generate_skeleton(subject, difficulty, mode)?;

    // Determine output path
    let target = if let Some(p) = output {
        p.to_path_buf()
    } else {
        let exercises_dir = exercises::resolve_exercises_dir()?;
        let dir = exercises_dir.join(subject);
        std::fs::create_dir_all(&dir)?;
        dir.join(format!("{}.json", exercise.id))
    };

    let json = serde_json::to_string_pretty(&exercise)
        .map_err(|e| KfError::Config(format!("sérialisation JSON : {e}")))?;
    std::fs::write(&target, &json)?;

    println!();
    println!(
        "  {} {}",
        "Squelette généré :".bold().green(),
        target.display()
    );
    println!();
    println!("  Champs à remplir :");
    for placeholder in &[
        "__TITLE__",
        "__DESCRIPTION__",
        "__STARTER_CODE__",
        "__SOLUTION_CODE__",
    ] {
        if json.contains(placeholder) {
            println!("    {} {}", "•".dimmed(), placeholder.yellow());
        }
    }
    if json.contains("__EXPECTED_OUTPUT__") {
        println!("    {} {}", "•".dimmed(), "__EXPECTED_OUTPUT__".yellow());
    }
    println!();
    println!(
        "  Valider ensuite avec : {}",
        format!("clings new --validate-only {}", target.display()).bold()
    );
    println!();
    Ok(())
}
