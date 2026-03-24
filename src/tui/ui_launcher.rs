//! Ratatui launch mode and chapter selector.

use ratatui::{
    layout::{Constraint, Direction, HorizontalAlignment, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use ratatui_macros::span;
use rusqlite::Connection;

use crate::chapters::CHAPTERS;
use crate::progress;
use crate::tui::common;

/// Result of the launcher selection.
pub enum LaunchChoice {
    /// Resume the last session.
    Continue,
    /// Start a new session with the given mode and optional chapter filter.
    Start {
        mode: LaunchMode,
        chapter: Option<u8>,
    },
    /// User quit without selecting.
    Quit,
}

/// Launch mode selection.
#[derive(Clone, Copy)]
pub enum LaunchMode {
    Watch,
    Piscine,
}

/// Which screen the launcher is showing.
enum Screen {
    Mode,
    Chapter(LaunchMode),
    Help,
}

/// Ratatui-based launch selector. Shows mode selection, then chapter selection.
pub fn select_launch(conn: &Connection) -> LaunchChoice {
    let last_session = progress::load_last_session(conn).ok().flatten();

    let mut screen = Screen::Mode;
    let mut cursor: usize = 0;
    let mut list_state = ListState::default();
    list_state.select(Some(0));

    let mut terminal = ratatui::init();

    let result = loop {
        match &screen {
            Screen::Mode => {
                let has_continue = last_session.is_some();
                let item_count = if has_continue { 3 } else { 2 };

                let _ = terminal.draw(|f| {
                    draw_mode_screen(f, cursor, &last_session);
                });

                match read_key() {
                    Some(Action::Up) => {
                        cursor = cursor.saturating_sub(1);
                        list_state.select(Some(cursor));
                    }
                    Some(Action::Down) => {
                        if cursor + 1 < item_count {
                            cursor += 1;
                        }
                        list_state.select(Some(cursor));
                    }
                    Some(Action::Select) => {
                        if has_continue && cursor == 0 {
                            break LaunchChoice::Continue;
                        }
                        let offset = if has_continue { 1 } else { 0 };
                        match cursor.saturating_sub(offset) {
                            0 => {
                                screen = Screen::Chapter(LaunchMode::Watch);
                                cursor = 0;
                                list_state.select(Some(0));
                            }
                            _ => {
                                screen = Screen::Chapter(LaunchMode::Piscine);
                                cursor = 0;
                                list_state.select(Some(0));
                            }
                        }
                    }
                    Some(Action::Help) => {
                        screen = Screen::Help;
                    }
                    Some(Action::Quit) => break LaunchChoice::Quit,
                    Some(Action::Back) => break LaunchChoice::Quit,
                    None => {}
                }
            }
            Screen::Help => {
                let _ = terminal.draw(|f| draw_help_screen(f));
                if let Some(_) = read_key() {
                    screen = Screen::Mode;
                }
            }
            Screen::Chapter(mode) => {
                let mode = *mode;
                // "Tous les chapitres" + 16 chapters
                let item_count = 1 + CHAPTERS.len();

                let _ = terminal.draw(|f| {
                    draw_chapter_screen(f, cursor, mode);
                });

                match read_key() {
                    Some(Action::Up) => {
                        cursor = cursor.saturating_sub(1);
                        list_state.select(Some(cursor));
                    }
                    Some(Action::Down) => {
                        if cursor + 1 < item_count {
                            cursor += 1;
                        }
                        list_state.select(Some(cursor));
                    }
                    Some(Action::Select) => {
                        let chapter = if cursor == 0 {
                            None
                        } else {
                            Some(CHAPTERS[cursor - 1].number)
                        };
                        break LaunchChoice::Start { mode, chapter };
                    }
                    Some(Action::Help) => {
                        screen = Screen::Help;
                    }
                    Some(Action::Quit) => break LaunchChoice::Quit,
                    Some(Action::Back) => {
                        screen = Screen::Mode;
                        cursor = 0;
                        list_state.select(Some(0));
                    }
                    None => {}
                }
            }
        }
    };

    ratatui::restore();
    result
}

enum Action {
    Up,
    Down,
    Select,
    Quit,
    Back,
    Help,
}

fn read_key() -> Option<Action> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    use std::time::Duration;

    if event::poll(Duration::from_millis(100)).unwrap_or(false) {
        if let Ok(Event::Key(key)) = event::read() {
            if key.kind == KeyEventKind::Press {
                return match key.code {
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => Some(Action::Up),
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => Some(Action::Down),
                    KeyCode::Enter => Some(Action::Select),
                    KeyCode::Char('q') | KeyCode::Char('Q') => Some(Action::Quit),
                    KeyCode::Esc => Some(Action::Back),
                    KeyCode::Char('?') => Some(Action::Help),
                    _ => None,
                };
            }
        }
    }
    None
}

fn draw_mode_screen(
    f: &mut Frame,
    cursor: usize,
    last_session: &Option<(String, Option<u8>, usize)>,
) {
    f.render_widget(
        Block::default().style(Style::default().bg(common::C_BG)),
        f.area(),
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " clings",
            Style::default()
                .fg(common::C_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            concat!(" v", env!("CARGO_PKG_VERSION")),
            Style::default().fg(common::C_TEXT_DIM),
        ),
        Span::styled(
            " — C Systems Programming Trainer",
            Style::default().fg(common::C_TEXT_DIM),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(common::C_BORDER)),
    );
    f.render_widget(header, chunks[0]);

    // List items
    let mut items: Vec<ListItem> = Vec::new();
    let mut idx = 0usize;

    if let Some((mode, chapter, _index)) = last_session {
        let ch_label = chapter
            .and_then(|n| CHAPTERS.iter().find(|c| c.number == n))
            .map(|c| format!("Ch.{} {}", c.number, c.title))
            .unwrap_or_else(|| "Tous les chapitres".to_string());
        let mode_label = if mode == "piscine" {
            "Piscine"
        } else {
            "Watch"
        };
        let label = format!("Continuer ({} — {})", ch_label, mode_label);
        items.push(make_item(&label, idx == cursor, common::C_SUCCESS));
        idx += 1;
    }

    items.push(make_item(
        "Watch (progression SRS)",
        idx == cursor,
        common::C_ACCENT,
    ));
    idx += 1;

    items.push(make_item(
        "Piscine (linéaire)",
        idx == cursor,
        common::C_WARNING,
    ));

    let list = List::new(items)
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(common::C_BORDER))
                .style(Style::default().bg(common::C_BG))
                .title(span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "Mode")),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_widget(list, chunks[1]);

    // Footer
    let footer = Paragraph::new("[↑↓/jk] naviguer  [Entrée] lancer  [?] aide  [q] quitter")
        .style(Style::default().fg(common::C_TEXT_DIM))
        .alignment(HorizontalAlignment::Left);
    f.render_widget(footer, chunks[2]);
}

fn draw_chapter_screen(f: &mut Frame, cursor: usize, mode: LaunchMode) {
    f.render_widget(
        Block::default().style(Style::default().bg(common::C_BG)),
        f.area(),
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(f.area());

    let mode_name = match mode {
        LaunchMode::Watch => "Watch",
        LaunchMode::Piscine => "Piscine",
    };

    // Header
    let header = Paragraph::new(Line::from(vec![Span::styled(
        format!(" clings > {}", mode_name),
        Style::default()
            .fg(common::C_ACCENT)
            .add_modifier(Modifier::BOLD),
    )]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(common::C_BORDER)),
    );
    f.render_widget(header, chunks[0]);

    // List: "Tous" + chapters
    let mut items: Vec<ListItem> = Vec::new();
    items.push(make_item(
        "Tous les chapitres",
        cursor == 0,
        common::C_ACCENT,
    ));

    for (i, ch) in CHAPTERS.iter().enumerate() {
        let label = format!("Ch.{:<2} {}", ch.number, ch.title);
        items.push(make_item(&label, cursor == i + 1, common::C_TEXT));
    }

    let list = List::new(items)
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(common::C_BORDER))
                .style(Style::default().bg(common::C_BG))
                .title(span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "Chapitre")),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_widget(list, chunks[1]);

    // Footer
    let footer = Paragraph::new("[↑↓/jk] naviguer  [Entrée] lancer  [Esc] retour  [q] quitter")
        .style(Style::default().fg(common::C_TEXT_DIM))
        .alignment(HorizontalAlignment::Left);
    f.render_widget(footer, chunks[2]);
}

fn draw_help_screen(f: &mut Frame) {
    f.render_widget(
        Block::default().style(Style::default().bg(common::C_BG)),
        f.area(),
    );

    let area = f.area();
    let popup_w = area.width.min(60);
    let popup_h = area.height.min(30);
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let popup_area = ratatui::layout::Rect::new(x, y, popup_w, popup_h);

    let bold_accent = Style::default()
        .fg(common::C_ACCENT)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(common::C_TEXT_DIM);
    let warn = Style::default().fg(common::C_WARNING);
    let text = Style::default().fg(common::C_TEXT);
    let success = Style::default().fg(common::C_SUCCESS);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  MODES", bold_accent)),
        Line::from(vec![
            Span::styled("  Watch (SRS)   ", warn),
            Span::styled("Progression adaptative par maîtrise.", text),
        ]),
        Line::from(Span::styled(
            "                 Seuls les exercices débloqués sont visibles.",
            dim,
        )),
        Line::from(Span::styled(
            "                 Recommandé pour l'apprentissage quotidien.",
            dim,
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Piscine        ", warn),
            Span::styled("Linéaire, tout débloqué.", text),
        ]),
        Line::from(Span::styled(
            "                 Idéal pour réviser en une seule session.",
            dim,
        )),
        Line::from(""),
        Line::from(Span::styled("  MAÎTRISE (SRS)", bold_accent)),
        Line::from(vec![
            Span::styled("  Score 0 → 5   ", warn),
            Span::styled("Succès : +1.0  │  Échec : −0.5", text),
        ]),
        Line::from(vec![
            Span::styled("  Decay         ", warn),
            Span::styled("−0.5 tous les 14 jours sans pratique", text),
        ]),
        Line::from(""),
        Line::from(Span::styled("  DIFFICULTÉS (par sujet)", bold_accent)),
        Line::from(vec![
            Span::styled("  ★         D1  ", success),
            Span::styled("Toujours disponible", text),
        ]),
        Line::from(vec![
            Span::styled("  ★★        D2  ", success),
            Span::styled("Maîtrise ≥ 2.0", text),
        ]),
        Line::from(vec![
            Span::styled("  ★★★       D3  ", warn),
            Span::styled("Maîtrise ≥ 4.0", text),
        ]),
        Line::from(vec![
            Span::styled("  ★★★★      D4  ", warn),
            Span::styled("Maîtrise ≥ 4.5", text),
        ]),
        Line::from(vec![
            Span::styled("  ★★★★★     D5  ", Style::default().fg(common::C_DANGER)),
            Span::styled("Maîtrise = 5.0", text),
        ]),
        Line::from(""),
        Line::from(Span::styled("  TOUCHES (mode Watch)", bold_accent)),
        Line::from(vec![
            Span::styled("  [r] ", warn),
            Span::styled("compiler    ", text),
            Span::styled("[h] ", warn),
            Span::styled("indice progressif    ", text),
            Span::styled("[s] ", warn),
            Span::styled("solution", text),
        ]),
        Line::from(vec![
            Span::styled("  [j/k] ", warn),
            Span::styled("suivant/préc  ", text),
            Span::styled("[l] ", warn),
            Span::styled("liste    ", text),
            Span::styled("[v] ", warn),
            Span::styled("visualiseur", text),
        ]),
        Line::from(vec![
            Span::styled("  [/] ", warn),
            Span::styled("recherche fuzzy              ", text),
            Span::styled("[q] ", warn),
            Span::styled("quitter", text),
        ]),
        Line::from(""),
        Line::from(Span::styled("         ↵ toute touche pour fermer", dim)),
        Line::from(""),
    ];

    let help = Paragraph::new(lines).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(common::C_ACCENT))
            .style(Style::default().bg(common::C_BG))
            .title(span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); " ? Aide ")),
    );
    f.render_widget(help, popup_area);
}

fn make_item(label: &str, selected: bool, color: ratatui::style::Color) -> ListItem<'static> {
    let prefix = if selected { "▶ " } else { "  " };
    let style = if selected {
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(common::C_SUBTEXT)
    };
    ListItem::new(Line::styled(format!("{}{}", prefix, label), style))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_choice_variants_exist() {
        // Verify the enum variants compile and can be constructed
        let _c = LaunchChoice::Continue;
        let _s = LaunchChoice::Start {
            mode: LaunchMode::Watch,
            chapter: Some(7),
        };
        let _q = LaunchChoice::Quit;
    }
}
