//! Ratatui TUI exam session selector.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::models::AnnaleSession;

/// Ratatui-based exam session selector.
/// Retourne l'ID de la session choisie, ou None si annulé.
pub fn select_exam_session(
    sessions: &[AnnaleSession],
    last_session_id: Option<&str>,
) -> Option<String> {
    if sessions.is_empty() {
        return None;
    }

    let initial_cursor = last_session_id
        .and_then(|id| sessions.iter().position(|s| s.id == id))
        .unwrap_or(0);
    let mut cursor = initial_cursor;
    let mut list_state = ListState::default();
    list_state.select(Some(cursor));

    let mut terminal = ratatui::init();
    let result = run_selector_loop(&mut terminal, sessions, &mut cursor, &mut list_state);
    ratatui::restore();

    result
}

fn run_selector_loop(
    terminal: &mut ratatui::DefaultTerminal,
    sessions: &[AnnaleSession],
    cursor: &mut usize,
    list_state: &mut ListState,
) -> Option<String> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    use std::time::Duration;

    loop {
        let _ = terminal.draw(|f| {
            draw_selector(f, sessions, *cursor);
        });

        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                            *cursor = cursor.saturating_sub(1);
                            list_state.select(Some(*cursor));
                        }
                        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                            if *cursor + 1 < sessions.len() {
                                *cursor += 1;
                            }
                            list_state.select(Some(*cursor));
                        }
                        KeyCode::Enter => {
                            return Some(sessions[*cursor].id.clone());
                        }
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            return None;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn draw_selector(f: &mut Frame, sessions: &[AnnaleSession], cursor: usize) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("Sélectionner une session d'exam")
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, chunks[0]);

    // List
    let items: Vec<ListItem> = sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let content = if i == cursor {
                format!(
                    "▶ {}  —  {}  ({} pts)",
                    s.id, s.title, s.total_points as i32
                )
            } else {
                format!(
                    "  {}  —  {}  ({} pts)",
                    s.id, s.title, s.total_points as i32
                )
            };
            ListItem::new(Line::raw(content))
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Sessions"));
    f.render_widget(list, chunks[1]);

    // Footer
    let footer =
        Paragraph::new("[↑↓/jk] naviguer  [Entrée] lancer  [q] annuler").alignment(Alignment::Left);
    f.render_widget(footer, chunks[2]);
}
