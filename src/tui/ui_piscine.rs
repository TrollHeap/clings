//! Vue piscine — rendu Ratatui pour le mode progression linéaire.

use std::borrow::Cow;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Gauge, Paragraph};
use ratatui::Frame;
use ratatui_macros::{span, vertical};

use crate::tui::app::AppState;
use crate::tui::common;

/// Point d'entrée du rendu piscine (appelé par App::run_piscine).
pub fn view(f: &mut Frame, state: &AppState) {
    let area = f.area();

    // Fond global opaque — évite la transparence terminal (Kitty/Alacritty)
    f.render_widget(
        Block::default().style(Style::default().bg(common::C_BG)),
        area,
    );

    if state.exercises.is_empty() {
        f.render_widget(
            Paragraph::new("Aucun exercice disponible.").block(Block::bordered()),
            area,
        );
        return;
    }

    // Layout : header (4) | timer (3 si timed) | body (fill) | status (1)
    let timer_constraint = if state.piscine_deadline.is_some() {
        Constraint::Length(3)
    } else {
        Constraint::Length(0)
    };

    let [header_area, timer_area, body_rest, status_area] = Layout::vertical([
        Constraint::Length(4),
        timer_constraint,
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    render_piscine_header(f, header_area, state);

    if state.piscine_deadline.is_some() {
        render_piscine_timer(f, timer_area, state);
    }

    if state.vis_active {
        common::render_visualizer_overlay(f, body_rest, state);
    } else if state.solution_active {
        common::render_solution_overlay(f, body_rest, &state.exercises[state.current_index]);
    } else if state.search_active {
        common::render_search_overlay(f, body_rest, state);
    } else {
        render_piscine_body(f, body_rest, state);
    }

    render_piscine_status_bar(f, status_area, state);
}

fn render_piscine_header(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let width = area.width as usize;

    // Ligne 1 : [idx/total] titre + droit: mini-map
    let line1 = common::render_header_line1(state, exercise, width);

    // Ligne 2 : difficulté | sujet | stage | temps écoulé
    let stars_line = common::difficulty_stars_line(exercise.difficulty);
    let mut meta_spans: Vec<Span> = Vec::new();
    meta_spans.extend(stars_line.spans);
    meta_spans.push(Span::raw("  │  "));
    meta_spans.push(Span::styled(
        exercise.subject.as_str(),
        Style::default().fg(common::C_TEXT_DIM),
    ));

    if let Some(stage) = state.current_stage {
        meta_spans.push(Span::raw("  │  "));
        meta_spans.push(common::stage_badge(stage));
    }

    if state.piscine_start.is_some() && !state.cached_piscine_elapsed_str.is_empty() {
        meta_spans.push(Span::raw("  │  "));
        meta_spans.push(Span::styled(
            state.cached_piscine_elapsed_str.as_str(),
            Style::default().fg(common::C_TEXT_DIM),
        ));
    }

    let line2 = Line::from(meta_spans);

    // Ligne 3 : échecs cumulés si > 0
    let line3 = if state.piscine_fail_count > 0 {
        Line::from(Span::styled(
            format!("✗ {} échec(s) cumulé(s)", state.piscine_fail_count),
            Style::default().fg(common::C_DANGER),
        ))
    } else {
        Line::raw("")
    };

    let text = Text::from(vec![line1, line2, line3]);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(common::C_BORDER))
        .style(Style::default().bg(common::C_BG))
        .title(span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "clings — piscine"));
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn render_piscine_timer(f: &mut Frame, area: Rect, state: &AppState) {
    if let (Some(_start), Some(deadline)) = (state.piscine_start, state.piscine_deadline) {
        let total_secs = state.piscine_timer_total as f64;
        let remaining_secs = (deadline - std::time::Instant::now())
            .as_secs_f64()
            .max(0.0);
        let ratio = if total_secs > 0.0 {
            (remaining_secs / total_secs).min(1.0)
        } else {
            0.0
        };

        let label = state.cached_piscine_remaining_str.as_str();

        let color = if ratio > 0.5 {
            common::C_SUCCESS
        } else if ratio > 0.2 {
            common::C_WARNING
        } else {
            common::C_DANGER
        };

        let gauge = Gauge::default()
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(common::C_BORDER))
                    .title(span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "⏰ Temps")),
            )
            .gauge_style(Style::default().fg(color))
            .ratio(ratio)
            .label(label);

        f.render_widget(gauge, area);
    }
}

fn render_piscine_body(f: &mut Frame, area: Rect, state: &AppState) {
    common::render_body_with_sidebar(f, area, state, render_piscine_sidebar);
}

fn render_piscine_sidebar(f: &mut Frame, area: Rect, state: &AppState) {
    let total = state.exercises.len();
    let done = state.completed.iter().filter(|&&c| c).count();
    let ratio = if total > 0 {
        done as f64 / total as f64
    } else {
        0.0
    };

    let idx = state.current_index;
    let map = common::mini_map(&state.completed, idx);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(common::C_BORDER))
        .style(Style::default().bg(common::C_BG))
        .title(
            span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "Piscine"),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Layout interne : gauge (1) | texte (fill)
    let [gauge_area, text_area] = vertical![==1, *=1].areas(inner);

    // Gauge de progression globale
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(common::C_SUCCESS))
        .style(Style::default().bg(common::C_BG))
        .ratio(ratio)
        .label(format!("{done}/{total}"));
    f.render_widget(gauge, gauge_area);

    // Texte restant : mini-map, timer, échecs
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        map,
        Style::default().fg(common::C_OVERLAY),
    )));
    lines.push(Line::raw(""));

    // Timer restant si timed
    if let Some(deadline) = state.piscine_deadline {
        let remaining = (deadline - std::time::Instant::now())
            .as_secs_f64()
            .max(0.0) as u64;
        let timer_str = if remaining >= 60 {
            format!("⏱ {}m{:02}s restant", remaining / 60, remaining % 60)
        } else {
            format!("⏱ {}s restant", remaining)
        };
        let color = if remaining > 300 {
            common::C_SUCCESS
        } else if remaining > 60 {
            common::C_WARNING
        } else {
            common::C_DANGER
        };
        lines.push(Line::from(Span::styled(
            timer_str,
            Style::default().fg(color),
        )));
        lines.push(Line::raw(""));
    }

    // Échecs cumulés
    if state.piscine_fail_count > 0 {
        lines.push(Line::from(Span::styled(
            format!("✗ {} échec(s)", state.piscine_fail_count),
            Style::default().fg(common::C_DANGER),
        )));
    }

    f.render_widget(Paragraph::new(lines), text_area);
}

fn render_piscine_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let has_vis = !exercise.visualizer.steps.is_empty();

    let has_hints = !exercise.hints.is_empty();

    let dim = Style::default().fg(common::C_TEXT_DIM);
    let key_style = Style::default()
        .fg(common::C_ACCENT)
        .add_modifier(Modifier::BOLD);

    let left_line: Line<'static> =
        if let Some(prefix) = common::status_bar_prefix_line(state, false) {
            prefix
        } else {
            let mut spans: Vec<Span<'static>> = Vec::new();

            macro_rules! push_key {
                ($key:literal, $desc:literal) => {
                    if !spans.is_empty() {
                        spans.push(Span::raw("  "));
                    }
                    spans.push(Span::styled($key, key_style));
                    spans.push(Span::styled($desc, dim));
                };
            }

            push_key!("[r]", " compiler");
            if has_hints {
                spans.push(Span::raw("  "));
                let hint_rest: Cow<'static, str> = if state.hint_index == 0 {
                    Cow::Borrowed(" indice")
                } else {
                    Cow::Owned(format!(
                        " indice ({}/{})",
                        state.hint_index,
                        exercise.hints.len()
                    ))
                };
                spans.push(Span::styled("[h]", key_style));
                spans.push(Span::styled(hint_rest, dim));
            }
            push_key!("[n]", " suivant");
            push_key!("[k]", " précédent");
            if has_vis {
                push_key!("[v]", " vis");
            }
            push_key!("[/]", " search");
            push_key!("[q]", " quitter");
            Line::from(spans)
        };

    // Droite : échecs cumulés
    let right_msg = if state.piscine_fail_count > 0 {
        format!("✗ {}", state.piscine_fail_count)
    } else {
        String::new()
    };

    common::render_split_status_bar(
        f,
        area,
        left_line,
        right_msg,
        Style::default().fg(common::C_DANGER),
        10,
    );
}
