//! CLI entry point — parses subcommands and dispatches to clings modules.

mod authoring;
mod chapters;
mod commands;
pub mod config;
pub mod constants;
mod error;
mod exam;
mod exercises;
mod libsys;
mod mastery;
mod models;
mod piscine;
mod progress;
mod reporting;
mod runner;
mod search;
mod sync;
mod tmux;
mod tui;
mod watcher;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::commands::{
    cmd_annales, cmd_config, cmd_export, cmd_hint, cmd_import, cmd_list, cmd_new, cmd_progress,
    cmd_report, cmd_reset, cmd_review, cmd_run, cmd_schema, cmd_search, cmd_solution, cmd_stats,
    cmd_sync_init, cmd_sync_now, cmd_sync_status, cmd_watch,
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
    /// Rapport d'apprentissage par chapitre
    Report {
        /// Restreindre à un seul chapitre (1-16)
        #[arg(long, short = 'c')]
        chapter: Option<u8>,
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
        /// Mode de validation : output, test
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
    /// Synchroniser la progression entre machines via Git
    #[command(subcommand)]
    Sync(SyncCommand),
    /// Générer exercise.schema.json pour l'autocomplétion IDE
    Schema {
        /// Fichier de sortie
        #[arg(short, long, default_value = "exercise.schema.json")]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum SyncCommand {
    /// Initialiser le sync Git avec un remote (ex: git@github.com:user/clings-sync.git)
    Init {
        /// URL du remote Git (SSH ou HTTPS)
        remote: String,
    },
    /// Afficher l'état du sync
    Status,
    /// Forcer un sync maintenant (pull + push)
    Now,
}

fn main() {
    // Panic hook : restaure le terminal avant d'afficher le backtrace.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        default_hook(info);
    }));

    config::init(config::load());

    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Watch { chapter }) => cmd_watch(chapter, false),
        Some(Commands::List { subject, due }) => cmd_list(subject.as_deref(), due),
        Some(Commands::Run { exercise_id }) => cmd_run(&exercise_id),
        Some(Commands::Progress { subject }) => cmd_progress(subject.as_deref()),
        Some(Commands::Hint { exercise_id }) => cmd_hint(&exercise_id),
        Some(Commands::Solution { exercise_id }) => cmd_solution(&exercise_id),
        Some(Commands::Reset { subject }) => cmd_reset(subject.as_deref()),
        Some(Commands::Piscine { chapter, timed }) => piscine::cmd_piscine(chapter, timed),
        Some(Commands::Review) => cmd_review(),
        Some(Commands::Stats { detailed }) => cmd_stats(detailed),
        Some(Commands::Report { chapter }) => cmd_report(chapter),
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
        Some(Commands::Sync(SyncCommand::Init { remote })) => cmd_sync_init(&remote),
        Some(Commands::Sync(SyncCommand::Status)) => cmd_sync_status(),
        Some(Commands::Sync(SyncCommand::Now)) => cmd_sync_now(),
        Some(Commands::Schema { output }) => cmd_schema(&output),
        None => (|| {
            let conn = progress::open_db()?;
            loop {
                match tui::ui_launcher::select_launch(&conn)? {
                    tui::ui_launcher::LaunchChoice::Continue => {
                        let (mode, chapter, _index) = progress::load_last_session(&conn)?
                            .unwrap_or_else(|| ("watch".to_string(), None, 0));
                        match mode.as_str() {
                            "piscine" => piscine::cmd_piscine(chapter, None)?,
                            _ => cmd_watch(chapter, false)?,
                        }
                    }
                    tui::ui_launcher::LaunchChoice::Start {
                        mode: tui::ui_launcher::LaunchMode::Watch,
                        chapter,
                    } => cmd_watch(chapter, false)?,
                    tui::ui_launcher::LaunchChoice::Start {
                        mode: tui::ui_launcher::LaunchMode::Piscine,
                        chapter,
                    } => piscine::cmd_piscine(chapter, None)?,
                    tui::ui_launcher::LaunchChoice::Start {
                        mode: tui::ui_launcher::LaunchMode::Nsy103,
                        chapter: _,
                    } => cmd_watch(None, true)?,
                    tui::ui_launcher::LaunchChoice::Start {
                        mode: tui::ui_launcher::LaunchMode::ExamNsy103,
                        chapter: _,
                    } => exam::cmd_exam(None, false)?,
                    tui::ui_launcher::LaunchChoice::Quit => break,
                }
            }
            Ok(())
        })(),
    };

    if let Err(e) = result {
        eprintln!("{} {e}", "Erreur:".bold().red());
        ratatui::restore();
        std::process::exit(1);
    }
}
