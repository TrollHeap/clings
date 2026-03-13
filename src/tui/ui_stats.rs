//! Vue Ratatui pour statistiques — mastery moyenne, sparkline, table par sujet.

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Row, Sparkline, Table, TableState};
use std::time::Duration;

use crate::error::Result;
use crate::models::Subject;
use crate::tui::common;

fn avg_mastery(subjects: &[Subject]) -> f64 {
    if subjects.is_empty() {
        return 0.0;
    }
    subjects.iter().map(|s| s.mastery_score.get()).sum::<f64>() / subjects.len() as f64
}

/// Render mastery bar as (█░░░░░░░░░ format) + score as Spans for Ratatui.
fn mastery_bar_spans(score: f64) -> Vec<Span<'static>> {
    let bar = common::mastery_bar_string(score, 10);
    let color = common::mastery_color(score);
    vec![
        Span::styled(bar, Style::default().fg(color)),
        Span::raw(format!(" {:.1}", score)),
    ]
}

/// Lance une vue Ratatui autonome pour afficher les statistiques.
pub fn run_stats(
    subjects: &[Subject],
    streak: u32,
    attempts: Option<&[(String, u32, u32)]>,
    daily: Option<&[(String, u32)]>,
) -> Result<()> {
    let mut terminal = ratatui::init();
    let mut table_state = TableState::default();

    loop {
        terminal.draw(|f| {
            draw_stats(f, subjects, streak, attempts, daily, &mut table_state);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break
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

fn draw_stats(
    f: &mut ratatui::Frame,
    subjects: &[Subject],
    streak: u32,
    attempts: Option<&[(String, u32, u32)]>,
    daily: Option<&[(String, u32)]>,
    table_state: &mut TableState,
) {
    let area = f.area();

    // Layout: header (3) | body (fill) | footer (1)
    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // ── Header ────────────────────────────────────────────────────────
    let header_text = Line::from(vec![Span::styled(
        "clings — Statistiques",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]);
    f.render_widget(
        Paragraph::new(header_text).block(Block::bordered().title("Stats")),
        header_area,
    );

    // ── Body ──────────────────────────────────────────────────────────
    if subjects.is_empty() {
        let text = "Aucun sujet pratiqué pour l'instant.";
        f.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::bordered()),
            body_area,
        );
    } else {
        // Build content lines
        let mut lines: Vec<Line> = Vec::new();

        // Streak
        lines.push(Line::from(vec![
            Span::styled("Série: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                streak.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  jours consécutifs"),
        ]));

        let avg = avg_mastery(subjects);

        // Average mastery
        let mut avg_line = vec![Span::styled(
            "Maîtrise moyenne: ",
            Style::default().fg(Color::Cyan),
        )];
        avg_line.extend(mastery_bar_spans(avg));
        lines.push(Line::from(avg_line));
        lines.push(Line::raw(""));

        // Determine which section to show
        let content_paragraph = Paragraph::new(lines).block(Block::bordered());

        // Sparkline for daily activity
        if let Some(daily_data) = daily {
            if !daily_data.is_empty() {
                let counts: Vec<u64> = daily_data.iter().map(|(_, c)| *c as u64).collect();
                let total_attempts: u32 = daily_data.iter().map(|(_, c)| c).sum();

                // Layout for sparkline + stats below
                let [para_area, spark_area] =
                    Layout::vertical([Constraint::Length(4), Constraint::Fill(1)]).areas(body_area);

                f.render_widget(content_paragraph, para_area);

                let sparkline = Sparkline::default()
                    .block(
                        Block::bordered()
                            .title(format!("Activité 30j — {} tentatives", total_attempts)),
                    )
                    .data(&counts)
                    .style(Style::default().fg(Color::Yellow));

                f.render_widget(sparkline, spark_area);
                return;
            }
        }

        // Attempts per subject table
        if let Some(attempts_data) = attempts {
            if !attempts_data.is_empty() {
                let rows: Vec<Row> = attempts_data
                    .iter()
                    .map(|(name, succ, fail)| {
                        let total = succ + fail;
                        Row::new(vec![
                            name.clone(),
                            succ.to_string(),
                            fail.to_string(),
                            total.to_string(),
                        ])
                    })
                    .collect();

                let table = Table::new(
                    rows,
                    [
                        Constraint::Percentage(40),
                        Constraint::Percentage(20),
                        Constraint::Percentage(20),
                        Constraint::Percentage(20),
                    ],
                )
                .header(
                    Row::new(vec!["Sujet", "Succès", "Échecs", "Total"]).style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                )
                .block(Block::bordered().title("Tentatives par sujet"));

                f.render_widget(table, body_area);
                return;
            }
        }

        // Mastery table (default)
        let mut sorted: Vec<&Subject> = subjects.iter().collect();
        sorted.sort_by(|a, b| {
            b.mastery_score
                .get()
                .partial_cmp(&a.mastery_score.get())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let rows: Vec<Row> = sorted
            .iter()
            .take(15)
            .map(|s| {
                let score = s.mastery_score.get();
                Row::new(vec![
                    s.name.clone(),
                    format!("{:.1}", score),
                    common::mastery_bar_string(score, 10),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(40),
                Constraint::Percentage(15),
                Constraint::Percentage(45),
            ],
        )
        .header(
            Row::new(vec!["Sujet", "Maîtrise", "Barre"]).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
        )
        .block(Block::bordered().title("Maîtrise par sujet"));

        f.render_stateful_widget(table, body_area, table_state);
    }

    // ── Footer ────────────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new("[q] quitter").style(Style::default().fg(Color::DarkGray)),
        footer_area,
    );
}
