//! Vue watch — rendu Ratatui pour le mode progression SRS.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::models::{Difficulty, ValidationMode};
use crate::runner::RunResult;
use crate::tui::app::AppState;

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

/// Barre de mastery unicode avec couleur gradient.
/// Retourne (bar_string, color) pour affichage coloré.
fn mastery_bar(score: f64, width: usize) -> (String, Color) {
    let filled = (score.clamp(0.0, 5.0) / 5.0 * width as f64).round() as usize;
    let full = "█".repeat(filled);
    let empty = "░".repeat(width - filled);
    let color = if score < 1.0 {
        Color::Red
    } else if score < 2.5 {
        Color::Yellow
    } else if score < 4.0 {
        Color::Green
    } else {
        Color::Cyan
    };
    (format!("{}{}", full, empty), color)
}

/// Mini-map de 8 exercices autour du curseur (●=courant, ◉=complété, ○=pas encore).
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

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let total = state.exercises.len();
    let idx = state.current_index;
    let width = area.width as usize;

    // Mastery du sujet courant
    let mastery = state
        .mastery_map
        .get(&exercise.subject)
        .copied()
        .unwrap_or(0.0);
    let (bar, bar_color) = mastery_bar(mastery, 10);
    let map = mini_map(&state.completed, idx);

    // ── Ligne 1 : [idx/total] Titre ── + droit: chapter mini-map ──────
    let left1 = format!("[{}/{}] {}", idx + 1, total, exercise.title);
    // chars().count() pour la largeur d'affichage (●◉○ = 3 octets mais 1 col)
    let right1_display = map.chars().count() + 2 + exercise.subject.chars().count();
    let pad1 = width.saturating_sub(left1.chars().count() + right1_display + 4);
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

    // ── Ligne 2 : stars | type | stage ── + droit: mastery bar ────────
    let stars = difficulty_stars(exercise.difficulty);
    let diff_color = difficulty_color(exercise.difficulty);
    let mut meta_spans: Vec<Span> = vec![
        Span::styled(stars, Style::default().fg(diff_color)),
        Span::raw("  │  "),
        Span::styled(
            exercise.exercise_type.to_string(),
            Style::default().fg(Color::Gray),
        ),
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

    // "mastery: X.X  " + 10 chars de barre
    let right2_display = format!("mastery: {:.1}  ", mastery).chars().count() + 10;
    let left2_display: usize = meta_spans
        .iter()
        .map(|s| s.content.chars().count())
        .sum::<usize>();
    let pad2 = width.saturating_sub(left2_display + right2_display + 4);
    meta_spans.push(Span::raw(" ".repeat(pad2 + 1)));
    meta_spans.push(Span::styled(
        format!("mastery: {:.1}  ", mastery),
        Style::default().fg(bar_color),
    ));
    meta_spans.push(Span::styled(bar, Style::default().fg(bar_color)));
    let line2 = Line::from(meta_spans);

    // ── Ligne 3 : révision due (optionnelle) ──────────────────────────
    let due_count = state
        .review_map
        .values()
        .filter(|v| v.map(|d| d <= 0).unwrap_or(false))
        .count();
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

fn render_body(f: &mut Frame, area: Rect, state: &AppState) {
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

    // ── Sidebar mastery ──────────────────────────────────────────────────
    if let Some(sb_area) = sidebar_opt {
        render_mastery_sidebar(f, sb_area, state);
    }
}

fn render_mastery_sidebar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];

    // Collecte les sujets uniques depuis les exercices
    let mut chapter_subjects: Vec<String> = Vec::new();
    for ex in &state.exercises {
        if !chapter_subjects.contains(&ex.subject) {
            chapter_subjects.push(ex.subject.clone());
        }
    }
    // Priorité au sujet courant puis les 7 premiers
    let top: Vec<&String> = {
        let mut result: Vec<&String> = chapter_subjects.iter().take(8).collect();
        if !result.contains(&&exercise.subject) {
            result.insert(0, &exercise.subject);
            result.truncate(8);
        }
        result
    };

    let mut lines: Vec<Line> = Vec::new();

    for subj in &top {
        let score = state.mastery_map.get(*subj).copied().unwrap_or(0.0);
        let (bar, bar_color) = mastery_bar(score, 8);
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
    let due_count = state
        .review_map
        .values()
        .filter(|v| v.map(|d| d <= 0).unwrap_or(false))
        .count();
    if due_count > 0 {
        lines.push(Line::from(Span::styled(
            format!("↻ {} révision(s)", due_count),
            Style::default().fg(Color::Yellow),
        )));
    }

    let block = Block::bordered().title("Progression");
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

/// Calcule la taille du popup visualiseur en fonction du contenu.
fn popup_size_for_vis(step: &crate::models::VisStep) -> (u16, u16) {
    let n_items = (step.stack.len() + step.heap.len()).max(3) as u16;
    let h_pct = (n_items * 6).clamp(35, 60);
    let w_pct = 65u16;
    (w_pct, h_pct)
}

fn render_visualizer_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let steps = &exercise.visualizer.steps;

    if steps.is_empty() {
        return;
    }

    let step_idx = state.vis_step.min(steps.len() - 1);
    let step = &steps[step_idx];

    let (w_pct, h_pct) = popup_size_for_vis(step);
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

fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let has_vis = !exercise.visualizer.steps.is_empty();

    let has_hints = !exercise.hints.is_empty();
    let left_msg = if let Some(status) = &state.status_msg {
        status.as_str().to_string()
    } else {
        let mut parts = vec![
            "[j] suiv".to_string(),
            "[k] préc".to_string(),
            "[n] skip".to_string(),
            "[r] run".to_string(),
        ];
        if has_hints {
            parts.insert(0, "[h] hint".to_string());
        }
        if has_vis {
            parts.push("[v] vis".to_string());
        }
        parts.push("[q] quit".to_string());
        parts.join("  ")
    };

    // Droite : failures ou révision
    let right_msg = if state.consecutive_failures > 0 {
        format!("✗ {}", state.consecutive_failures)
    } else {
        let due = state
            .review_map
            .values()
            .filter(|v| v.map(|d| d <= 0).unwrap_or(false))
            .count();
        if due > 0 {
            format!("révision: {}j", due)
        } else {
            String::new()
        }
    };

    if right_msg.is_empty() || area.width < 40 {
        f.render_widget(
            Paragraph::new(left_msg).style(Style::default().fg(Color::DarkGray)),
            area,
        );
    } else {
        let [left_area, right_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(15)]).areas(area);
        f.render_widget(
            Paragraph::new(left_msg).style(Style::default().fg(Color::DarkGray)),
            left_area,
        );
        let right_style = if state.consecutive_failures > 0 {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Yellow)
        };
        f.render_widget(Paragraph::new(right_msg).style(right_style), right_area);
    }
}
