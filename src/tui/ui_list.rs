//! Vue Ratatui pour liste d'exercices — list scrollable avec filtrage.

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use std::time::Duration;

use crate::error::Result;
use crate::models::{Exercise, Subject};
use crate::tui::common;

/// Lance une vue Ratatui autonome pour afficher la liste d'exercices.
pub fn run_list(
    exercises: &[Exercise],
    subjects: &[Subject],
    filter_subject: Option<&str>,
    due_subjects: Option<&[String]>,
) -> Result<()> {
    let mut terminal = ratatui::init();

    // Filter exercises
    let filtered: Vec<&Exercise> = exercises
        .iter()
        .filter(|e| {
            filter_subject.is_none_or(|f| e.subject == f)
                && due_subjects.is_none_or(|due| due.iter().any(|d| d == &e.subject))
        })
        .collect();

    if filtered.is_empty() {
        terminal.draw(|f| {
            let area = f.area();
            f.render_widget(
                Paragraph::new("Aucun exercice trouvé.").block(Block::bordered()),
                area,
            );
        })?;
        std::thread::sleep(Duration::from_secs(1));
        ratatui::restore();
        return Ok(());
    }

    // Build subject map for mastery lookup
    let subject_map: std::collections::HashMap<&str, &Subject> =
        subjects.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut list_state = ListState::default();
    list_state.select(Some(0));

    loop {
        terminal.draw(|f| {
            draw_list(f, &filtered, &subject_map, filter_subject, due_subjects);
        })?;

        // Update selection position
        if let Some(idx) = list_state.selected() {
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break,
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                break
                            }
                            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                                if idx + 1 < filtered.len() {
                                    list_state.select(Some(idx + 1));
                                }
                            }
                            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                                if idx > 0 {
                                    list_state.select(Some(idx - 1));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        } else {
            break;
        }
    }

    ratatui::restore();
    Ok(())
}

fn draw_list(
    f: &mut ratatui::Frame,
    filtered: &[&Exercise],
    subject_map: &std::collections::HashMap<&str, &Subject>,
    _filter_subject: Option<&str>,
    due_subjects: Option<&[String]>,
) {
    let area = f.area();

    // Layout: header (3) | list (fill) | footer (1)
    let [header_area, list_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // ── Header ────────────────────────────────────────────────────────
    let header_title = if due_subjects.is_some() {
        format!("clings — Exercices dus [{}]", filtered.len())
    } else {
        format!("clings — Exercices [{}]", filtered.len())
    };

    let header_text = Line::from(vec![Span::styled(
        header_title,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]);
    f.render_widget(
        Paragraph::new(header_text).block(Block::bordered().title("List")),
        header_area,
    );

    // ── List ──────────────────────────────────────────────────────────
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|ex| {
            let subject = subject_map.get(ex.subject.as_str()).copied();
            let diff = common::difficulty_stars(ex.difficulty);
            let diff_color = common::difficulty_color(ex.difficulty);
            let mastery_info = subject
                .map(|s| format!(" [{:.1}]", s.mastery_score.get()))
                .unwrap_or_default();

            let content = Line::from(vec![
                Span::styled(diff, Style::default().fg(diff_color)),
                Span::raw("  "),
                Span::styled(ex.id.clone(), Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::raw(ex.title.clone()),
                Span::styled(mastery_info, Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(Block::bordered().title("Exercices"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(list, list_area);

    // ── Footer ────────────────────────────────────────────────────────
    let footer_text = "[↑↓/jk] naviguer  [q] quitter";
    f.render_widget(
        Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray)),
        footer_area,
    );
}
