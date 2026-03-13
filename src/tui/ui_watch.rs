//! Vue watch — rendu Ratatui pour le mode progression SRS.

use std::borrow::Cow;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::tui::app::AppState;
use crate::tui::common;

/// Point d'entrée du rendu watch (appelé par App::run_watch).
pub fn view(f: &mut Frame, state: &AppState) {
    let area = f.area();

    // Fond global opaque — évite la transparence terminal (Kitty/Alacritty)
    f.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
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
    // chars().count() pour la largeur d'affichage (●◉○ = 3 octets mais 1 col)
    let right1_display = state.cached_mini_map_len + 2 + exercise.subject.chars().count();
    let pad1 = width.saturating_sub(state.cached_header_left_len + right1_display + 4);
    let line1 = Line::from(vec![
        Span::styled(
            state.cached_exercise_counter.as_str(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            exercise.title.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad1 + 1)),
        Span::styled(
            state.cached_mini_map.as_str(),
            Style::default().fg(Color::Gray),
        ),
        Span::raw("  "),
        Span::styled(exercise.subject.as_str(), Style::default().fg(Color::Gray)),
    ]);

    // ── Ligne 2 : stars | type | stage ── + droit: mastery bar ────────
    let stars = common::difficulty_stars(exercise.difficulty);
    let diff_color = common::difficulty_color(exercise.difficulty);
    let mut meta_spans: Vec<Span> = vec![
        Span::styled(stars, Style::default().fg(diff_color)),
        Span::raw("  │  "),
        Span::styled(
            exercise.exercise_type.to_string(),
            Style::default().fg(Color::Gray),
        ),
    ];
    if let Some(stage) = state.current_stage {
        meta_spans.push(Span::raw("  │  "));
        meta_spans.push(Span::styled(
            common::stage_label(stage),
            Style::default().fg(Color::Gray),
        ));
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
            Style::default().fg(Color::Yellow),
        ))
    } else {
        Line::raw("")
    };

    let text = Text::from(vec![line1, line2, line3]);
    let block = Block::bordered().title("clings — watch");
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
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
                Color::Magenta,
            )
        } else {
            ("  ", Style::default().fg(Color::DarkGray), Color::DarkGray)
        };
        lines.push(Line::from(vec![
            Span::styled(indicator, name_style),
            Span::styled(format!("{:<8}", short_name), name_style),
            Span::raw(" "),
            Span::styled(bar, Style::default().fg(bar_color)),
            Span::styled(format!(" {:.1}", score), Style::default().fg(score_color)),
        ]));
    }

    // Séparateur
    lines.push(Line::raw(""));

    // Failures consécutives
    if state.consecutive_failures > 0 {
        lines.push(Line::from(Span::styled(
            format!("✗ {} erreurs consec.", state.consecutive_failures),
            Style::default().fg(Color::Red),
        )));
    }

    // Révisions dues
    let due_count = state.due_count();
    if due_count > 0 {
        lines.push(Line::from(Span::styled(
            format!("↻ {} révision(s)", due_count),
            Style::default().fg(Color::Yellow),
        )));
    }

    let block = Block::bordered().title("Progression");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let has_vis = !exercise.visualizer.steps.is_empty();

    let has_hints = !exercise.hints.is_empty();
    let left_msg = if state.compile_pending {
        "⏳ Compilation en cours…".to_string()
    } else if state.help_active {
        "[Esc/?] fermer".to_string()
    } else if state.solution_active {
        "[Esc/s] fermer solution".to_string()
    } else if state.search_active {
        "[↑↓/jk] nav  [Entrée] aller  [Esc] fermer".to_string()
    } else if let Some(status) = &state.status_msg {
        status.as_str().to_string()
    } else {
        let mut parts: Vec<Cow<'static, str>> = vec![
            Cow::Borrowed("[j] suiv"),
            Cow::Borrowed("[k] préc"),
            Cow::Borrowed("[n] skip"),
            Cow::Borrowed("[r] run"),
        ];
        if has_hints {
            let hint_label: Cow<'static, str> = if state.hint_index == 0 {
                Cow::Borrowed("[h] hint")
            } else {
                Cow::Owned(format!(
                    "[h] hint ({}/{})",
                    state.hint_index,
                    exercise.hints.len()
                ))
            };
            parts.insert(0, hint_label);
        }
        if has_vis {
            parts.push(Cow::Borrowed("[v] vis"));
        }
        parts.push(Cow::Borrowed("[/] search"));
        parts.push(Cow::Borrowed("[?] aide"));
        parts.push(Cow::Borrowed("[q] quit"));
        parts.join("  ")
    };

    // Droite : failures ou révision
    let (right_msg, right_style) = if state.consecutive_failures > 0 {
        (
            format!("✗ {}", state.consecutive_failures),
            Style::default().fg(Color::Red),
        )
    } else {
        let due = state.due_count();
        if due > 0 {
            (
                format!("révision: {}j", due),
                Style::default().fg(Color::Yellow),
            )
        } else {
            (String::new(), Style::default())
        }
    };

    common::render_split_status_bar(f, area, left_msg, right_msg, right_style, 15);
}
