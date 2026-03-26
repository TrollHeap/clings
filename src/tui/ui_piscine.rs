//! Vue piscine — rendu Ratatui pour le mode progression linéaire.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Gauge, Paragraph};
use ratatui::Frame;
use ratatui_macros::{span, vertical};

use crate::tui::app::AppState;
use crate::tui::common;

/// Point d'entrée du rendu piscine (appelé par App::run_piscine).
pub fn view(f: &mut Frame, state: &AppState) {
    let area = f.area();

    // Fond global opaque — évite la transparence terminal (Kitty/Alacritty)
    common::render_opaque_background(f, area);

    if state.exercises.is_empty() {
        f.render_widget(
            Paragraph::new("Aucun exercice disponible.").block(Block::bordered()),
            area,
        );
        return;
    }

    // Layout : header (3) | timer (3 si timed) | body (fill) | status (1)
    let timer_constraint = if state.piscine_deadline.is_some() {
        Constraint::Length(3)
    } else {
        Constraint::Length(0)
    };

    let [header_area, timer_area, body_rest, status_area] = Layout::vertical([
        Constraint::Length(3),
        timer_constraint,
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    render_piscine_header(f, header_area, state);

    if state.piscine_deadline.is_some() {
        render_piscine_timer(f, timer_area, state);
    }

    if state.overlay.success_overlay {
        render_piscine_body(f, body_rest, state);
        common::render_success_overlay(f, body_rest);
    } else if state.overlay.list_active {
        common::render_list_overlay(f, body_rest, state);
    } else if state.overlay.vis_active {
        common::render_visualizer_overlay(f, body_rest, state);
    } else if state.overlay.solution_active {
        common::render_solution_overlay(f, body_rest, &state.exercises[state.current_index]);
    } else if state.overlay.search_active {
        common::render_search_overlay(f, body_rest, state);
    } else {
        render_piscine_body(f, body_rest, state);
    }

    render_piscine_status_bar(f, status_area, state);
}

fn render_piscine_header(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let width = area.width as usize;

    // ── L1 : clings ─ piscine  [N/total] Titre   ★diff  subject  [S2] ──
    let stars = common::difficulty_stars(exercise.difficulty);
    let stars_color = common::difficulty_color(exercise.difficulty);
    let stage_badge = state.current_stage.map(common::stage_badge);

    // "clings ─ piscine  " = 18 chars display
    let prefix_len = 18usize;
    let stage_char_len: usize = stage_badge
        .as_ref()
        .map(|s| s.content.chars().count())
        .unwrap_or(0);
    let subj_len = exercise.subject.chars().count();
    let right1_len = 2
        + 5
        + 2
        + subj_len
        + if stage_char_len > 0 {
            2 + stage_char_len
        } else {
            0
        };
    let left1_len = prefix_len + state.header_cache.cached_header_left_len;
    let pad1 = width.saturating_sub(left1_len + right1_len + 2);

    let mut line1_spans: Vec<Span<'_>> = vec![
        span!(Style::default().fg(common::C_ACCENT).add_modifier(Modifier::BOLD); "clings ─ piscine  "),
        Span::styled(
            state.header_cache.cached_exercise_counter.as_str(),
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
        Span::styled(
            exercise.subject.as_str(),
            Style::default().fg(common::C_TEXT_DIM),
        ),
    ];
    if let Some(sb) = stage_badge {
        line1_spans.push(Span::raw("  "));
        line1_spans.push(sb);
    }
    let line1 = Line::from(line1_spans);

    // ── L2 : mini_map  elapsed              ✗ N échec(s) ─────────────────
    let right2 = if state.piscine_fail_count > 0 {
        format!("✗ {} échec(s)", state.piscine_fail_count)
    } else {
        String::new()
    };
    let elapsed = if state.piscine_start.is_some()
        && !state.timer_cache.cached_piscine_elapsed_str.is_empty()
    {
        state.timer_cache.cached_piscine_elapsed_str.as_str()
    } else {
        ""
    };
    let left2_chars = state.header_cache.cached_mini_map_len
        + if elapsed.is_empty() {
            0
        } else {
            2 + elapsed.chars().count()
        };
    let pad2 = if right2.is_empty() {
        0usize
    } else {
        width.saturating_sub(left2_chars + right2.chars().count() + 2)
    };

    let mut line2_spans: Vec<Span<'_>> = vec![Span::styled(
        state.header_cache.cached_mini_map.as_str(),
        Style::default().fg(common::C_TEXT_DIM),
    )];
    if !elapsed.is_empty() {
        line2_spans.push(Span::raw("  "));
        line2_spans.push(Span::styled(
            elapsed,
            Style::default().fg(common::C_TEXT_DIM),
        ));
    }
    if !right2.is_empty() {
        line2_spans.push(Span::raw(" ".repeat(pad2 + 1)));
        line2_spans.push(Span::styled(right2, Style::default().fg(common::C_DANGER)));
    }
    let line2 = Line::from(line2_spans);

    // ── L3 : progression piscine [done/total] ────────────────────────────
    let total = state.exercises.len();
    let done = state.completed.iter().filter(|&&c| c).count();
    let line3 = if done > 0 {
        Line::from(Span::styled(
            format!("✓ {}/{} exercices complétés", done, total),
            Style::default().fg(common::C_SUCCESS),
        ))
    } else {
        Line::raw("")
    };

    f.render_widget(
        Paragraph::new(vec![line1, line2, line3])
            .block(Block::default().style(Style::default().bg(common::C_BG))),
        area,
    );
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

        let label = state.timer_cache.cached_piscine_remaining_str.as_str();

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
    use crate::constants::STATUS_BAR_SPACING;

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
            let mut binds: Vec<(&str, &str)> = Vec::with_capacity(8);
            binds.push(("[r]", " compiler"));
            if has_hints {
                binds.push(("[h]", " indice"));
            }
            binds.push(("[n]", " suivant"));
            binds.push(("[k]", " précédent"));
            if has_vis {
                binds.push(("[v]", " vis"));
            }
            binds.push(("[l]", " liste"));
            binds.push(("[/]", " search"));
            binds.push(("[q]", " quitter"));

            let mut spans = common::render_keybinds(&binds, key_style, dim);

            common::append_hint_counter_if_visible(
                &mut spans,
                " indice",
                state.hint_index,
                exercise.hints.len(),
            );
            Line::from(spans)
        };

    let (right_msg, right_style) = common::render_status_right_piscine(state);
    common::render_split_status_bar(
        f,
        area,
        left_line,
        right_msg,
        right_style,
        STATUS_BAR_SPACING,
    );
}
