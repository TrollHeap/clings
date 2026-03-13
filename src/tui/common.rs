//! Fonctions TUI partagées entre ui_watch, ui_piscine, ui_list et ui_stats.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::models::{Difficulty, ValidationMode};
use crate::runner::RunResult;
use crate::tui::app::AppState;

/// Étiquette de stage d'échafaudage (S0–S4).
pub fn stage_label(stage: u8) -> &'static str {
    match stage {
        0 => "S0",
        1 => "S1",
        2 => "S2",
        3 => "S3",
        _ => "S4",
    }
}

/// Calcule la zone d'un popup centré avec des marges en pourcentage.
fn centered_popup(area: Rect, margin_v_pct: u16, margin_h_pct: u16) -> Rect {
    let content_v = 100u16.saturating_sub(margin_v_pct * 2);
    let content_h = 100u16.saturating_sub(margin_h_pct * 2);
    let [_, popup_v, _] = Layout::vertical([
        Constraint::Percentage(margin_v_pct),
        Constraint::Percentage(content_v),
        Constraint::Percentage(margin_v_pct),
    ])
    .areas(area);
    let [_, popup, _] = Layout::horizontal([
        Constraint::Percentage(margin_h_pct),
        Constraint::Percentage(content_h),
        Constraint::Percentage(margin_h_pct),
    ])
    .areas(popup_v);
    popup
}

/// Couleur associée à un niveau de difficulté.
pub fn difficulty_color(d: Difficulty) -> Color {
    match d {
        Difficulty::Easy => Color::Green,
        Difficulty::Medium => Color::Yellow,
        Difficulty::Hard => Color::Red,
        Difficulty::Advanced => Color::Magenta,
        Difficulty::Expert => Color::Cyan,
    }
}

/// Étoiles unicode associées à un niveau de difficulté.
pub fn difficulty_stars(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Easy => "★☆☆☆☆",
        Difficulty::Medium => "★★☆☆☆",
        Difficulty::Hard => "★★★☆☆",
        Difficulty::Advanced => "★★★★☆",
        Difficulty::Expert => "★★★★★",
    }
}

/// Couleur gradient pour un score de maîtrise (0.0–5.0).
pub fn mastery_color(score: f64) -> Color {
    if score < 1.0 {
        Color::Red
    } else if score < 2.5 {
        Color::Yellow
    } else if score < 4.0 {
        Color::Green
    } else {
        Color::Cyan
    }
}

/// Mini-map de 9 exercices autour du curseur (●=courant, ◉=complété, ○=pas encore).
pub fn mini_map(completed: &[bool], current: usize) -> String {
    let total = completed.len();
    if total == 0 {
        return String::new();
    }
    let half = 4usize;
    let start = current.saturating_sub(half);
    let end = (start + 9).min(total);
    let start = end.saturating_sub(9).min(start);

    // ●/◉/○ = 3 bytes UTF-8 chacun — allocation exacte, zéro Vec intermédiaire
    let mut map = String::with_capacity((end - start) * 3);
    for i in start..end {
        map.push_str(if i == current {
            "●"
        } else if completed.get(i).copied().unwrap_or(false) {
            "◉"
        } else {
            "○"
        });
    }
    map
}

/// Hauteur dynamique du panneau run_result.
pub fn run_result_height(result: &RunResult) -> u16 {
    if result.success || result.timeout {
        3
    } else if result.compile_error {
        7
    } else {
        9
    }
}

/// Rendu du panneau résultat de compilation/exécution.
pub fn render_run_result(
    f: &mut Frame,
    area: Rect,
    result: &RunResult,
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
pub fn popup_size_for_vis(step: &crate::models::VisStep) -> (u16, u16) {
    let n_items = (step.stack.len() + step.heap.len()).max(3) as u16;
    let h_pct = (n_items * 6).clamp(35, 60);
    let w_pct = 65u16;
    (w_pct, h_pct)
}

/// Overlay visualiseur mémoire (partagé entre watch et piscine).
pub fn render_visualizer_overlay(f: &mut Frame, area: Rect, state: &AppState) {
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
    let popup = centered_popup(area, margin_v, margin_h);

    f.render_widget(Clear, popup);

    let mut lines: Vec<Line> = Vec::new();

    // "● " = 4 bytes max — allocation exacte, zéro Vec intermédiaire
    let mut dots = String::with_capacity(steps.len() * 4);
    for i in 0..steps.len() {
        if i > 0 {
            dots.push(' ');
        }
        dots.push_str(if i == step_idx { "●" } else { "○" });
    }
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

/// Overlay de recherche fuzzy (touche `/` depuis watch).
pub fn render_search_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    let popup = centered_popup(area, 15, 10);
    f.render_widget(Clear, popup);

    // Split: query input (3 lines) | results list (fill) | hint bar (1 line)
    let [query_area, results_area, hint_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(popup);

    // Query input
    let cursor = if (f.count() / 4).is_multiple_of(2) {
        "█"
    } else {
        " "
    };
    let query_display = format!("{}{}", state.search_query, cursor);
    let overlay_title = if state.search_subject_filter {
        let subject = state
            .exercises
            .get(state.current_index)
            .map(|ex| ex.subject.as_str())
            .unwrap_or("?");
        format!("/ Recherche (sujet: {})", subject)
    } else {
        "/ Recherche".to_string()
    };
    f.render_widget(
        Paragraph::new(query_display).block(
            Block::bordered()
                .title(overlay_title)
                .style(Style::default().bg(Color::Black))
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        query_area,
    );

    // Results list — iterate directly from indices, no intermediate Vec
    let items: Vec<ListItem> = state
        .search_results
        .iter()
        .filter_map(|&idx| state.exercises.get(idx))
        .map(|ex| {
            let stars = difficulty_stars(ex.difficulty);
            let color = difficulty_color(ex.difficulty);
            // char_indices().nth(N) gives the byte boundary without allocating an intermediate String
            let title_end = ex
                .title
                .char_indices()
                .nth(28)
                .map(|(i, _)| i)
                .unwrap_or(ex.title.len());
            let subj_end = ex
                .subject
                .char_indices()
                .nth(16)
                .map(|(i, _)| i)
                .unwrap_or(ex.subject.len());
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<30}", &ex.title[..title_end]),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("{:<18}", &ex.subject[..subj_end]),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(stars, Style::default().fg(color)),
            ]))
        })
        .collect();

    let count = state.search_results.len();
    let list_title = if state.search_query.is_empty() {
        format!(" {count} exercices ")
    } else {
        format!(" {count} résultats ")
    };

    let mut list_state = ListState::default();
    if !state.search_results.is_empty() {
        list_state.select(Some(state.search_selected));
    }

    f.render_stateful_widget(
        List::new(items)
            .block(
                Block::bordered()
                    .title(list_title)
                    .style(Style::default().bg(Color::Black))
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
        results_area,
        &mut list_state,
    );

    // Hint bar
    f.render_widget(
        Paragraph::new(
            "[↑↓/jk] nav  [g/G] début/fin  [Entrée] aller  [Tab] filtre sujet  [Esc] fermer",
        )
        .style(Style::default().fg(Color::DarkGray)),
        hint_area,
    );
}

/// Overlay solution — affiche le code solution de l'exercice courant.
pub fn render_solution_overlay(f: &mut Frame, area: Rect, exercise: &crate::models::Exercise) {
    let popup = centered_popup(area, 10, 10);
    f.render_widget(Clear, popup);

    let lines: Vec<Line> = exercise.solution_code.lines().map(Line::raw).collect();
    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .title("Solution — [Esc/s] fermer")
                    .style(Style::default().bg(Color::Black))
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}

/// Overlay d'aide — raccourcis clavier du mode watch.
pub fn render_help_overlay(f: &mut Frame, area: Rect) {
    let popup = centered_popup(area, 15, 20);
    f.render_widget(Clear, popup);

    let bindings: &[(&str, &str)] = &[
        ("[j] / [n]", "Exercice suivant"),
        ("[k]", "Exercice précédent"),
        ("[r]", "Compiler et vérifier"),
        ("[h]", "Afficher l'indice"),
        ("[v]", "Visualiseur mémoire"),
        ("[/]", "Recherche fuzzy"),
        ("[Tab]", "Filtrer par sujet (en recherche)"),
        ("[←][→]", "Étape visualiseur"),
        ("[q]", "Quitter"),
        ("", ""),
        ("[?]", "Afficher cette aide"),
    ];

    let mut lines: Vec<Line> = vec![Line::raw("")];
    for (key, desc) in bindings {
        if key.is_empty() {
            lines.push(Line::raw(""));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<10}", key),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(*desc, Style::default().fg(Color::White)),
            ]));
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "  Appuyez sur n'importe quelle touche pour fermer",
        Style::default().fg(Color::DarkGray),
    ));

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .title("Aide — raccourcis")
                    .style(Style::default().bg(Color::Black))
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}
