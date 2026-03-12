//! Vue piscine — rendu Ratatui pour le mode progression linéaire.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use crate::models::{Difficulty, ValidationMode};
use crate::runner::RunResult;
use crate::tui::app::AppState;

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
        render_piscine_visualizer_overlay(f, body_rest, state);
    } else {
        render_piscine_body(f, body_rest, state);
    }

    render_piscine_status_bar(f, status_area, state);
}

fn difficulty_color(d: Difficulty) -> Color {
    match d {
        Difficulty::Easy => Color::Green,
        Difficulty::Medium => Color::Yellow,
        Difficulty::Hard => Color::Red,
        Difficulty::Advanced => Color::Magenta,
        Difficulty::Expert => Color::Cyan,
    }
}

fn difficulty_stars(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Easy => "★☆☆☆☆",
        Difficulty::Medium => "★★☆☆☆",
        Difficulty::Hard => "★★★☆☆",
        Difficulty::Advanced => "★★★★☆",
        Difficulty::Expert => "★★★★★",
    }
}

/// Mini-map de 8 exercices autour du curseur.
fn mini_map(completed: &[bool], current: usize) -> String {
    let total = completed.len();
    if total == 0 {
        return String::new();
    }
    let half = 4usize;
    let start = current.saturating_sub(half);
    let end = (start + 9).min(total);
    let start = end.saturating_sub(9).min(start);

    (start..end)
        .map(|i| {
            if i == current {
                "●"
            } else if completed.get(i).copied().unwrap_or(false) {
                "◉"
            } else {
                "○"
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

fn render_piscine_header(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let total = state.exercises.len();
    let idx = state.current_index;
    let width = area.width as usize;

    let map = mini_map(&state.completed, idx);

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
    let stars = difficulty_stars(exercise.difficulty);
    let diff_color = difficulty_color(exercise.difficulty);
    let mut meta_spans = vec![
        Span::styled(stars, Style::default().fg(diff_color)),
        Span::raw("  │  "),
        Span::styled(exercise.subject.as_str(), Style::default().fg(Color::Gray)),
    ];

    if let Some(stage) = state.current_stage {
        let stage_label = match stage {
            0 => "S0",
            1 => "S1",
            2 => "S2",
            3 => "S3",
            _ => "S4",
        };
        meta_spans.push(Span::raw("  │  "));
        meta_spans.push(Span::styled(stage_label, Style::default().fg(Color::Gray)));
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

/// Hauteur dynamique du panneau run_result.
fn run_result_height(result: &RunResult) -> u16 {
    if result.success || result.timeout {
        3
    } else if result.compile_error {
        7
    } else {
        9
    }
}

fn render_piscine_body(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];

    // Layout body : [left | right sidebar (si width >= 90)]
    let (content_area, sidebar_opt) = if area.width >= 90 {
        let [left, right] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(26)]).areas(area);
        (left, Some(right))
    } else {
        (area, None)
    };

    // Layout contenu : description (fill) | result (hauteur dynamique si présent)
    let body_areas = if let Some(result) = &state.run_result {
        let h = run_result_height(result);
        let [desc, res] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(h)]).areas(content_area);
        vec![desc, res]
    } else {
        vec![content_area]
    };

    // ── Description / hints ──────────────────────────────────────────────
    let desc_area = body_areas[0];
    let mut lines: Vec<Line> = Vec::new();

    for line in exercise.description.lines() {
        lines.push(Line::from(line));
    }
    let has_meta = exercise.key_concept.is_some()
        || exercise.common_mistake.is_some()
        || !exercise.files.is_empty();
    if has_meta {
        lines.push(Line::styled(
            "─".repeat(36),
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        lines.push(Line::raw(""));
    }

    if let Some(kc) = &exercise.key_concept {
        lines.push(Line::from(vec![
            Span::styled("concept : ", Style::default().fg(Color::Cyan)),
            Span::raw(kc.as_str()),
        ]));
    }
    if let Some(cm) = &exercise.common_mistake {
        lines.push(Line::from(vec![
            Span::styled("piège   : ", Style::default().fg(Color::Yellow)),
            Span::styled(cm.as_str(), Style::default().fg(Color::DarkGray)),
        ]));
    }
    if !exercise.files.is_empty() {
        let names: Vec<&str> = exercise.files.iter().map(|f| f.name.as_str()).collect();
        lines.push(Line::from(vec![
            Span::styled("fichiers: ", Style::default().fg(Color::Gray)),
            Span::styled(names.join(", "), Style::default().fg(Color::DarkGray)),
        ]));
    }

    if state.hint_shown && !exercise.hints.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "── Indices ──",
            Style::default().fg(Color::Cyan),
        ));
        for (i, hint) in exercise.hints.iter().enumerate() {
            lines.push(Line::from(format!("  {}. {}", i + 1, hint)));
        }
    }

    let title = if let Some(path) = &state.source_path {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("current.c");
        format!("Exercice — {}", filename)
    } else {
        "Exercice".to_string()
    };

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(title.as_str()))
            .wrap(Wrap { trim: false }),
        desc_area,
    );

    // ── Résultat de compilation ──────────────────────────────────────────
    if let Some(result_area) = body_areas.get(1) {
        if let Some(result) = &state.run_result {
            render_run_result(f, *result_area, result, exercise);
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
    let map = mini_map(&state.completed, idx);

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

fn render_run_result(
    f: &mut Frame,
    area: Rect,
    result: &crate::runner::RunResult,
    exercise: &crate::models::Exercise,
) {
    let (title, title_color) = if result.success {
        (format!("✓ SUCCÈS ({}ms)", result.duration_ms), Color::Green)
    } else if result.compile_error {
        ("✗ ERREUR DE COMPILATION".to_string(), Color::Red)
    } else if result.timeout {
        ("✗ TIMEOUT".to_string(), Color::Red)
    } else {
        let is_test = matches!(
            exercise.validation.mode,
            ValidationMode::Test | ValidationMode::Both
        );
        if is_test {
            ("✗ TESTS ÉCHOUÉS".to_string(), Color::Red)
        } else {
            ("✗ SORTIE INCORRECTE".to_string(), Color::Red)
        }
    };

    let color = title_color;
    let mut lines: Vec<Line> = Vec::new();

    if result.success {
        for line in result.stdout.lines() {
            lines.push(Line::from(Span::styled(
                line,
                Style::default().fg(Color::Green),
            )));
        }
    } else if result.compile_error {
        for line in result.stderr.lines().take(5) {
            lines.push(Line::from(Span::styled(
                line,
                Style::default().fg(Color::Red),
            )));
        }
    } else if result.timeout {
        lines.push(Line::from("Dépassement de 10s — boucle infinie ?"));
    } else if let Some(expected) = &exercise.validation.expected_output {
        let exp_lines: Vec<&str> = expected.trim().lines().collect();
        let got_lines: Vec<&str> = result.stdout.trim().lines().collect();
        let max_len = exp_lines.len().max(got_lines.len());
        for i in 0..max_len.min(4) {
            match (exp_lines.get(i), got_lines.get(i)) {
                (Some(e), Some(g)) if *e == *g => {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", e),
                        Style::default().fg(Color::Green),
                    )));
                }
                (Some(e), Some(g)) => {
                    lines.push(Line::from(Span::styled(
                        format!("- {}", e),
                        Style::default().fg(Color::Red),
                    )));
                    lines.push(Line::from(Span::styled(
                        format!("+ {}", g),
                        Style::default().fg(Color::Yellow),
                    )));
                }
                (Some(e), None) => {
                    lines.push(Line::from(Span::styled(
                        format!("- {}", e),
                        Style::default().fg(Color::Red),
                    )));
                }
                (None, Some(g)) => {
                    lines.push(Line::from(Span::styled(
                        format!("+ {}", g),
                        Style::default().fg(Color::Yellow),
                    )));
                }
                (None, None) => {}
            }
        }
    }

    let block = Block::bordered()
        .title(Span::styled(
            title,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(color));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_piscine_visualizer_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let steps = &exercise.visualizer.steps;

    if steps.is_empty() {
        return;
    }

    let step_idx = state.vis_step.min(steps.len() - 1);
    let step = &steps[step_idx];

    // Popup taille dynamique
    let n_items = (step.stack.len() + step.heap.len()).max(3) as u16;
    let h_pct = (n_items * 6).clamp(35, 60);
    let w_pct = 65u16;
    let margin_v = (100u16.saturating_sub(h_pct)) / 2;
    let margin_h = (100u16.saturating_sub(w_pct)) / 2;

    let [_, popup_v, _] = Layout::vertical([
        Constraint::Percentage(margin_v),
        Constraint::Percentage(h_pct),
        Constraint::Percentage(margin_v),
    ])
    .areas(area);
    let [_, popup, _] = Layout::horizontal([
        Constraint::Percentage(margin_h),
        Constraint::Percentage(w_pct),
        Constraint::Percentage(margin_h),
    ])
    .areas(popup_v);

    f.render_widget(Clear, popup);

    let mut lines: Vec<Line> = Vec::new();

    let dots: String = (0..steps.len())
        .map(|i| if i == step_idx { "●" } else { "○" })
        .collect::<Vec<_>>()
        .join(" ");
    lines.push(Line::styled(dots, Style::default().fg(Color::Yellow)));
    lines.push(Line::raw(""));

    let label = if !step.step_label.is_empty() {
        &step.step_label
    } else {
        &step.label
    };
    lines.push(Line::styled(
        label.as_str(),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    lines.push(Line::from(vec![
        Span::styled(
            format!("{:<25}", "STACK"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "HEAP",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let max_rows = step.stack.len().max(step.heap.len()).max(1);
    for i in 0..max_rows {
        let left = step
            .stack
            .get(i)
            .map(|v| format!("{}: {}", v.name, v.value))
            .unwrap_or_default();
        let right = step
            .heap
            .get(i)
            .map(|v| format!("{}: {}", v.name, v.value))
            .unwrap_or_else(|| {
                if step.heap.is_empty() && i == 0 {
                    "(vide)".to_string()
                } else {
                    String::new()
                }
            });
        lines.push(Line::from(vec![
            Span::styled(format!("{:<25}", left), Style::default().fg(Color::Green)),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(right, Style::default().fg(Color::Cyan)),
        ]));
    }

    lines.push(Line::raw(""));

    if !step.explanation.is_empty() {
        for part in step.explanation.split(". ") {
            lines.push(Line::styled(part, Style::default().fg(Color::Gray)));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "[←] préc   [→] suiv   [v] fermer",
        Style::default().fg(Color::Gray),
    ));

    let title = format!("Visualiseur {}/{}", step_idx + 1, steps.len());
    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .title(title)
                    .style(Style::default().bg(Color::Black))
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn render_piscine_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let has_vis = !exercise.visualizer.steps.is_empty();

    let has_hints = !exercise.hints.is_empty();
    let left_msg = if let Some(status) = &state.status_msg {
        status.as_str().to_string()
    } else {
        let mut parts = vec![
            "[r] compiler".to_string(),
            "[n] suivant".to_string(),
            "[k] précédent".to_string(),
        ];
        if has_hints {
            parts.insert(1, "[h] indice".to_string());
        }
        if has_vis {
            parts.push("[v] vis".to_string());
        }
        parts.push("[q] quitter".to_string());
        parts.join("  ")
    };

    // Droite : échecs cumulés
    let right_msg = if state.piscine_fail_count > 0 {
        format!("✗ {}", state.piscine_fail_count)
    } else {
        String::new()
    };

    if right_msg.is_empty() || area.width < 40 {
        f.render_widget(
            Paragraph::new(left_msg).style(Style::default().fg(Color::DarkGray)),
            area,
        );
    } else {
        let [left_area, right_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(10)]).areas(area);
        f.render_widget(
            Paragraph::new(left_msg).style(Style::default().fg(Color::DarkGray)),
            left_area,
        );
        f.render_widget(
            Paragraph::new(right_msg).style(Style::default().fg(Color::Red)),
            right_area,
        );
    }
}
