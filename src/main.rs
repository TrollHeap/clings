//! CLI entry point — parses subcommands and dispatches to clings modules.

mod authoring;
mod chapters;
mod commands;
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
mod search;
mod tmux;
mod tui;
mod watcher;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::commands::{
    cmd_annales, cmd_config, cmd_export, cmd_hint, cmd_import, cmd_list, cmd_new, cmd_progress,
    cmd_reset, cmd_review, cmd_run, cmd_search, cmd_solution, cmd_stats, cmd_watch,
};

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
    /// Générer les completions shell pour bash, zsh, fish, etc.
    Completions {
        /// Shell cible
        shell: clap_complete::Shell,
    },
    /// Rechercher des exercices par mot-clé (fuzzy)
    Search {
        /// Terme de recherche (titre, ID, sujet, concept-clé)
        query: String,
        /// Filtrer par sujet
        #[arg(long, short = 's')]
        subject: Option<String>,
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
        Some(Commands::Completions { shell }) => {
            use clap::CommandFactory;
            clap_complete::generate(shell, &mut Cli::command(), "clings", &mut std::io::stdout());
            Ok(())
        }
        Some(Commands::Search { query, subject }) => cmd_search(&query, subject.as_deref()),
        None => cmd_watch(None),
    };

    if let Err(e) = result {
        eprintln!("{} {e}", "Erreur:".bold().red());
        std::process::exit(1);
    }
}

/// RAII guard that restores crossterm raw mode on drop.
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> Option<Self> {
        crossterm::terminal::enable_raw_mode().ok().map(|_| {
            // crossterm::enable_raw_mode() calls cfmakeraw() which clears OPOST|ONLCR.
            // Restore output processing so println! still emits \r\n in raw mode.
            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                let fd = std::io::stdout().as_raw_fd();
                unsafe {
                    let mut t: libc::termios = std::mem::zeroed();
                    if libc::tcgetattr(fd, &mut t) == 0 {
                        t.c_oflag |= libc::OPOST | libc::ONLCR;
                        let _ = libc::tcsetattr(fd, libc::TCSANOW, &t);
                    }
                }
            }
            Self
        })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

/// Enable terminal raw mode for single-key input.
/// Returns a guard that restores the terminal on drop.
pub(crate) fn enable_raw_mode() -> Option<RawModeGuard> {
    RawModeGuard::enable()
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
