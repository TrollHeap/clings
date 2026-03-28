//! Sync commands — progress synchronization across machines.

use colored::Colorize;

use crate::constants::clings_data_dir;
use crate::error::{KfError, Result};
use crate::{config, progress, sync};

/// Initialize sync: clone or create a git repository and save remote + branch to config.
pub fn cmd_sync_init(remote: &str) -> Result<()> {
    let clings_dir = clings_data_dir();
    sync::init(remote, &clings_dir)?;
    println!(
        "  {} Sync activé — progression sauvegardée vers {}",
        "✓".bold().green(),
        remote
    );
    println!(
        "  {} Lancez `clings sync init <remote>` sur vos autres machines.",
        "→".bold()
    );
    Ok(())
}

/// Display sync status: enabled/disabled, remote, branch, last commit, and subject count.
pub fn cmd_sync_status() -> Result<()> {
    let cfg = config::get();
    let clings_dir = clings_data_dir();
    let status = sync::status(&clings_dir, &cfg.sync)?;

    println!();
    println!(
        "  Sync : {}",
        if status.enabled {
            "activé".bold().green().to_string()
        } else {
            "désactivé".dimmed().to_string()
        }
    );
    if !status.remote.is_empty() {
        println!("  Remote  : {}", status.remote.bold());
        println!("  Branche : {}", status.branch);
    }
    if let Some(commit) = &status.last_commit {
        println!("  Dernier commit : {commit}");
    }
    println!("  Sujets dans le snapshot : {}", status.subject_count);
    println!();
    Ok(())
}

/// Perform a sync now: pull from remote (merge), push local progress snapshot. Requires sync enabled.
pub fn cmd_sync_now() -> Result<()> {
    let cfg = config::get();
    if !cfg.sync.enabled {
        return Err(KfError::Config(
            "Sync non activé — lancez d'abord `clings sync init <remote>`".to_string(),
        ));
    }
    let clings_dir = clings_data_dir();
    let mut conn = progress::open_db()?;

    // Pull
    match sync::pull_and_merge(&clings_dir, &mut conn) {
        Ok(Some(n)) => println!("  {} {n} sujet(s) mis à jour depuis le remote.", "↪".bold()),
        Ok(None) => println!("  {} Déjà à jour.", "✓".bold().green()),
        Err(e) => eprintln!("  {} pull: {e}", "⚠".yellow()),
    }

    // Push
    sync::export_and_push(&clings_dir, &conn, &cfg.sync)?;
    println!("  {} Progression synchronisée.", "✓".bold().green());
    Ok(())
}
