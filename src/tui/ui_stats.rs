//! Vue Ratatui pour statistiques — mastery moyenne, sparkline, table par sujet.

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Bar, BarChart, BarGroup, Block, BorderType, Paragraph, Row, Sparkline, Table, TableState,
};
use ratatui_macros::{line, span, vertical};
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
        span!(Style::default().fg(color); "{}", bar),
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

    // Fond global opaque — évite la transparence terminal
    f.render_widget(
        Block::default().style(Style::default().bg(common::C_BG)),
        area,
    );

    // Layout: header (3) | body (fill) | footer (1)
    let [header_area, body_area, footer_area] = vertical![==3, *=1, ==1].areas(area);

    // ── Header ────────────────────────────────────────────────────────
    let header_text = line![
        span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "clings — Statistiques")
    ];
    f.render_widget(
        Paragraph::new(header_text).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(common::C_BORDER))
                .style(Style::default().bg(common::C_BG))
                .title("Stats"),
        ),
        header_area,
    );

    // ── Body ──────────────────────────────────────────────────────────
    if subjects.is_empty() {
        let text = "Aucun sujet pratiqué pour l'instant.";
        f.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(common::C_OVERLAY))
                .block(
                    Block::bordered()
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(common::C_BORDER))
                        .style(Style::default().bg(common::C_BG)),
                ),
            body_area,
        );
    } else {
        // Build content lines
        let mut lines: Vec<Line> = Vec::new();

        // Streak
        lines.push(line![
            span!(common::C_ACCENT; "Série: "),
            span!(Style::default().fg(common::C_YELLOW).add_modifier(Modifier::BOLD); "{}", streak),
            Span::raw("  jours consécutifs"),
        ]);

        let avg = avg_mastery(subjects);

        // Average mastery
        let mut avg_line = vec![span!(common::C_ACCENT; "Maîtrise moyenne: ")];
        avg_line.extend(mastery_bar_spans(avg));
        lines.push(Line::from(avg_line));
        lines.push(Line::raw(""));

        // Determine which section to show
        let content_paragraph = Paragraph::new(lines).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(common::C_BORDER))
                .style(Style::default().bg(common::C_BG)),
        );

        // Sparkline for daily activity
        if let Some(daily_data) = daily {
            if !daily_data.is_empty() {
                let counts: Vec<u64> = daily_data.iter().map(|(_, c)| *c as u64).collect();
                let total_attempts: u32 = daily_data.iter().map(|(_, c)| c).sum();

                // Layout for sparkline + stats below
                let [para_area, spark_area] = vertical![==4, *=1].areas(body_area);

                f.render_widget(content_paragraph, para_area);

                let sparkline = Sparkline::default()
                    .block(
                        Block::bordered()
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(common::C_BORDER))
                            .style(Style::default().bg(common::C_BG))
                            .title(format!("Activité 30j — {} tentatives", total_attempts)),
                    )
                    .data(&counts)
                    .bar_set(ratatui::symbols::bar::NINE_LEVELS)
                    .style(Style::default().fg(common::C_YELLOW))
                    .absent_value_style(Style::default().fg(common::C_OVERLAY));

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
                            .fg(common::C_ACCENT)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                )
                .block(
                    Block::bordered()
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(common::C_BORDER))
                        .style(Style::default().bg(common::C_BG))
                        .title("Tentatives par sujet"),
                );

                f.render_widget(table, body_area);
                return;
            }
        }

        // Mastery BarChart (default) — horizontal, 1 barre par sujet
        let mut sorted: Vec<&Subject> = subjects.iter().collect();
        sorted.sort_by(|a, b| {
            b.mastery_score
                .get()
                .partial_cmp(&a.mastery_score.get())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let [para_area, chart_area] = vertical![==4, *=1].areas(body_area);
        f.render_widget(content_paragraph, para_area);

        // Build labels first (owned Strings) then borrow into Bar
        let label_strings: Vec<String> = sorted
            .iter()
            .take(10)
            .map(|s| {
                if s.name.len() > 10 {
                    s.name[..10].to_string()
                } else {
                    s.name.clone()
                }
            })
            .collect();

        let bars: Vec<Bar<'_>> = sorted
            .iter()
            .take(10)
            .zip(label_strings.iter())
            .map(|(s, label)| {
                let score = s.mastery_score.get();
                Bar::default()
                    .value((score * 10.0) as u64)
                    .style(Style::default().fg(common::mastery_color(score)))
                    .text_value(format!("{:.1}", score))
                    .label(Line::from(label.as_str()))
            })
            .collect();

        let group = BarGroup::default().bars(&bars);
        let bar_chart = BarChart::default()
            .direction(Direction::Horizontal)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(common::C_BORDER))
                    .style(Style::default().bg(common::C_BG))
                    .title("Maîtrise par sujet"),
            )
            .bar_width(1)
            .bar_gap(0)
            .max(50)
            .data(group);

        f.render_widget(bar_chart, chart_area);
        let _ = table_state; // unused in this branch
    }

    // ── Footer ────────────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new("[q] quitter").style(Style::default().fg(common::C_OVERLAY)),
        footer_area,
    );
}
