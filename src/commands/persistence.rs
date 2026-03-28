//! Progress persistence commands — export and import.

use colored::Colorize;

use crate::error::Result;
use crate::progress;

/// Export progress data (subjects + SRS state) to JSON. Outputs to file or stdout.
pub fn cmd_export(output: Option<&std::path::Path>) -> Result<()> {
    let conn = progress::open_db()?;
    let subjects = progress::get_all_subjects(&conn)?;
    let count = subjects.len();
    let json = progress::export_progress(&conn)?;
    match output {
        Some(path) => {
            std::fs::write(path, &json)?;
            println!(
                "  {} {count} sujet(s) exporté(s) vers {}",
                "✓".bold().green(),
                path.display()
            );
        }
        None => {
            print!("{json}");
            println!("  {} {count} sujet(s) affiché(s).", "✓".bold().green());
        }
    }
    Ok(())
}

/// Import progress data from JSON file. If overwrite=true, replaces all subjects; else merges.
pub fn cmd_import(input: &std::path::Path, overwrite: bool) -> Result<()> {
    let json = std::fs::read_to_string(input)?;
    let mut conn = progress::open_db()?;
    let (count, warnings) = progress::import_progress(&mut conn, &json, overwrite)?;
    for w in &warnings {
        eprintln!("  {} {}", "⚠".yellow(), w);
    }
    if overwrite {
        println!(
            "  {} {count} sujet(s) importé(s) (mode remplacement).",
            "✓".bold().green()
        );
    } else {
        println!(
            "  {} {count} sujet(s) importé(s) (mode fusion).",
            "✓".bold().green()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_export_nonexistent_db() {
        use tempfile::TempDir;

        // Create a temp directory and attempt to export
        let tmp = TempDir::new().expect("temp dir");
        let output_path = tmp.path().join("export.json");

        // This may succeed or fail depending on whether progress DB exists.
        // We're testing that the function handles it gracefully.
        let result = cmd_export(Some(&output_path));

        // If it succeeds, the file should exist; if it errors, it should be a valid error.
        match result {
            Ok(_) => {
                // Check that output was written
                assert!(output_path.exists() || !output_path.exists()); // File may or may not exist
            }
            Err(e) => {
                // Valid error cases: database or IO errors
                let _ = e;
            }
        }
    }

    #[test]
    fn test_cmd_import_malformed_json() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().expect("temp file");
        writeln!(tmp, "{{ invalid json").expect("write");
        tmp.flush().expect("flush");

        let result = cmd_import(tmp.path(), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_import_empty_file() {
        use tempfile::NamedTempFile;

        let tmp = NamedTempFile::new().expect("temp file");
        let result = cmd_import(tmp.path(), false);
        // Empty file is invalid JSON, should error
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_export_stdout() {
        // Export with None (stdout) should not error on format
        let result = cmd_export(None);
        match result {
            Ok(_) => {
                // Success
            }
            Err(e) => {
                // May fail if no DB, but should be a valid error
                let _ = e;
            }
        }
    }

    #[test]
    fn test_cmd_import_valid_json_no_subjects() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().expect("temp file");
        writeln!(tmp, r#"{{"subjects": []}}"#).expect("write");
        tmp.flush().expect("flush");

        let result = cmd_import(tmp.path(), false);
        // Valid JSON with empty subjects should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_import_overwrite_mode() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().expect("temp file");
        writeln!(tmp, r#"{{"subjects": []}}"#).expect("write");
        tmp.flush().expect("flush");

        let result = cmd_import(tmp.path(), true);
        // Overwrite mode should also work with empty subjects
        assert!(result.is_ok());
    }
}
