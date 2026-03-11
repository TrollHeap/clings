mod chapters;
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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::constants::{CONSECUTIVE_FAILURE_THRESHOLD, SUCCESS_PAUSE_SECS};
use crate::error::{KfError, Result};
use crate::models::ValidationMode;
use crate::watcher::WatchAction;

#[derive(Parser)]
#[command(
    name = "clings",
    version,
    propagate_version = true,
    about = "clings — C Systems Programming Trainer"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start watch mode: SRS-prioritized exercises with auto-advance
    Watch {
        /// Restreindre à un seul chapitre (1-16)
        #[arg(long, short = 'c')]
        chapter: Option<u8>,
    },
    /// List all exercises (optionally filtered by subject)
    List {
        #[arg(long)]
        subject: Option<String>,
    },
    /// Run a specific exercise by ID
    Run {
        /// Exercise ID (e.g. "ptr-deref-01")
        exercise_id: String,
    },
    /// Show progress overview
    Progress,
    /// Show hints for an exercise
    Hint {
        /// Exercise ID
        exercise_id: String,
    },
    /// Show solution for an exercise (requires >= 1 attempt)
    Solution {
        /// Exercise ID
        exercise_id: String,
    },
    /// Reset all progress (with confirmation)
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
    /// Reinforce due subjects via SRS scheduling
    Review,
    /// Show global statistics
    Stats,
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
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Watch { chapter }) => cmd_watch(chapter),
        Some(Commands::List { subject }) => cmd_list(subject.as_deref()),
        Some(Commands::Run { exercise_id }) => cmd_run(&exercise_id),
        Some(Commands::Progress) => cmd_progress(),
        Some(Commands::Hint { exercise_id }) => cmd_hint(&exercise_id),
        Some(Commands::Solution { exercise_id }) => cmd_solution(&exercise_id),
        Some(Commands::Reset { subject }) => cmd_reset(subject.as_deref()),
        Some(Commands::Piscine { chapter, timed }) => piscine::cmd_piscine(chapter, timed),
        Some(Commands::Review) => cmd_review(),
        Some(Commands::Stats) => cmd_stats(),
        Some(Commands::Annales) => cmd_annales(),
        Some(Commands::Exam { session, list }) => exam::cmd_exam(session.as_deref(), list),
        Some(Commands::Export { output }) => cmd_export(output.as_deref()),
        Some(Commands::Import { input, overwrite }) => cmd_import(&input, overwrite),
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
    let gated_exercises: Vec<crate::models::Exercise> = all_exercises
        .into_iter()
        .filter(|ex| {
            let unlocked = subject_map.get(ex.subject.as_str()).copied().unwrap_or(1);
            (ex.difficulty as i32) <= unlocked
        })
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

    let mut index = 0;
    while index < total {
        let exercise = exercise_order[index];

        // Skip test-only exercises (not yet supported in CLI)
        if matches!(exercise.validation.mode, ValidationMode::Test) {
            println!(
                "  {} Exercice {} ignoré (validation Test non supportée en CLI)",
                "⚠".yellow(),
                exercise.id
            );
            completed[index] = true;
            index += 1;
            continue;
        }

        // Select starter code stage based on subject mastery
        let (source_path, current_stage) = runner::prepare_exercise_source(&conn, exercise)?;

        // Display exercise
        let ch_ctx = chapters::chapter_context_at(&chapter_blocks, index);
        display::show_exercise_watch(
            exercise,
            index,
            total,
            &completed,
            Some(&ch_ctx),
            current_stage,
        );
        display::show_watching(&source_path);
        display::show_keybinds_with_vis(!exercise.visualizer.steps.is_empty());

        // Open/update neovim pane in tmux
        editor_pane = tmux::update_editor_pane(editor_pane.as_deref(), &source_path);

        let exercise_clone = exercise.clone();
        let conn_for_watch = progress::open_db()?;
        let source_for_change = source_path.clone();
        let mut hint_shown = false;
        let mut vis_active = false;
        let mut vis_step: usize = 0;
        let mut vis_lines: usize = 0;
        let mut escape_buf: Vec<u8> = Vec::new();
        let already_recorded = Arc::new(AtomicBool::new(false));
        let mut consecutive_failures: u32 = 0;

        let action = watcher::watch_file_interactive(
            &source_path,
            // On file change: notify only, no auto-compile
            || {
                display::show_file_saved();
                display::show_keybinds_with_vis(!exercise_clone.visualizer.steps.is_empty());
                WatchAction::Continue
            },
            // On keyboard input
            |key| {
                // Accumulate escape sequences for arrow keys (3-byte: ESC [ C/D)
                if !escape_buf.is_empty() {
                    escape_buf.push(key);
                    if escape_buf.len() == 3 {
                        let seq = std::mem::take(&mut escape_buf);
                        if vis_active {
                            let n = exercise_clone.visualizer.steps.len();
                            match seq.as_slice() {
                                [0x1b, b'[', b'C'] => {
                                    // Arrow right → next step
                                    vis_step = (vis_step + 1).min(n.saturating_sub(1));
                                    print!("\x1b[{}A\x1b[J", vis_lines);
                                    io::stdout().flush().ok();
                                    vis_lines = display::show_visualizer(&exercise_clone, vis_step);
                                }
                                [0x1b, b'[', b'D'] => {
                                    // Arrow left → previous step
                                    vis_step = vis_step.saturating_sub(1);
                                    print!("\x1b[{}A\x1b[J", vis_lines);
                                    io::stdout().flush().ok();
                                    vis_lines = display::show_visualizer(&exercise_clone, vis_step);
                                }
                                _ => {}
                            }
                        }
                    }
                    return None;
                }
                if key == 0x1b {
                    escape_buf.push(key);
                    return None;
                }

                // Any non-arrow key closes the visualizer
                if vis_active {
                    vis_active = false;
                    display::show_exercise_watch(
                        &exercise_clone,
                        index,
                        total,
                        &completed,
                        None,
                        current_stage,
                    );
                    display::show_keybinds_with_vis(!exercise_clone.visualizer.steps.is_empty());
                    return None;
                }

                match key {
                    b'v' | b'V' => {
                        if !exercise_clone.visualizer.steps.is_empty() {
                            vis_step = 0;
                            vis_active = true;
                            vis_lines = display::show_visualizer(&exercise_clone, vis_step);
                        }
                        None
                    }
                    b'h' | b'H' => {
                        if !hint_shown {
                            println!();
                            display::show_hints(&exercise_clone);
                            hint_shown = true;
                        }
                        None
                    }
                    b'n' | b'N' => Some(WatchAction::Skip),
                    b'j' | b'J' => Some(WatchAction::Next),
                    b'k' | b'K' => Some(WatchAction::Prev),
                    b'q' | b'Q' | 0x03 | 0x1a => Some(WatchAction::Quit),
                    b'l' | b'L' => {
                        // Quick exercise list
                        match progress::open_db() {
                            Err(e) => eprintln!("  {} {e}", "DB Error:".red()),
                            Ok(c) => match progress::get_all_subjects(&c) {
                                Err(e) => eprintln!("  {} {e}", "DB Error:".red()),
                                Ok(subjects) => match exercises::load_all_exercises() {
                                    Err(e) => eprintln!("  {} {e}", "Erreur:".red()),
                                    Ok((all, _)) => {
                                        display::show_exercise_list(&all, &subjects, None);
                                    }
                                },
                            },
                        }
                        println!("  {}", "Press any key to return...".dimmed());
                        None
                    }
                    b'r' | b'R' => {
                        // Explicit run: compile and check now
                        let result = runner::compile_and_run(&source_for_change, &exercise_clone);
                        display::show_result(&result, &exercise_clone);
                        if result.success {
                            consecutive_failures = 0;
                            if !already_recorded.swap(true, Ordering::SeqCst) {
                                record_and_show(
                                    &conn_for_watch,
                                    &exercise_clone.subject,
                                    &exercise_clone.id,
                                    true,
                                );
                            }
                            println!(
                                "  {}",
                                "Exercice résolu ! Avancement dans 2s...".bold().green()
                            );
                            std::thread::sleep(std::time::Duration::from_secs(SUCCESS_PAUSE_SECS));
                            return Some(WatchAction::Advance);
                        }
                        consecutive_failures += 1;
                        if consecutive_failures >= CONSECUTIVE_FAILURE_THRESHOLD as u32
                            && !exercise_clone.hints.is_empty()
                        {
                            println!();
                            println!(
                                "  {}",
                                "Indice automatique après 3 tentatives :".dimmed().yellow()
                            );
                            display::show_hints(&exercise_clone);
                        }
                        display::show_keybinds_with_vis(
                            !exercise_clone.visualizer.steps.is_empty(),
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
pub(crate) struct RawModeGuard;

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
        std::process::exit(0);
    }) {
        eprintln!("Warning: failed to install Ctrl-C handler: {e}");
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
            Err(e) => eprintln!("  {} {e}", "DB Error:".red()),
        }
    } else if let Err(e) = progress::record_attempt(conn, subject, exercise_id, false) {
        eprintln!("  {} {e}", "DB Error:".red());
    }
}

fn cmd_list(filter_subject: Option<&str>) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let conn = progress::open_db()?;
    let subjects = progress::get_all_subjects(&conn)?;

    display::show_exercise_list(&all_exercises, &subjects, filter_subject);
    Ok(())
}

fn cmd_run(exercise_id: &str) -> Result<()> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| KfError::ExerciseNotFound(exercise_id.to_string()))?;

    display::show_exercise(exercise, 0, 1);

    let conn = progress::open_db()?;
    let subject_mastery = progress::get_subject(&conn, &exercise.subject)?.map(|s| s.mastery_score);
    let source_path = runner::write_starter_code(exercise, subject_mastery)?;

    display::show_edit_instructions(&source_path);
    let exercise_clone = exercise.clone();
    let source_for_change = source_path.clone();

    let action = watcher::watch_file_interactive(
        &source_path,
        || {
            let result = runner::compile_and_run(&source_for_change, &exercise_clone);
            display::show_result(&result, &exercise_clone);

            if result.success {
                record_and_show(&conn, &exercise_clone.subject, &exercise_clone.id, true);
                println!("  {}", "Exercise completed!".bold().green());
                return WatchAction::Advance;
            }

            if !result.compile_error {
                record_and_show(&conn, &exercise_clone.subject, &exercise_clone.id, false);
            }

            println!("  {}", "Waiting for next save...".dimmed());
            WatchAction::Continue
        },
        |key| {
            if matches!(key, b'q' | b'Q' | 0x03 | 0x1a) {
                Some(WatchAction::Quit)
            } else {
                None
            }
        },
    )?;

    if matches!(action, WatchAction::Advance) {
        println!("  {}", "Done!".bold().green());
    }

    Ok(())
}

fn cmd_progress() -> Result<()> {
    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;
    let subjects = progress::get_all_subjects(&conn)?;
    let streak = progress::get_streak(&conn)?;
    display::show_progress(&subjects, streak);
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
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM practice_log WHERE exercise_id = ?1")?;
    let count: i64 = stmt
        .query_row([exercise_id], |row| row.get(0))
        .unwrap_or_else(|e| {
            eprintln!("DB error checking attempts: {e}");
            0
        });

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

    // For each due subject, prefer the most-recently-failed exercise; fallback to first
    let mut due_exercises: Vec<&crate::models::Exercise> = due
        .iter()
        .filter_map(|subject_name| {
            // Try to find the last failed exercise for this subject
            let failed_id = progress::get_failed_exercise(&conn, subject_name)
                .ok()
                .flatten();
            if let Some(ref id) = failed_id {
                if let Some(ex) = all_exercises.iter().find(|e| &e.id == id) {
                    return Some(ex);
                }
            }
            // Fallback: first exercise belonging to this subject
            all_exercises.iter().find(|e| &e.subject == subject_name)
        })
        .collect();

    // Sort by (mastery_score ASC, next_review_at ASC) — weakest and most-overdue first
    due_exercises.sort_by(|a, b| {
        let sa = subject_map.get(a.subject.as_str());
        let sb = subject_map.get(b.subject.as_str());
        let ma = sa.map(|s| s.mastery_score).unwrap_or(0.0);
        let mb = sb.map(|s| s.mastery_score).unwrap_or(0.0);
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

fn cmd_stats() -> Result<()> {
    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;
    let subjects = progress::get_all_subjects(&conn)?;
    let streak = progress::get_streak(&conn)?;
    display::show_stats(&subjects, streak as u32);
    Ok(())
}

fn cmd_annales() -> Result<()> {
    let exercises_dir = exercises::resolve_exercises_dir()?;
    let map_path = exercises_dir.join("annales_map.json");
    let raw = std::fs::read_to_string(&map_path)?;
    let annales: Vec<display::AnnaleExam> = serde_json::from_str(&raw)
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
    let count = progress::import_progress(&mut conn, &json, overwrite)?;
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
        io::stdout().flush().ok();
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
        io::stdout().flush().ok();
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
