//! Vue Ratatui pour annales — table scrollable des sessions et questions.

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Paragraph, Row, Table, TableState};
use ratatui_macros::{line, span, vertical};
use std::time::Duration;

use crate::error::Result;
use crate::models::{AnnaleSession, Exercise};
use crate::tui::common;

/// Lance une vue Ratatui autonome pour afficher les annales.
pub fn run_annales(annales: &[AnnaleSession], exercises: &[Exercise]) -> Result<()> {
    let mut terminal = ratatui::init();
    let mut table_state = TableState::default();

    // Flatten annales into rows (session_id, q#, title, subjects, exercises)
    let rows_data: Vec<(String, String, String, String, String)> = annales
        .iter()
        .flat_map(|session| {
            session.questions.iter().map(move |q| {
                let subjects = q.subjects.join(", ");
                let exercises_str = if !q.exercises.is_empty() {
                    // Reference existing exercises list—no clone needed
                    if q.exercises.len() > 5 {
                        format!(
                            "{}  +{}",
                            q.exercises[..5].join(", "),
                            q.exercises.len() - 5
                        )
                    } else {
                        q.exercises.join(", ")
                    }
                } else {
                    // Build exercise IDs from filtered exercises
                    let ids: Vec<String> = exercises
                        .iter()
                        .filter(|e| q.subjects.iter().any(|s| s == &e.subject))
                        .map(|e| e.id.clone())
                        .collect();
                    if ids.len() > 5 {
                        format!("{}  +{}", ids[..5].join(", "), ids.len() - 5)
                    } else {
                        ids.join(", ")
                    }
                };
                (
                    session.id.clone(),
                    format!("Q{}", q.number),
                    q.title.clone(),
                    subjects,
                    exercises_str,
                )
            })
        })
        .collect();

    if rows_data.is_empty() {
        ratatui::restore();
        eprintln!("Aucune annale disponible.");
        return Ok(());
    }

    table_state.select(Some(0));

    loop {
        terminal.draw(|f| {
            draw_annales(f, &rows_data, &mut table_state);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break
                        }
                        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                            let current = table_state.selected().unwrap_or(0);
                            if current + 1 < rows_data.len() {
                                table_state.select(Some(current + 1));
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                            let current = table_state.selected().unwrap_or(0);
                            if current > 0 {
                                table_state.select(Some(current - 1));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    ratatui::restore();
    Ok(())
}

fn draw_annales(
    f: &mut ratatui::Frame,
    rows_data: &[(String, String, String, String, String)],
    table_state: &mut TableState,
) {
    let area = f.area();

    // Fond global opaque — évite la transparence terminal
    f.render_widget(
        Block::default().style(Style::default().bg(common::C_BG)),
        area,
    );

    // Layout: header (3) | table (fill) | footer (1)
    let [header_area, table_area, footer_area] = vertical![==3, *=1, ==1].areas(area);

    // ── Header ────────────────────────────────────────────────────────
    let header_text = line![
        span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "Annales NSY103"),
        Span::raw(" — correspondance exercices"),
    ];
    f.render_widget(
        Paragraph::new(header_text).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(common::C_BORDER))
                .style(Style::default().bg(common::C_BG))
                .title("Annales"),
        ),
        header_area,
    );

    // ── Table ─────────────────────────────────────────────────────────
    let rows: Vec<Row> = rows_data
        .iter()
        .map(|(session, q_num, title, subjects, exercises)| {
            Row::new(vec![
                session.as_str(),
                q_num.as_str(),
                title.as_str(),
                subjects.as_str(),
                exercises.as_str(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(4),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
            Constraint::Percentage(36),
        ],
    )
    .header(
        Row::new(vec!["Session", "Q#", "Titre", "Sujets", "Exercices"]).style(
            Style::default()
                .fg(common::C_ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
    )
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(common::C_BORDER))
            .style(Style::default().bg(common::C_BG)),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(table, table_area, table_state);

    // ── Footer ────────────────────────────────────────────────────────
    let footer_text = "[↑↓/jk] naviguer  [q] quitter";
    f.render_widget(
        Paragraph::new(footer_text).style(Style::default().fg(common::C_OVERLAY)),
        footer_area,
    );
}
