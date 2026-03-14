//! Vue watch — rendu Ratatui pour le mode progression SRS.

use std::borrow::Cow;

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use ratatui::Frame;
use ratatui_macros::{line, span, vertical};

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

    // Layout : header (3) | body (fill) | status (1)
    let [header_area, body_area, status_area] = vertical![==3, *=1, ==1].areas(area);

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

    // ── L1 : clings ─ [N/total] Titre   ★diff  TYPE  [S2] ──────────────
    let stars = common::difficulty_stars(exercise.difficulty);
    let stars_color = common::difficulty_color(exercise.difficulty);
    let type_badge = common::exercise_type_badge(exercise.exercise_type.clone());
    let stage_badge = state.current_stage.map(common::stage_badge);

    // "clings ─ " = 9 chars display, cached_header_left_len = counter + title chars
    let prefix_len = 9usize;
    let type_char_len: usize = type_badge.content.chars().count();
    let stage_char_len: usize = stage_badge
        .as_ref()
        .map(|s| s.content.chars().count())
        .unwrap_or(0);
    let right1_len = 2
        + 5
        + 2
        + type_char_len
        + if stage_char_len > 0 {
            2 + stage_char_len
        } else {
            0
        };
    let left1_len = prefix_len + state.cached_header_left_len;
    let pad1 = width.saturating_sub(left1_len + right1_len + 2);

    let mut line1_spans: Vec<Span<'_>> = vec![
        span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "clings ─ "),
        Span::styled(
            state.cached_exercise_counter.as_str(),
            Style::default()
                .fg(common::C_SUCCESS)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            exercise.title.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad1 + 1)),
        Span::styled(stars, Style::default().fg(stars_color)),
        Span::raw("  "),
        type_badge,
    ];
    if let Some(sb) = stage_badge {
        line1_spans.push(Span::raw("  "));
        line1_spans.push(sb);
    }
    let line1 = Line::from(line1_spans);

    // ── L2 : mini_map  subject                  ↻ N révision(s) ─────────
    let due_count = state.due_count();
    let right2 = if due_count > 0 {
        format!("↻ {} révision(s)", due_count)
    } else {
        String::new()
    };
    let left2_chars = state.cached_mini_map_len + 2 + exercise.subject.chars().count();
    let pad2 = if right2.is_empty() {
        0usize
    } else {
        width.saturating_sub(left2_chars + right2.chars().count() + 2)
    };

    let mut line2_spans: Vec<Span<'_>> = vec![
        Span::styled(
            state.cached_mini_map.as_str(),
            Style::default().fg(common::C_TEXT_DIM),
        ),
        Span::raw("  "),
        Span::styled(
            exercise.subject.as_str(),
            Style::default().fg(common::C_TEXT_DIM),
        ),
    ];
    if !right2.is_empty() {
        line2_spans.push(Span::raw(" ".repeat(pad2 + 1)));
        line2_spans.push(Span::styled(right2, Style::default().fg(common::C_WARNING)));
    }
    let line2 = Line::from(line2_spans);

    // ── L3 : Maîtrise ████████░░ 2.4/5.0   key_concept ──────────────────
    let mastery = state
        .mastery_map
        .get(&exercise.subject)
        .copied()
        .unwrap_or(0.0);
    let bar = common::mastery_bar_string(mastery, 10);
    let bar_color = common::mastery_color(mastery);

    let mut line3_spans: Vec<Span<'_>> = vec![
        Span::styled("Maîtrise ", Style::default().fg(common::C_TEXT_DIM)),
        Span::styled(bar, Style::default().fg(bar_color)),
        Span::styled(
            format!(" {:.1}/5.0", mastery),
            Style::default().fg(bar_color),
        ),
    ];
    if let Some(kc) = &exercise.key_concept {
        line3_spans.push(Span::raw("   "));
        line3_spans.push(Span::styled(
            kc.as_str(),
            Style::default().fg(common::C_OVERLAY),
        ));
    }
    let line3 = Line::from(line3_spans);

    f.render_widget(
        Paragraph::new(vec![line1, line2, line3])
            .block(Block::default().style(Style::default().bg(common::C_BG))),
        area,
    );
}

fn render_body(f: &mut Frame, area: Rect, state: &AppState) {
    common::render_body_with_sidebar(f, area, state, render_mastery_sidebar);
}

fn render_mastery_sidebar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let mastery = state
        .mastery_map
        .get(&exercise.subject)
        .copied()
        .unwrap_or(0.0);
    let bar_color = common::mastery_color(mastery);

    let mut lines: Vec<Line<'_>> = Vec::new();

    // ── Section EXERCICE ─────────────────────────────────────────────────
    lines.push(Line::from(Span::styled(
        exercise.title.as_str(),
        Style::default()
            .fg(common::C_TEXT)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::raw(""));
    lines.push(line![
        span!(common::C_TEXT_DIM; "sujet   "),
        Span::styled(
            exercise.subject.as_str(),
            Style::default().fg(common::C_SUBTEXT)
        ),
    ]);

    let diff_line = common::difficulty_stars_line(exercise.difficulty);
    let mut diff_spans: Vec<Span<'_>> = vec![span!(common::C_TEXT_DIM; "diff    ")];
    diff_spans.extend(diff_line.spans);
    lines.push(Line::from(diff_spans));

    lines.push(line![
        span!(common::C_TEXT_DIM; "type    "),
        common::exercise_type_badge(exercise.exercise_type.clone()),
    ]);

    if let Some(stage) = state.current_stage {
        lines.push(line![
            span!(common::C_TEXT_DIM; "étape   "),
            common::stage_badge(stage),
        ]);
    }

    // ── Séparateur ────────────────────────────────────────────────────────
    lines.push(Line::styled(
        common::SEPARATOR,
        Style::default().fg(common::C_OVERLAY),
    ));

    // ── Section SUJET ─────────────────────────────────────────────────────
    let bar = common::mastery_bar_string(mastery, 10);
    lines.push(line![
        span!(Style::default().fg(bar_color); "{}", bar),
        span!(Style::default().fg(bar_color); " {:.1}/5.0", mastery),
    ]);

    // Barre de progression vers le stage suivant
    if let Some((floor, threshold, next_stage)) = common::next_stage_threshold(mastery) {
        let ratio = ((mastery - floor) / (threshold - floor)).clamp(0.0, 1.0);
        let next_bar = common::mastery_bar_string(ratio * 5.0, 8);
        let remaining = threshold - mastery;
        lines.push(line![
            span!(common::C_INFO; "{}", next_bar),
            span!(common::C_TEXT_DIM; " →S{} ({:.1})", next_stage, remaining),
        ]);
    }

    // Révisions dues ou prochaine révision
    let due_count = state.due_count();
    if due_count > 0 {
        lines.push(Line::from(Span::styled(
            format!("↻ {} révision(s) dues", due_count),
            Style::default().fg(common::C_WARNING),
        )));
    } else {
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

    // Erreurs consécutives
    if state.consecutive_failures > 0 {
        lines.push(Line::from(Span::styled(
            format!("✗ {} erreurs consec.", state.consecutive_failures),
            Style::default().fg(common::C_DANGER),
        )));
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(common::C_BORDER))
        .style(Style::default().bg(common::C_BG))
        .title(
            span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "Exercice"),
        );
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
