//! Vue watch — rendu Ratatui pour le mode progression SRS.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use ratatui_macros::{line, span, vertical};

use crate::tui::app::AppState;
use crate::tui::common;

/// Point d'entrée du rendu watch (appelé par App::run_watch).
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

    // Layout : header (6) | body (fill) | status (1)
    let [header_area, body_area, status_area] = vertical![==6, *=1, ==1].areas(area);

    render_header(f, header_area, state);

    if state.overlay.success_overlay {
        render_body(f, body_area, state);
        common::render_success_overlay(f, body_area);
    } else if state.overlay.help_active {
        common::render_help_overlay(f, body_area);
    } else if state.overlay.list_active {
        common::render_list_overlay(f, body_area, state);
    } else if state.overlay.vis_active {
        common::render_visualizer_overlay(f, body_area, state);
    } else if state.overlay.solution_active {
        common::render_solution_overlay(f, body_area, &state.exercises[state.current_index]);
    } else if state.overlay.search_active {
        common::render_search_overlay(f, body_area, state);
    } else {
        render_body(f, body_area, state);
    }

    // Nav confirm s'affiche par-dessus tout (y compris les autres overlays).
    if state.overlay.nav_confirm_active {
        common::render_nav_confirm_overlay(f, body_area, state.overlay.nav_confirm_next);
    }

    render_status_bar(f, status_area, state);
}

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let width = area.width as usize;

    let stars = common::difficulty_stars(exercise.difficulty);
    let stars_color = common::difficulty_color(exercise.difficulty);
    let type_badge = common::exercise_type_badge(exercise.exercise_type);
    let stage_badge = state.current_stage.map(common::stage_badge);

    // ── L1 : Titre (plein gauche)   TYPE_BADGE (droit) ───────────────────
    let title_len = exercise.title.chars().count();
    let type_char_len = type_badge.content.chars().count();
    let pad1 = width.saturating_sub(title_len + type_char_len + 1);

    let line1 = Line::from(vec![
        Span::styled(
            exercise.title.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad1)),
        type_badge,
    ]);

    // ── L2 : clings · N/total · subject · ★★★★★ · [S0]   ↻ N révision(s) ─
    let due_count = state.due_count();
    let counter_str = format!("{}/{}", state.current_index + 1, state.exercises.len());
    let right2 = if due_count > 0 {
        format!("↻ {} révision(s)", due_count)
    } else {
        String::new()
    };

    // Compute left2 display length for padding
    let stage_display_len = stage_badge
        .as_ref()
        .map(|sb| 3 + sb.content.chars().count()) // " · [S0]"
        .unwrap_or(0);
    let version_tag = concat!(" v", env!("CARGO_PKG_VERSION"));
    let left2_len = 6 // "clings"
        + version_tag.len()
        + 3 + counter_str.len()       // " · 1/61"
        + 3 + exercise.subject.chars().count() // " · pointers"
        + 3 + 5                        // " · ★★★★★"
        + stage_display_len;
    let pad2 = if right2.is_empty() {
        0usize
    } else {
        width.saturating_sub(left2_len + right2.chars().count() + 1)
    };

    let mut line2_spans: Vec<Span<'_>> = vec![
        Span::styled(
            format!("clings v{}", env!("CARGO_PKG_VERSION")),
            Style::default()
                .fg(common::C_MAUVE)
                .add_modifier(Modifier::BOLD),
        ),
        span!(Style::default().fg(common::C_OVERLAY); " · "),
        span!(Style::default().fg(common::C_SUCCESS); "{}", counter_str),
        span!(Style::default().fg(common::C_OVERLAY); " · "),
        Span::styled(
            exercise.subject.as_str(),
            Style::default().fg(common::C_SUBTEXT),
        ),
        span!(Style::default().fg(common::C_OVERLAY); " · "),
        Span::styled(stars, Style::default().fg(stars_color)),
    ];
    if let Some(sb) = stage_badge {
        line2_spans.push(span!(Style::default().fg(common::C_OVERLAY); " · "));
        line2_spans.push(sb);
    }
    if !right2.is_empty() {
        line2_spans.push(Span::raw(" ".repeat(pad2 + 1)));
        line2_spans.push(Span::styled(right2, Style::default().fg(common::C_WARNING)));
    }
    let line2 = Line::from(line2_spans);

    // ── L3 : ██████████ N.N/5.0  —  key_concept ─────────────────────────
    let mastery = state
        .mastery_map
        .get(&exercise.subject)
        .copied()
        .unwrap_or(0.0);
    let bar = common::mastery_bar_string(mastery, 10);
    let bar_color = common::mastery_color(mastery);

    let mut line3_spans: Vec<Span<'_>> = vec![
        Span::styled(bar, Style::default().fg(bar_color)),
        Span::styled(
            format!(" {:.1}/5.0", mastery),
            Style::default().fg(bar_color),
        ),
    ];
    if let Some(kc) = &exercise.key_concept {
        line3_spans.push(span!(Style::default().fg(common::C_OVERLAY); "  —  "));
        line3_spans.push(Span::styled(
            kc.as_str(),
            Style::default().fg(common::C_OVERLAY),
        ));
    }
    let line3 = Line::from(line3_spans);

    f.render_widget(
        Paragraph::new(vec![line1, Line::from(""), line2, Line::from(""), line3]).block(
            Block::default()
                .style(Style::default().bg(common::C_BG))
                .borders(Borders::BOTTOM)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(common::C_BORDER)),
        ),
        area,
    );
}

fn render_body(f: &mut Frame, area: Rect, state: &AppState) {
    common::render_body_with_sidebar(f, area, state, render_mastery_sidebar);
}

/// Ligne de progression de stage : S0 ── S1 ── [S2] ── S3 ── S4
/// Stage courant en accent+bold, stages passés en vert, futurs en gris.
fn stage_progress_line(current: Option<u8>) -> Line<'static> {
    let sep = Span::styled(" ── ", Style::default().fg(common::C_OVERLAY));
    let spans: Vec<Span<'static>> = (0u8..=4)
        .flat_map(|s| {
            let label = match current {
                Some(c) if s == c => Span::styled(
                    format!("[S{}]", s),
                    Style::default()
                        .fg(common::C_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Some(c) if s < c => {
                    Span::styled(format!("S{}", s), Style::default().fg(common::C_SUCCESS))
                }
                _ => Span::styled(format!("S{}", s), Style::default().fg(common::C_BORDER)),
            };
            if s < 4 {
                vec![label, sep.clone()]
            } else {
                vec![label]
            }
        })
        .collect();
    Line::from(spans)
}

fn render_mastery_sidebar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let mastery = state
        .mastery_map
        .get(&exercise.subject)
        .copied()
        .unwrap_or(0.0);
    let bar_color = common::mastery_color(mastery);

    // ── Section EXERCICE + séparateur ────────────────────────────────────
    let title_line = Line::from(Span::styled(
        exercise.title.as_str(),
        Style::default()
            .fg(common::C_TEXT)
            .add_modifier(Modifier::BOLD),
    ));
    let sep_line = Line::styled(common::SEPARATOR, Style::default().fg(common::C_OVERLAY));
    let mut lines: Vec<Line<'_>> = vec![
        title_line,
        Line::raw(""),
        stage_progress_line(state.current_stage),
        sep_line,
    ];

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

    // Badge visualizer — met en évidence si l'exercice a des étapes visuelles
    if !exercise.visualizer.steps.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("[ ", Style::default().fg(common::C_TEXT_DIM)),
            Span::styled(
                "vis ",
                Style::default()
                    .fg(common::C_INFO)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} étapes", exercise.visualizer.steps.len()),
                Style::default().fg(common::C_SUBTEXT),
            ),
            Span::styled(" ]", Style::default().fg(common::C_TEXT_DIM)),
        ]));
        lines.push(Line::from(Span::styled(
            "  → [v] pour visualiser",
            Style::default()
                .fg(common::C_TEXT_DIM)
                .add_modifier(Modifier::ITALIC),
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
    use crate::constants::STATUS_BAR_KEY_MIN_WIDTH;

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
        let mut binds: Vec<(&str, &str)> = Vec::with_capacity(10);
        if has_hints {
            binds.push(("[h]", " hint"));
        }
        binds.push(("[j]", " suiv"));
        binds.push(("[k]", " préc"));
        binds.push(("[n]", " skip"));
        binds.push(("[r]", " run"));
        if has_vis {
            binds.push(("[v]", " vis"));
        }
        binds.push(("[l]", " list"));
        binds.push(("[/]", " search"));
        binds.push(("[?]", " aide"));
        binds.push(("[q]", " quit"));

        let mut spans = common::render_keybinds(&binds, key_style, dim);

        common::append_hint_counter_if_visible(
            &mut spans,
            " hint",
            state.hint_index,
            exercise.hints.len(),
        );
        Line::from(spans)
    };

    let (right_msg, right_style) = common::render_status_right_watch(state);
    common::render_split_status_bar(
        f,
        area,
        left_line,
        right_msg,
        right_style,
        STATUS_BAR_KEY_MIN_WIDTH,
    );
}
