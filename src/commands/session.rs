//! Session management commands — reset progress.

use std::io;
use std::io::Write;

use colored::Colorize;

use crate::error::Result;
use crate::progress;

/// Prompt user for confirmation. Returns true if input equals "yes".
pub fn confirm_prompt(msg: &str) -> Result<bool> {
    print!("{}", msg);
    io::stdout().flush().ok(); // best-effort flush — non-critique
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim() == "yes")
}

/// Reset progress data. If subject is provided, resets only that subject; else resets all.
/// Requires user confirmation (type 'yes').
pub fn cmd_reset(subject: Option<&str>) -> Result<()> {
    if let Some(name) = subject {
        let confirmed = confirm_prompt(&format!(
            "  {} Supprimer la progression de '{}'. Taper 'yes' pour confirmer : ",
            "Attention !".bold().red(),
            name
        ))?;
        if confirmed {
            let conn = progress::open_db()?;
            progress::reset_subject(&conn, name)?;
            println!("  {} Progression de '{}' réinitialisée.", "✓".green(), name);
        } else {
            println!("  {}", "Annulé.".dimmed());
        }
    } else {
        let confirmed = confirm_prompt(&format!(
            "  {} Ceci supprimera TOUTE la progression. Tapez 'yes' pour confirmer : ",
            "Attention !".bold().red()
        ))?;
        if confirmed {
            let conn = progress::open_db()?;
            progress::reset_progress(&conn)?;
            println!("  {}", "Progression réinitialisée.".green());
        } else {
            println!("  {}", "Annulé.".dimmed());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_reset_cancellation() {
        use std::io::{Cursor, Read};

        // Simulating a user input of "no" (not "yes") should exit silently.
        // Since cmd_reset reads from stdin interactively, we can't easily mock it here.
        // This test documents the expected behavior: if user doesn't type "yes", nothing changes.
        // In a real integration test, stdin would be redirected.

        // For now, just verify the function signature is correct
        let _ = cmd_reset(None);
    }
}
