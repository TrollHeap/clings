//! Optional tmux integration — opens the exercise file in a neovim split.
//!
//! Activates only when running inside a tmux session (`$TMUX` is set).
//! Editor binary is validated before use. Falls back gracefully if tmux is unavailable.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::constants::TMUX_EDITOR;

/// Check if we're running inside a tmux session.
pub fn is_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

fn is_valid_executable(bin: &str) -> bool {
    let path = std::path::Path::new(bin);
    if path.is_absolute() {
        return path.is_file();
    }
    Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Resolve the editor binary: $VISUAL → $EDITOR → config/TMUX_EDITOR fallback.
fn resolve_editor() -> String {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| {
            let cfg_editor = &crate::config::get().ui.editor;
            if cfg_editor.is_empty() {
                TMUX_EDITOR.to_owned()
            } else {
                cfg_editor.clone()
            }
        });

    let bin = editor.split_whitespace().next().unwrap_or("");
    if is_valid_executable(bin) {
        editor
    } else {
        eprintln!(
            "  [clings] éditeur invalide '{}', fallback sur {}",
            editor, TMUX_EDITOR
        );
        TMUX_EDITOR.to_owned()
    }
}

/// Open a tmux pane on the right with the configured editor editing the given file.
/// Returns the pane ID for later cleanup.
pub fn open_editor_pane(file: &Path) -> Option<String> {
    if !is_tmux() {
        return None;
    }

    let editor = resolve_editor();
    let pane_width = crate::config::get().ui.tmux_pane_width.to_string();
    let output = Command::new("tmux")
        .args([
            "split-window",
            "-h",
            "-p",
            &pane_width,
            "-P",
            "-F",
            "#{pane_id}",
            &editor,
            "--",
        ])
        .arg(file)
        .output()
        .ok()?;

    if output.status.success() {
        let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // Validate pane_id format: must be "%N" where N is all digits
        if !pane_id
            .strip_prefix('%')
            .map(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false)
        {
            return None;
        }
        // Return focus to the original pane (left) — best-effort, ignore error
        let _ = Command::new("tmux").args(["select-pane", "-L"]).status();
        Some(pane_id)
    } else {
        None
    }
}

/// Kill a tmux pane by ID.
pub fn kill_pane(pane_id: &str) {
    // Pane cleanup is best-effort — ignore error (pane may already be dead)
    let _ = Command::new("tmux")
        .args(["kill-pane", "-t", pane_id])
        .stderr(Stdio::null())
        .status();
}

/// Update the neovim pane to edit a new file.
/// Kills the old pane and opens a new one.
pub fn update_editor_pane(old_pane: Option<&str>, new_file: &Path) -> Option<String> {
    if let Some(id) = old_pane {
        kill_pane(id);
    }
    open_editor_pane(new_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_tmux_matches_env_contract() {
        // is_tmux() must exactly mirror whether TMUX env var is present.
        let expected = std::env::var("TMUX").is_ok();
        assert_eq!(is_tmux(), expected);
    }

    #[test]
    fn test_open_editor_pane_none_without_tmux() {
        // If TMUX is not set, open_editor_pane should short-circuit and return None
        // without attempting to spawn any process.
        if std::env::var("TMUX").is_ok() {
            // Running inside tmux: skip this test to avoid spawning a real pane.
            return;
        }
        let result = open_editor_pane(Path::new("/tmp/test_clings.c"));
        assert!(
            result.is_none(),
            "should return None when not in a tmux session"
        );
    }

    #[test]
    fn test_update_editor_pane_none_without_tmux() {
        if std::env::var("TMUX").is_ok() {
            return;
        }
        // No old pane, no tmux → must return None
        let result = update_editor_pane(None, Path::new("/tmp/test_clings.c"));
        assert!(result.is_none());
    }
}
