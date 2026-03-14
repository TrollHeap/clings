//! Vue watch — rendu Ratatui pour le mode progression SRS.

use std::borrow::Cow;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph};
use ratatui::Frame;
use ratatui_macros::{line, span};

use crate::tui::app::AppState;
use crate::tui::common;

/// Point d'entrée du rendu watch (appelé par App::run_watch).
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

    // Layout : header (4) | body (fill) | status (1)
    let [header_area, body_area, status_area] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    render_header(f, header_area, state);

    if state.help_active {
        common::render_help_overlay(f, body_area);
    } else if state.vis_active {
        common::render_visualizer_overlay(f, body_area, state);
    } else if state.solution_active {
        common::render_solution_overlay(f, body_area, &state.exercises[state.current_index]);
    } else if state.search_active {
        common::render_search_overlay(f, body_area, state);
    } else {
        render_body(f, body_area, state);
    }

    render_status_bar(f, status_area, state);
}

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let width = area.width as usize;

    // Mastery du sujet courant
    let mastery = state
        .mastery_map
        .get(&exercise.subject)
        .copied()
        .unwrap_or(0.0);
    let bar = common::mastery_bar_string(mastery, 10);
    let bar_color = common::mastery_color(mastery);

    // ── Ligne 1 : [idx/total] Titre ── + droit: chapter mini-map ──────
    let line1 = common::render_header_line1(state, exercise, width);

    // ── Ligne 2 : stars | type | stage ── + droit: mastery bar ────────
    let stars_line = common::difficulty_stars_line(exercise.difficulty);
    let mut meta_spans: Vec<Span> = Vec::new();
    meta_spans.extend(stars_line.spans);
    meta_spans.push(Span::raw("  │  "));
    meta_spans.push(common::exercise_type_badge(exercise.exercise_type.clone()));
    if let Some(stage) = state.current_stage {
        meta_spans.push(Span::raw("  │  "));
        meta_spans.push(common::stage_badge(stage));
    }

    // "mastery: X.X  " (14 chars fixes, mastery ∈ [0.0,5.0] → toujours 1 chiffre) + 10 barre
    let right2_display = 14 + 10;
    let left2_display: usize = meta_spans
        .iter()
        .map(|s| s.content.chars().count())
        .sum::<usize>();
    let pad2 = width.saturating_sub(left2_display + right2_display + 4);
    meta_spans.push(Span::raw(" ".repeat(pad2 + 1)));
    meta_spans.push(Span::styled(
        state.cached_mastery_display.as_str(),
        Style::default().fg(bar_color),
    ));
    meta_spans.push(Span::styled(bar, Style::default().fg(bar_color)));
    let line2 = Line::from(meta_spans);

    // ── Ligne 3 : révision due (optionnelle) ──────────────────────────
    let due_count = state.due_count();
    let line3 = if due_count > 0 {
        Line::from(Span::styled(
            format!("↻ {} révision(s) due(s)", due_count),
            Style::default().fg(common::C_WARNING),
        ))
    } else {
        Line::raw("")
    };

    let text = Text::from(vec![line1, line2, line3]);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(common::C_BORDER))
        .style(Style::default().bg(common::C_BG))
        .title(span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "clings — watch"));
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn render_body(f: &mut Frame, area: Rect, state: &AppState) {
    common::render_body_with_sidebar(f, area, state, render_mastery_sidebar);
}

fn render_mastery_sidebar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];

    // Priorité au sujet courant puis les 7 premiers depuis le cache
    let top: Vec<&String> = {
        let mut result: Vec<&String> = state.subject_order.iter().take(8).collect();
        if !result.contains(&&exercise.subject) {
            result.insert(0, &exercise.subject);
            result.truncate(8);
        }
        result
    };

    let mut lines: Vec<Line> = Vec::new();

    for subj in &top {
        let score = state.mastery_map.get(*subj).copied().unwrap_or(0.0);
        let bar = common::mastery_bar_string(score, 8);
        let bar_color = common::mastery_color(score);
        // Tronque le nom à 9 chars pour tenir dans 26 cols (2 indicateur + 9 nom + 1 espace + 8 barre + 3 score)
        let short_name = if subj.len() > 9 {
            &subj[..9]
        } else {
            subj.as_str()
        };
        let is_current = *subj == &exercise.subject;
        let (indicator, name_style, score_color) = if is_current {
            (
                "▶ ",
                Style::default()
                    .fg(common::C_ACCENT)
                    .add_modifier(Modifier::BOLD),
                common::C_ACCENT,
            )
        } else {
            (
                "  ",
                Style::default().fg(common::C_TEXT_DIM),
                common::C_TEXT_DIM,
            )
        };
        lines.push(line![
            Span::styled(indicator, name_style),
            span!(name_style; "{:<8}", short_name),
            Span::raw(" "),
            span!(Style::default().fg(bar_color); "{}", bar),
            span!(Style::default().fg(score_color); " {:.1}", score),
        ]);
    }

    // Séparateur
    lines.push(Line::raw(""));

    // Failures consécutives
    if state.consecutive_failures > 0 {
        lines.push(Line::from(Span::styled(
            format!("✗ {} erreurs consec.", state.consecutive_failures),
            Style::default().fg(common::C_DANGER),
        )));
    }

    // Révisions dues
    let due_count = state.due_count();
    if due_count > 0 {
        lines.push(Line::from(Span::styled(
            format!("↻ {} révision(s)", due_count),
            Style::default().fg(common::C_WARNING),
        )));
    }

    // Barre de progression vers le stage suivant
    let mastery_score = state
        .mastery_map
        .get(&exercise.subject)
        .copied()
        .unwrap_or(0.0);
    if let Some((floor, threshold, next_stage)) = common::next_stage_threshold(mastery_score) {
        let ratio = ((mastery_score - floor) / (threshold - floor)).clamp(0.0, 1.0);
        let bar = common::mastery_bar_string(ratio * 5.0, 8);
        let remaining = threshold - mastery_score;
        lines.push(Line::raw(""));
        lines.push(line![
            span!(common::C_INFO; "{}", bar),
            span!(common::C_TEXT_DIM; " →S{} ({:.1})", next_stage, remaining),
        ]);
    }

    // Prochaine révision (si aucune n'est due maintenant)
    if due_count == 0 {
        let next_due_days = state
            .review_map
            .values()
            .filter_map(|v| *v)
            .filter(|&d| d > 0)
            .min();
        if let Some(days) = next_due_days {
            lines.push(Line::from(Span::styled(
                format!("↻ dans {}j", days),
                Style::default().fg(common::C_TEXT_DIM),
            )));
        }
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(common::C_BORDER))
        .style(Style::default().bg(common::C_BG))
        .title(span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "Progression"));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let has_vis = !exercise.visualizer.steps.is_empty();
    let has_hints = !exercise.hints.is_empty();

    let dim = Style::default().fg(common::C_TEXT_DIM);
    let key_style = Style::default()
        .fg(common::C_ACCENT)
        .add_modifier(Modifier::BOLD);

    let left_line: Line<'static> = if let Some(prefix) = common::status_bar_prefix_line(state, true)
    {
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

        if has_hints {
            if !spans.is_empty() {
                spans.push(Span::raw("  "));
            }
            let hint_rest: Cow<'static, str> = if state.hint_index == 0 {
                Cow::Borrowed(" hint")
            } else {
                Cow::Owned(format!(
                    " hint ({}/{})",
                    state.hint_index,
                    exercise.hints.len()
                ))
            };
            spans.push(Span::styled("[h]", key_style));
            spans.push(Span::styled(hint_rest, dim));
        }
        push_key!("[j]", " suiv");
        push_key!("[k]", " préc");
        push_key!("[n]", " skip");
        push_key!("[r]", " run");
        if has_vis {
            push_key!("[v]", " vis");
        }
        push_key!("[/]", " search");
        push_key!("[?]", " aide");
        push_key!("[q]", " quit");
        Line::from(spans)
    };

    // Droite : failures ou révision
    let (right_msg, right_style) = if state.consecutive_failures > 0 {
        (
            format!("✗ {}", state.consecutive_failures),
            Style::default().fg(common::C_DANGER),
        )
    } else {
        let due = state.due_count();
        if due > 0 {
            (
                format!("révision: {}j", due),
                Style::default().fg(common::C_WARNING),
            )
        } else {
            (String::new(), Style::default())
        }
    };

    common::render_split_status_bar(f, area, left_line, right_msg, right_style, 15);
}
