//! Vue watch — rendu Ratatui pour le mode progression SRS.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
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

    if state.vis_active {
        common::render_visualizer_overlay(f, body_area, state);
    } else {
        render_body(f, body_area, state);
    }

    render_status_bar(f, status_area, state);
}

/// Barre de mastery unicode avec couleur gradient.
/// Retourne (bar_string, color) pour affichage coloré.
fn mastery_bar(score: f64, width: usize) -> (String, Color) {
    let filled = (score.clamp(0.0, 5.0) / 5.0 * width as f64).round() as usize;
    let full = "█".repeat(filled);
    let empty = "░".repeat(width - filled);
    (format!("{}{}", full, empty), common::mastery_color(score))
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
    let map = common::mini_map(&state.completed, idx);

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
        let h = common::run_result_height(result);
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
            common::render_run_result(f, *result_area, result, exercise);
        }
    }

    // ── Sidebar mastery ──────────────────────────────────────────────────
    if let Some(sb_area) = sidebar_opt {
        render_mastery_sidebar(f, sb_area, state);
    }
}

fn render_mastery_sidebar(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];

    // Collecte les sujets uniques depuis les exercices — O(n) via HashSet
    let mut seen = std::collections::HashSet::new();
    let chapter_subjects: Vec<&String> = state
        .exercises
        .iter()
        .map(|ex| &ex.subject)
        .filter(|s| seen.insert(s.as_str()))
        .collect();
    // Priorité au sujet courant puis les 7 premiers
    let top: Vec<&String> = {
        let mut result: Vec<&String> = chapter_subjects.iter().copied().take(8).collect();
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
