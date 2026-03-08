mod chapters;
mod display;
mod exercises;
mod mastery;
mod models;
mod progress;
mod runner;
mod tmux;
mod watcher;

use std::io::{self, Write};

use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::models::ValidationMode;
use crate::watcher::WatchAction;

#[derive(Parser)]
#[command(name = "kf", about = "KernelForge CLI — C Systems Programming Trainer")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start watch mode: SRS-prioritized exercises with auto-advance
    Watch,
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
    Reset,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Watch) => cmd_watch(),
        Some(Commands::List { subject }) => cmd_list(subject.as_deref()),
        Some(Commands::Run { exercise_id }) => cmd_run(&exercise_id),
        Some(Commands::Progress) => cmd_progress(),
        Some(Commands::Hint { exercise_id }) => cmd_hint(&exercise_id),
        Some(Commands::Solution { exercise_id }) => cmd_solution(&exercise_id),
        Some(Commands::Reset) => cmd_reset(),
        None => cmd_watch(),
    };

    if let Err(e) = result {
        eprintln!("{} {e}", "Error:".bold().red());
        std::process::exit(1);
    }
}

fn cmd_watch() -> Result<(), String> {
    install_ctrlc_handler();

    let (all_exercises, _) = exercises::load_all_exercises()?;
    let conn = progress::open_db()?;

    progress::apply_all_decay(&conn)?;

    for ex in &all_exercises {
        progress::ensure_subject(&conn, &ex.subject)?;
    }

    let subjects = progress::get_all_subjects(&conn)?;
    let chapter_blocks = chapters::order_by_chapters(&all_exercises, &subjects);
    let exercise_order = chapters::flatten_chapters(&chapter_blocks);

    if exercise_order.is_empty() {
        println!("{}", "  Aucun exercice disponible.".dimmed());
        return Ok(());
    }

    let total = exercise_order.len();
    let mut completed = vec![false; total];
    let mut editor_pane: Option<String> = None;

    // Enable raw mode for keyboard input if possible
    let raw_mode = enable_raw_mode();

    let mut index = 0;
    while index < total {
        let exercise = exercise_order[index];

        // Skip test-only exercises
        if matches!(exercise.validation.mode, ValidationMode::Test) {
            completed[index] = true;
            index += 1;
            continue;
        }

        // Select starter code stage based on subject mastery
        let subject_mastery =
            progress::get_subject(&conn, &exercise.subject)?.map(|s| s.mastery_score);
        let current_stage = subject_mastery.map(runner::mastery_to_stage);
        let source_path = runner::write_starter_code(exercise, subject_mastery)
            .map_err(|e| format!("Failed to write starter code: {e}"))?;

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
        display::show_keybinds();

        // Open/update neovim pane in tmux
        editor_pane = tmux::update_editor_pane(editor_pane.as_deref(), &source_path);

        let exercise_clone = exercise.clone();
        let conn_for_watch = progress::open_db()?;
        let source_for_change = source_path.clone();
        let mut hint_shown = false;

        let action = watcher::watch_file_interactive(
            &source_path,
            // On file change
            || {
                let result = runner::compile_and_run(&source_for_change, &exercise_clone);

                // Redraw screen with result
                display::show_exercise_watch(
                    &exercise_clone,
                    index,
                    total,
                    &completed,
                    None,
                    current_stage,
                );
                display::show_result(&result, &exercise_clone);

                if result.success {
                    match progress::record_attempt(
                        &conn_for_watch,
                        &exercise_clone.subject,
                        &exercise_clone.id,
                        true,
                    ) {
                        Ok(sub) => display::show_mastery_update(&sub, true),
                        Err(e) => eprintln!("  {} {e}", "DB Error:".red()),
                    }
                    println!(
                        "  {}",
                        "Exercice résolu ! Avancement dans 2s...".bold().green()
                    );
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    return WatchAction::Advance;
                }

                if !result.compile_error {
                    if let Err(e) = progress::record_attempt(
                        &conn_for_watch,
                        &exercise_clone.subject,
                        &exercise_clone.id,
                        false,
                    ) {
                        eprintln!("  {} {e}", "DB Error:".red());
                    }
                }

                display::show_keybinds();
                WatchAction::Continue
            },
            // On keyboard input
            |key| match key {
                b'h' | b'H' => {
                    if !hint_shown {
                        println!();
                        display::show_hints(&exercise_clone);
                        hint_shown = true;
                    }
                    None
                }
                b'n' | b'N' => Some(WatchAction::Skip),
                b'q' | b'Q' => Some(WatchAction::Quit),
                b'l' | b'L' => {
                    // Quick exercise list
                    let conn = progress::open_db().ok();
                    if let Some(c) = &conn {
                        if let Ok(subjects) = progress::get_all_subjects(c) {
                            let (all, _) = exercises::load_all_exercises().unwrap_or_default();
                            display::show_exercise_list(&all, &subjects, None);
                        }
                    }
                    println!("  {}", "Press any key to return...".dimmed());
                    None
                }
                b'c' | b'C' => {
                    // Manual check: compile and run now
                    let result = runner::compile_and_run(&source_for_change, &exercise_clone);
                    display::show_result(&result, &exercise_clone);
                    if result.success {
                        match progress::record_attempt(
                            &conn_for_watch,
                            &exercise_clone.subject,
                            &exercise_clone.id,
                            true,
                        ) {
                            Ok(sub) => display::show_mastery_update(&sub, true),
                            Err(e) => eprintln!("  {} {e}", "DB Error:".red()),
                        }
                        println!("  {}", "Exercise solved! Advancing...".bold().green());
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        return Some(WatchAction::Advance);
                    }
                    display::show_keybinds();
                    None
                }
                _ => None,
            },
        )?;

        match action {
            WatchAction::Advance => {
                completed[index] = true;
                index += 1;
            }
            WatchAction::Skip => {
                index += 1;
            }
            WatchAction::Quit => {
                break;
            }
            WatchAction::Continue => {}
        }
    }

    // Cleanup
    if raw_mode {
        disable_raw_mode();
    }
    if let Some(pane) = &editor_pane {
        tmux::kill_pane(pane);
    }

    let done = completed.iter().filter(|&&c| c).count();
    if done == total {
        println!(
            "\n  {} Tous les exercices complétés ! Lancez `kf progress` pour voir vos stats.",
            "Félicitations !".bold().green()
        );
    } else {
        println!(
            "\n  {} {}/{} exercices complétés. Lancez `kf watch` pour continuer.",
            "Session terminée.".bold(),
            done,
            total
        );
    }

    Ok(())
}

/// Enable terminal raw mode for single-key input.
/// Returns true if raw mode was enabled.
fn enable_raw_mode() -> bool {
    // Use termios to set raw mode on stdin
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = std::io::stdin().as_raw_fd();
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut termios) == 0 {
                let original = termios;
                // Store original for restore
                ORIGINAL_TERMIOS.lock().unwrap().replace(original);

                termios.c_lflag &= !(libc::ICANON | libc::ECHO);
                termios.c_cc[libc::VMIN] = 1;
                termios.c_cc[libc::VTIME] = 0;
                libc::tcsetattr(fd, libc::TCSANOW, &termios) == 0
            } else {
                false
            }
        }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Restore terminal to normal mode.
fn disable_raw_mode() {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = std::io::stdin().as_raw_fd();
        if let Some(original) = ORIGINAL_TERMIOS.lock().unwrap().take() {
            unsafe {
                libc::tcsetattr(fd, libc::TCSANOW, &original);
            }
        }
    }
}

#[cfg(unix)]
static ORIGINAL_TERMIOS: std::sync::Mutex<Option<libc::termios>> = std::sync::Mutex::new(None);

/// Install Ctrl+C handler to restore terminal and clean up tmux panes.
fn install_ctrlc_handler() {
    ctrlc::set_handler(move || {
        disable_raw_mode();
        // Print newline so prompt is clean
        println!();
        std::process::exit(0);
    })
    .ok();
}

fn cmd_list(filter_subject: Option<&str>) -> Result<(), String> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let conn = progress::open_db()?;
    let subjects = progress::get_all_subjects(&conn)?;

    display::show_exercise_list(&all_exercises, &subjects, filter_subject);
    Ok(())
}

fn cmd_run(exercise_id: &str) -> Result<(), String> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| format!("Exercise not found: {exercise_id}"))?;

    display::show_exercise(exercise, 0, 1);

    let conn = progress::open_db()?;
    let subject_mastery = progress::get_subject(&conn, &exercise.subject)?.map(|s| s.mastery_score);
    let source_path = runner::write_starter_code(exercise, subject_mastery)
        .map_err(|e| format!("Failed to write starter code: {e}"))?;

    display::show_edit_instructions(&source_path);
    let exercise_clone = exercise.clone();
    let source_for_change = source_path.clone();

    let action = watcher::watch_file_interactive(
        &source_path,
        || {
            let result = runner::compile_and_run(&source_for_change, &exercise_clone);
            display::show_result(&result, &exercise_clone);

            if result.success {
                match progress::record_attempt(
                    &conn,
                    &exercise_clone.subject,
                    &exercise_clone.id,
                    true,
                ) {
                    Ok(sub) => display::show_mastery_update(&sub, true),
                    Err(e) => eprintln!("  {} {e}", "DB Error:".red()),
                }
                println!("  {}", "Exercise completed!".bold().green());
                return WatchAction::Advance;
            }

            if !result.compile_error {
                let _ = progress::record_attempt(
                    &conn,
                    &exercise_clone.subject,
                    &exercise_clone.id,
                    false,
                );
            }

            println!("  {}", "Waiting for next save...".dimmed());
            WatchAction::Continue
        },
        |key| {
            if key == b'q' || key == b'Q' {
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

fn cmd_progress() -> Result<(), String> {
    let conn = progress::open_db()?;
    progress::apply_all_decay(&conn)?;
    let subjects = progress::get_all_subjects(&conn)?;
    let streak = progress::get_streak(&conn)?;
    display::show_progress(&subjects, streak);
    Ok(())
}

fn cmd_hint(exercise_id: &str) -> Result<(), String> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| format!("Exercise not found: {exercise_id}"))?;
    display::show_hints(exercise);
    Ok(())
}

fn cmd_solution(exercise_id: &str) -> Result<(), String> {
    let (all_exercises, _) = exercises::load_all_exercises()?;
    let exercise = exercises::find_exercise(&all_exercises, exercise_id)
        .ok_or_else(|| format!("Exercise not found: {exercise_id}"))?;

    let conn = progress::open_db()?;
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM practice_log WHERE exercise_id = ?1")
        .map_err(|e| format!("Query error: {e}"))?;
    let count: i64 = stmt.query_row([exercise_id], |row| row.get(0)).unwrap_or(0);

    if count == 0 {
        println!(
            "  {} You must attempt the exercise at least once before viewing the solution.",
            "Locked:".bold().yellow()
        );
        println!("  Run: kf run {exercise_id}");
        return Ok(());
    }

    display::show_solution(exercise);
    Ok(())
}

fn cmd_reset() -> Result<(), String> {
    print!(
        "  {} This will delete ALL progress. Type 'yes' to confirm: ",
        "Warning!".bold().red()
    );
    io::stdout().flush().ok();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Read error: {e}"))?;

    if input.trim() == "yes" {
        let conn = progress::open_db()?;
        progress::reset_progress(&conn)?;
        println!("  {}", "Progress reset.".green());
    } else {
        println!("  {}", "Cancelled.".dimmed());
    }
    Ok(())
}
