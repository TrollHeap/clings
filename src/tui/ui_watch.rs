//! Vue watch — rendu Ratatui pour le mode progression SRS.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::models::{Difficulty, ValidationMode};
use crate::tui::app::AppState;

/// Point d'entrée du rendu watch (appelé par App::run_watch).
pub fn view(f: &mut Frame, state: &AppState) {
    let area = f.area();

    if state.exercises.is_empty() {
        f.render_widget(
            Paragraph::new("Aucun exercice disponible.").block(Block::bordered()),
            area,
        );
        return;
    }

    // Layout : header (3) | body (fill) | status (1)
    let [header_area, body_area, status_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    render_header(f, header_area, state);

    if state.vis_active {
        render_visualizer_overlay(f, body_area, state);
    } else {
        render_body(f, body_area, state);
    }

    render_status_bar(f, status_area, state);
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

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let total = state.exercises.len();
    let idx = state.current_index;

    // Ligne 1 : [idx/total] titre
    let progress_span = Span::styled(
        format!("[{}/{}] ", idx + 1, total),
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    );
    let title_span = Span::styled(
        exercise.title.as_str(),
        Style::default().add_modifier(Modifier::BOLD),
    );
    let line1 = Line::from(vec![progress_span, title_span]);

    // Ligne 2 : difficulté | type | sujet | stage | révision SRS
    let stars = difficulty_stars(exercise.difficulty);
    let diff_color = difficulty_color(exercise.difficulty);
    let mut meta_spans = vec![
        Span::styled(stars, Style::default().fg(diff_color)),
        Span::raw("  │  "),
        Span::styled(
            exercise.exercise_type.to_string(),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("  │  "),
        Span::styled(
            exercise.subject.as_str(),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    if let Some(stage) = state.current_stage {
        let stage_label = match stage {
            0 => "S0 Exemple",
            1 => "S1 Guide",
            2 => "S2 Blancs",
            3 => "S3 Squelette",
            _ => "S4 Autonome",
        };
        meta_spans.push(Span::raw("  │  "));
        meta_spans.push(Span::styled(
            stage_label,
            Style::default().fg(Color::DarkGray),
        ));
    }

    if let Some(days) = state.review_map.get(&exercise.subject).copied().flatten() {
        meta_spans.push(Span::raw("  │  "));
        let (label, color) = if days <= 0 {
            ("dû".to_string(), Color::Yellow)
        } else {
            (format!("dans {} j", days), Color::DarkGray)
        };
        meta_spans.push(Span::styled(label, Style::default().fg(color)));
    }

    let line2 = Line::from(meta_spans);

    let text = Text::from(vec![line1, line2]);
    let block = Block::bordered().title("clings — watch");
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn render_body(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];

    // Layout body : description (fill) | result (si run_result present, max 10)
    let body_areas = if state.run_result.is_some() {
        let [desc, result] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(12)]).areas(area);
        vec![desc, result]
    } else {
        vec![area]
    };

    // ── Description / hints ──────────────────────────────────────────────
    let desc_area = body_areas[0];
    let mut lines: Vec<Line> = Vec::new();

    // Description
    for line in exercise.description.lines() {
        lines.push(Line::from(line));
    }
    lines.push(Line::raw(""));

    if let Some(kc) = &exercise.key_concept {
        lines.push(Line::from(vec![
            Span::styled(
                "💡 Concept clé : ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(kc.as_str()),
        ]));
    }
    if let Some(cm) = &exercise.common_mistake {
        lines.push(Line::from(vec![
            Span::styled(
                "⚠ Piège : ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(cm.as_str(), Style::default().fg(Color::DarkGray)),
        ]));
    }
    if !exercise.files.is_empty() {
        let names: Vec<&str> = exercise.files.iter().map(|f| f.name.as_str()).collect();
        lines.push(Line::from(vec![
            Span::styled("📎 Fichiers : ", Style::default().fg(Color::Cyan)),
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
        format!("Exercice — {}", path.display())
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
        for line in result.stderr.lines().take(8) {
            lines.push(Line::from(Span::styled(
                line,
                Style::default().fg(Color::Red),
            )));
        }
    } else if result.timeout {
        lines.push(Line::from("Dépassement de 10s — boucle infinie ?"));
    } else {
        // Diff expected/got
        if let Some(expected) = &exercise.validation.expected_output {
            let exp_lines: Vec<&str> = expected.trim().lines().collect();
            let got_lines: Vec<&str> = result.stdout.trim().lines().collect();
            let max_len = exp_lines.len().max(got_lines.len());
            for i in 0..max_len.min(6) {
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
    }

    let block = Block::bordered()
        .title(Span::styled(
            title,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(color));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_visualizer_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let steps = &exercise.visualizer.steps;

    if steps.is_empty() {
        return;
    }

    let step_idx = state.vis_step.min(steps.len() - 1);
    let step = &steps[step_idx];

    // Popup centré (80% largeur, 70% hauteur)
    let [_, popup_v, _] = Layout::vertical([
        Constraint::Percentage(15),
        Constraint::Percentage(70),
        Constraint::Percentage(15),
    ])
    .areas(area);
    let [_, popup, _] = Layout::horizontal([
        Constraint::Percentage(10),
        Constraint::Percentage(80),
        Constraint::Percentage(10),
    ])
    .areas(popup_v);

    f.render_widget(Clear, popup);

    let mut lines: Vec<Line> = Vec::new();

    // Progress dots
    let dots: String = (0..steps.len())
        .map(|i| if i == step_idx { "●" } else { "○" })
        .collect::<Vec<_>>()
        .join(" ");
    lines.push(Line::styled(dots, Style::default().fg(Color::Yellow)));
    lines.push(Line::raw(""));

    // Label
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

    // Stack/Heap headers
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:<28}", "STACK"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "HEAP",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Variables
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
            Span::styled(format!("{:<28}", left), Style::default().fg(Color::Green)),
            Span::styled(right, Style::default().fg(Color::Cyan)),
        ]));
    }

    lines.push(Line::raw(""));

    // Explanation
    if !step.explanation.is_empty() {
        for part in step.explanation.split(". ") {
            lines.push(Line::styled(part, Style::default().fg(Color::DarkGray)));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "[←] préc   [→] suiv   [v] fermer",
        Style::default().fg(Color::DarkGray),
    ));

    let title = format!("Visualiseur {}/{}", step_idx + 1, steps.len());
    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .title(title)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let has_vis = !exercise.visualizer.steps.is_empty();

    let msg = if let Some(status) = &state.status_msg {
        status.as_str().to_string()
    } else {
        let mut parts = vec![
            "[h] hint".to_string(),
            "[j] suiv".to_string(),
            "[k] préc".to_string(),
            "[n] skip".to_string(),
            "[r] run".to_string(),
        ];
        if has_vis {
            parts.push("[v] visualiser".to_string());
        }
        parts.push("[q] quit".to_string());
        parts.join("  ")
    };

    f.render_widget(
        Paragraph::new(msg).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}
