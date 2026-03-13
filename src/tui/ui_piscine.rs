//! Vue piscine — rendu Ratatui pour le mode progression linéaire.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Gauge, Paragraph};
use ratatui::Frame;

use crate::tui::app::AppState;
use crate::tui::common;

/// Point d'entrée du rendu piscine (appelé par App::run_piscine).
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
    let total = state.exercises.len();
    let idx = state.current_index;
    let width = area.width as usize;

    let map = common::mini_map(&state.completed, idx);

    // Ligne 1 : [idx/total] titre + droit: mini-map
    let pad1 = {
        let left_len = format!("[{}/{}] {}", idx + 1, total, exercise.title)
            .chars()
            .count();
        // chars().count() pour ●◉○ (3 octets chacun, 1 col d'affichage)
        let right_len = map.chars().count() + exercise.subject.chars().count() + 2;
        width.saturating_sub(left_len + right_len + 4)
    };
    let line1 = Line::from(vec![
        Span::styled(
            format!("[{}/{}] ", idx + 1, total),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            exercise.title.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad1 + 1)),
        Span::styled(map, Style::default().fg(Color::Gray)),
        Span::raw("  "),
        Span::styled(exercise.subject.as_str(), Style::default().fg(Color::Gray)),
    ]);

    // Ligne 2 : difficulté | sujet | stage | temps écoulé
    let stars = common::difficulty_stars(exercise.difficulty);
    let diff_color = common::difficulty_color(exercise.difficulty);
    let mut meta_spans = vec![
        Span::styled(stars, Style::default().fg(diff_color)),
        Span::raw("  │  "),
        Span::styled(exercise.subject.as_str(), Style::default().fg(Color::Gray)),
    ];

    if let Some(stage) = state.current_stage {
        meta_spans.push(Span::raw("  │  "));
        meta_spans.push(Span::styled(
            common::stage_label(stage),
            Style::default().fg(Color::Gray),
        ));
    }

    if let Some(start) = state.piscine_start {
        let elapsed = start.elapsed().as_secs();
        let elapsed_str = format!("⏱ {}m{:02}s", elapsed / 60, elapsed % 60);
        meta_spans.push(Span::raw("  │  "));
        meta_spans.push(Span::styled(elapsed_str, Style::default().fg(Color::Gray)));
    }

    let line2 = Line::from(meta_spans);

    // Ligne 3 : échecs cumulés si > 0
    let line3 = if state.piscine_fail_count > 0 {
        Line::from(Span::styled(
            format!("✗ {} échec(s) cumulé(s)", state.piscine_fail_count),
            Style::default().fg(Color::Red),
        ))
    } else {
        Line::raw("")
    };

    let text = Text::from(vec![line1, line2, line3]);
    let block = Block::bordered().title("clings — piscine");
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

        let label = if remaining_secs <= 0.0 {
            "Temps écoulé".to_string()
        } else {
            let secs = remaining_secs as u64;
            if secs >= 60 {
                format!("{}m{:02}s restantes", secs / 60, secs % 60)
            } else {
                format!("{}s restantes", secs)
            }
        };

        let color = if ratio > 0.5 {
            Color::Green
        } else if ratio > 0.2 {
            Color::Yellow
        } else {
            Color::Red
        };

        let gauge = Gauge::default()
            .block(Block::bordered().title("⏰ Temps"))
            .gauge_style(Style::default().fg(color))
            .ratio(ratio)
            .label(label);

        f.render_widget(gauge, area);
    }
}

fn render_piscine_body(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];

    // Layout body : [left | right sidebar (si width >= BODY_SIDEBAR_THRESHOLD)]
    let (content_area, sidebar_opt) = if area.width >= common::BODY_SIDEBAR_THRESHOLD {
        let [left, right] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(common::SIDEBAR_WIDTH),
        ])
        .areas(area);
        (left, Some(right))
    } else {
        (area, None)
    };

    // Layout contenu : description (fill) | result (hauteur dynamique si présent)
    let (desc_area, result_area_opt) = if let Some(result) = &state.run_result {
        let h = common::run_result_height(result);
        let [desc, res] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(h)]).areas(content_area);
        (desc, Some(res))
    } else {
        (content_area, None)
    };

    common::render_description_panel(f, desc_area, state);

    if let Some(result_area) = result_area_opt {
        if let Some(result) = &state.run_result {
            common::render_run_result(f, result_area, result, exercise);
        }
    }

    // ── Sidebar piscine ──────────────────────────────────────────────────
    if let Some(sb_area) = sidebar_opt {
        render_piscine_sidebar(f, sb_area, state);
    }
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

    let mut lines: Vec<Line> = Vec::new();

    // Barre de progression globale
    let bar_width = 20usize;
    let filled = (ratio * bar_width as f64).round() as usize;
    let progress_bar = format!(
        "[{}{}] {}/{}",
        "█".repeat(filled),
        "░".repeat(bar_width - filled),
        done,
        total
    );
    lines.push(Line::from(Span::styled(
        progress_bar,
        Style::default().fg(Color::Green),
    )));
    lines.push(Line::raw(""));

    // Mini-map du chapitre courant
    lines.push(Line::from(Span::styled(
        map,
        Style::default().fg(Color::DarkGray),
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
            Color::Green
        } else if remaining > 60 {
            Color::Yellow
        } else {
            Color::Red
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
            Style::default().fg(Color::Red),
        )));
    }

    let block = Block::bordered().title("Piscine");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_piscine_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let has_vis = !exercise.visualizer.steps.is_empty();

    let has_hints = !exercise.hints.is_empty();
    let left_msg = if state.compile_pending {
        "⏳ Compilation en cours…".to_string()
    } else if state.solution_active {
        "[Esc/s] fermer solution".to_string()
    } else if state.search_active {
        "[↑↓/jk] nav  [Entrée] aller  [Esc] fermer".to_string()
    } else if let Some(status) = &state.status_msg {
        status.as_str().to_string()
    } else {
        let mut parts = vec![
            "[r] compiler".to_string(),
            "[n] suivant".to_string(),
            "[k] précédent".to_string(),
        ];
        if has_hints {
            let hint_label = if state.hint_index == 0 {
                "[h] indice".to_string()
            } else {
                format!("[h] indice ({}/{})", state.hint_index, exercise.hints.len())
            };
            parts.insert(1, hint_label);
        }
        if has_vis {
            parts.push("[v] vis".to_string());
        }
        parts.push("[/] search".to_string());
        parts.push("[q] quitter".to_string());
        parts.join("  ")
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
        left_msg,
        right_msg,
        Style::default().fg(Color::Red),
        10,
    );
}
