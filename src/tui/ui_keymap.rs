//! Key event → Command mapping for watch and piscine modes.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::ui_messages::Command;

/// Mappe un `KeyEvent` vers une `Command` sémantique.
///
/// `piscine` : `true` = mode piscine (désactive `OpenList`, `ShowHelp`, `OpenSearch`, défilement).
pub fn key_to_cmd(key: &KeyEvent, piscine: bool) -> Option<Command> {
    Some(match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Command::Quit,
        KeyCode::Char('r') | KeyCode::Char('R') => Command::CompileRun,
        KeyCode::Char('h') | KeyCode::Char('H') => Command::ShowHint,
        KeyCode::Char('s') | KeyCode::Char('S') => Command::ToggleSolution,
        KeyCode::Char('v') | KeyCode::Char('V') => Command::OpenVisualizer,
        KeyCode::Char('b') | KeyCode::Char('B') => Command::OpenLibsys,
        KeyCode::Char('j') | KeyCode::Char('J') | KeyCode::Char('n') | KeyCode::Char('N') => {
            Command::NavNext
        }
        KeyCode::Char('k') | KeyCode::Char('K') => Command::NavPrev,
        KeyCode::Char('/') => Command::OpenSearch,
        KeyCode::Char('l') | KeyCode::Char('L') => Command::OpenList,
        KeyCode::Char('?') if !piscine => Command::ShowHelp,
        KeyCode::PageDown => Command::ScrollDown,
        KeyCode::PageUp => Command::ScrollUp,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Command::Quit,
        KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => Command::Quit,
        _ => return None,
    })
}
