use std::path::Path;
use std::process::Command;

/// Check if we're running inside a tmux session.
pub fn is_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

/// Open a tmux pane on the right (50% width) with neovim editing the given file.
/// Returns the pane ID for later cleanup.
pub fn open_editor_pane(file: &Path) -> Option<String> {
    if !is_tmux() {
        return None;
    }

    let output = Command::new("tmux")
        .args([
            "split-window",
            "-h",
            "-p",
            "50",
            "-P",
            "-F",
            "#{pane_id}",
            "nvim",
            "--",
        ])
        .arg(file)
        .output()
        .ok()?;

    if output.status.success() {
        let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // Validate pane_id format: must be "%N" where N is all digits
        if !pane_id.starts_with('%') || !pane_id[1..].chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        // Return focus to the original pane (left)
        let _ = Command::new("tmux").args(["select-pane", "-L"]).status();
        Some(pane_id)
    } else {
        None
    }
}

/// Kill a tmux pane by ID.
pub fn kill_pane(pane_id: &str) {
    let _ = Command::new("tmux")
        .args(["kill-pane", "-t", pane_id])
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
