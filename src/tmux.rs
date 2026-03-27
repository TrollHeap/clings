//! Editor integration — opens the exercise file for editing.
//!
//! Inside tmux (and `tmux.enabled = true`): splits a pane with the configured editor.
//! Otherwise: launches the editor as a detached background process.
//! Editor binary is validated before use.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::constants::TMUX_EDITOR;

/// Check if we're running inside a tmux session.
pub fn is_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

/// Validates that `bin` is a safe, existing executable.
/// Absolute paths are checked with `is_file()`; relative names are character-whitelisted
/// then resolved via `command -v` (injection-safe via positional args).
fn is_valid_executable(bin: &str) -> bool {
    let path = std::path::Path::new(bin);
    if path.is_absolute() {
        return path.is_file();
    }
    // Reject names with characters that could inject into a shell command.
    if !bin
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return false;
    }
    // Use the POSIX shell builtin `command -v` with a positional argument ($1)
    // so `bin` is never interpolated into the shell command string — no injection possible.
    Command::new("sh")
        .args(["-c", "command -v \"$1\"", "--", bin])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Resolve the editor binary with cascading fallback: `$VISUAL` → `$EDITOR` → config → `nvim`.
/// Validates the resolved binary before returning; falls back to `TMUX_EDITOR` if invalid.
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

/// Éditeurs GUI connus qui se détachent naturellement du terminal.
const GUI_EDITORS: &[&str] = &[
    "code",
    "codium",
    "subl",
    "sublime_text",
    "gedit",
    "kate",
    "xed",
    "mousepad",
    "pluma",
    "atom",
    "zed",
    "lapce",
    "lite-xl",
    "gvim",
];

/// Vérifie si le binaire est un éditeur GUI (peut tourner en parallèle du TUI).
/// Gère aussi les IDs Flatpak reverse-DNS (ex: `com.visualstudio.code` → `code`).
fn is_gui_editor(bin: &str) -> bool {
    let name = std::path::Path::new(bin)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(bin);
    if GUI_EDITORS.contains(&name) {
        return true;
    }
    // Flatpak reverse-DNS: extract last segment (e.g. "com.visualstudio.code" → "code")
    if let Some(last) = name.rsplit('.').next() {
        return GUI_EDITORS.contains(&last);
    }
    false
}

/// Lance l'éditeur en background (hors tmux).
/// Retourne `true` si l'éditeur a été lancé, `false` si c'est un éditeur terminal
/// (qui ne peut pas fonctionner sans TTY en parallèle du TUI).
fn open_editor_standalone(file: &Path) -> bool {
    let editor = resolve_editor();
    let parts: Vec<&str> = editor.split_whitespace().collect();
    let (bin, args) = match parts.split_first() {
        Some(v) => v,
        None => return false,
    };
    if !is_gui_editor(bin) {
        return false;
    }
    if let Err(e) = Command::new(bin)
        .args(args.iter().copied())
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        eprintln!(
            "[clings/tmux] avertissement : impossible de lancer l'éditeur '{}' : {e}",
            bin
        );
    }
    true
}

/// Open a tmux pane on the right with the configured editor editing the given file.
/// Returns the pane ID for later cleanup.
/// Falls back to `open_editor_standalone` when not in tmux or when `tmux.enabled = false`.
pub fn open_editor_pane(file: &Path) -> Option<String> {
    if !is_tmux() || !crate::config::get().tmux.enabled {
        if !open_editor_standalone(file) {
            eprintln!("  [clings] Ouvrez {} dans votre éditeur", file.display());
        }
        return None;
    }

    let editor = resolve_editor();
    let pane_width = crate::config::get().ui.tmux_pane_width.to_string();
    // Split editor string into binary + extra args so that editors like "nvim -u init.lua"
    // are passed to tmux as separate tokens, not as a single arg.
    let editor_parts: Vec<&str> = editor.split_whitespace().collect();
    let (editor_bin, editor_args) = editor_parts.split_first()?;
    // Reject any arg that contains shell-special characters to prevent injection.
    // '/' is allowed for absolute editor paths (e.g. /usr/bin/nvim) and path flags (e.g. --cmd /path).
    let safe_chars = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/');
    if editor_args.iter().any(|a| !a.chars().all(safe_chars)) {
        return None;
    }
    let output = Command::new("tmux")
        .args([
            "split-window",
            "-h",
            "-p",
            &pane_width,
            "-P",
            "-F",
            "#{pane_id}",
        ])
        .arg(editor_bin)
        .args(editor_args)
        .arg("--")
        .arg(file)
        .output()
        .map_err(|e| eprintln!("[clings/tmux] impossible de lancer tmux split-window : {e}"))
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

    #[test]
    fn test_is_valid_executable_rejects_shell_chars() {
        assert!(!is_valid_executable("vim;rm -rf"));
        assert!(!is_valid_executable("vim$(whoami)"));
        assert!(!is_valid_executable("vim`id`"));
        assert!(!is_valid_executable("vim | cat"));
        assert!(!is_valid_executable(""));
    }

    #[test]
    fn test_is_valid_executable_accepts_valid_names() {
        // These may or may not exist — test the character validation, not PATH resolution
        assert!(is_valid_executable("sh")); // should exist on all POSIX systems
    }

    #[test]
    fn test_is_valid_executable_absolute_nonexistent() {
        assert!(!is_valid_executable("/nonexistent/path/to/editor"));
    }

    #[test]
    fn test_is_gui_editor() {
        assert!(is_gui_editor("code"));
        assert!(is_gui_editor("/usr/bin/code"));
        assert!(is_gui_editor("subl"));
        assert!(is_gui_editor("zed"));
        assert!(is_gui_editor("gvim"));
        assert!(!is_gui_editor("nvim"));
        assert!(!is_gui_editor("vim"));
        assert!(!is_gui_editor("nano"));
        assert!(!is_gui_editor("emacs"));
    }

    #[test]
    fn test_is_gui_editor_flatpak() {
        assert!(is_gui_editor("com.visualstudio.code"));
        assert!(is_gui_editor("org.kde.kate"));
        assert!(is_gui_editor("org.codium.codium"));
        assert!(!is_gui_editor("org.vim.Vim")); // "Vim" ≠ "vim" — case-sensitive
        assert!(!is_gui_editor("org.neovim.nvim")); // nvim is terminal
    }
}
