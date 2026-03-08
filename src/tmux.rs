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
            &format!("nvim {}", file.display()),
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
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
