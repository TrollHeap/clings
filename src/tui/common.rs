//! Fonctions TUI partagées entre ui_watch, ui_piscine, ui_list et ui_stats.
//!
//! Les sous-modules `style` et `overlays` sont re-exportés ici pour compatibilité
//! des importeurs existants (`use crate::tui::common::*`).

pub use crate::tui::overlays::{
    centered_popup, is_pointer_value, render_help_overlay, render_list_overlay,
    render_nav_confirm_overlay, render_opaque_background, render_quit_confirm_overlay,
    render_search_overlay, render_solution_overlay, render_success_overlay,
    render_visualizer_overlay, vis_col_widths,
};
pub use crate::tui::style::{
    difficulty_color, difficulty_stars, exercise_type_badge, mastery_bar_string, mastery_color,
    mini_map, next_stage_threshold, stage_badge, BODY_SIDEBAR_THRESHOLD, C_ACCENT, C_BG, C_BORDER,
    C_DANGER, C_INFO, C_MAUVE, C_OVERLAY, C_SUBTEXT, C_SUCCESS, C_SURFACE, C_TEAL, C_TEXT,
    C_TEXT_DIM, C_WARNING, C_YELLOW, SEPARATOR, SIDEBAR_WIDTH,
};

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui::Frame;
use ratatui_macros::{line, span};

use crate::models::{Exercise, ValidationMode};
use crate::runner::RunResult;
use crate::tui::app::{ActiveOverlay, AppState};

/// Parse une ligne avec syntaxe backtick inline : `code` → C_ACCENT BOLD.
fn parse_inline_code(line: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut is_code = false;
    for part in line.split('`') {
        if !part.is_empty() {
            if is_code {
                spans.push(Span::styled(
                    part.to_owned(),
                    Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::raw(part.to_owned()));
            }
        }
        is_code = !is_code;
    }
    Line::from(spans)
}

/// Panneau description/indices — partagé entre watch et piscine.
/// Affiche description, key_concept, common_mistake, fichiers, et indices révélés.
pub fn render_description_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let Some(exercise) = state.ex.exercises.get(state.ex.current_index) else {
        return;
    };
    let mut lines: Vec<Line> = Vec::with_capacity(16);

    for line in exercise.description.lines() {
        lines.push(parse_inline_code(line));
    }
    let has_meta = exercise.key_concept.is_some()
        || exercise.common_mistake.is_some()
        || !exercise.files.is_empty();
    if has_meta {
        lines.push(Line::styled(SEPARATOR, Style::default().fg(C_OVERLAY)));
    } else {
        lines.push(Line::raw(""));
    }

    if let Some(kc) = &exercise.key_concept {
        lines.push(line![span!(C_TEAL; "concept : "), Span::raw(kc.as_str()),]);
    }
    if let Some(cm) = &exercise.common_mistake {
        lines.push(line![
            span!(C_WARNING; "piège   : "),
            span!(C_OVERLAY; "{}", cm.as_str()),
        ]);
    }
    if !exercise.files.is_empty() {
        let names: Vec<&str> = exercise.files.iter().map(|fi| fi.name.as_str()).collect();
        lines.push(line![
            span!(C_TEXT_DIM; "fichiers: "),
            span!(C_OVERLAY; "{}", names.join(", ")),
        ]);
    }

    if state.ex.hint_index > 0 && !exercise.hints.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("── Indices ──", Style::default().fg(C_TEAL)));
        for (i, hint) in exercise.hints[..state.ex.hint_index].iter().enumerate() {
            lines.push(Line::from(format!("  {}. {}", i + 1, hint)));
        }
    }

    let title = if let Some(path) = &state.ex.source_path {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("current.c");
        format!("Exercice — {}", filename)
    } else {
        "Exercice".to_string()
    };

    let content_length = lines.len();
    let scroll = state.ex.description_scroll;
    let mut scroll_state = ScrollbarState::new(content_length).position(scroll as usize);

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(C_BORDER))
                    .style(Style::default().bg(C_BG))
                    .title(span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD); "{}", title)),
            )
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false }),
        area,
    );

    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None),
        area,
        &mut scroll_state,
    );
}

/// Retourne la ligne de statut pour les états spéciaux (overlay actif, compilation, status_msg).
/// Retourne `None` si aucun état spécial — l'appelant construit alors la ligne de touches.
/// `has_help` : true pour le mode watch (qui possède un overlay d'aide [?]).
pub fn status_bar_prefix_line(state: &AppState, has_help: bool) -> Option<Line<'static>> {
    let dim = Style::default().fg(C_TEXT_DIM);
    if state.session.compile_pending {
        return Some(Line::styled("⏳ Compilation en cours…", dim));
    }
    if has_help && state.overlay.active == ActiveOverlay::Help {
        return Some(Line::styled("[Esc/?] fermer", dim));
    }
    if state.overlay.active == ActiveOverlay::Solution {
        return Some(Line::styled("[Esc/s] fermer solution", dim));
    }
    if state.overlay.active == ActiveOverlay::List {
        return Some(Line::styled(
            "[↑↓/jk] nav  [Tab/S-Tab] chapitre  [Entrée] aller  [Esc/l/q] fermer",
            dim,
        ));
    }
    if state.overlay.active == ActiveOverlay::Search {
        return Some(Line::styled(
            "[↑↓/jk] nav  [Entrée] aller  [Esc] fermer",
            dim,
        ));
    }
    if let Some(status) = &state.session.status_msg {
        return Some(Line::styled(status.clone(), dim));
    }
    None
}

/// Remplace le label d'un keybind par un compteur (ex: " hint" → " hint (2/3)").
pub fn update_hint_counter(spans: &mut [Span<'static>], label: &str, index: usize, total: usize) {
    if let Some(pos) = spans.iter().position(|s| s.content == label) {
        spans[pos] = Span::styled(
            format!("{} ({}/{})", label.trim(), index, total),
            Style::default().fg(C_TEXT_DIM),
        );
    }
}

/// Ajoute le compteur d'indices à la barre de statut si des indices ont été révélés.
/// Évite la duplication de la condition `if` dans chaque mode (watch, piscine).
pub fn append_hint_counter_if_visible(
    spans: &mut [Span<'static>],
    label: &str,
    hint_index: usize,
    hints_len: usize,
) {
    if hint_index > 0 && hints_len > 0 {
        update_hint_counter(spans, label, hint_index, hints_len);
    }
}

/// Barre de statut à deux colonnes — partagée entre watch et piscine.
/// Si `right_msg` est vide ou la largeur < 40, affiche seulement `left_line`.
pub fn render_split_status_bar(
    f: &mut Frame,
    area: Rect,
    left_line: Line<'static>,
    right_msg: String,
    right_style: Style,
    right_width: u16,
) {
    if right_msg.is_empty() || area.width < 40 {
        f.render_widget(Paragraph::new(left_line), area);
    } else {
        let [left_area, right_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(right_width)]).areas(area);
        f.render_widget(Paragraph::new(left_line), left_area);
        f.render_widget(Paragraph::new(right_msg).style(right_style), right_area);
    }
}

/// Layout body partagé : description + résultat + sidebar optionnelle.
/// Élimine la duplication entre `render_body` (watch) et `render_piscine_body`.
pub fn render_body_with_sidebar(
    f: &mut Frame,
    area: Rect,
    state: &AppState,
    render_sidebar: fn(&mut Frame, Rect, &AppState),
) {
    let (content_area, sidebar_opt) = if area.width >= BODY_SIDEBAR_THRESHOLD {
        let [left, right] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(SIDEBAR_WIDTH)])
                .areas(area);
        (left, Some(right))
    } else {
        (area, None)
    };

    let (desc_area, result_area_opt) = if let Some(result) = &state.ex.run_result {
        let Some(exercise) = state.ex.exercises.get(state.ex.current_index) else {
            return;
        };
        let expected = exercise.validation.expected_output.as_deref();
        let h = run_result_height(result, expected);
        let [desc, res] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(h)]).areas(content_area);
        (desc, Some(res))
    } else {
        (content_area, None)
    };

    render_description_panel(f, desc_area, state);

    if let Some(result_area) = result_area_opt {
        if let Some(result) = &state.ex.run_result {
            if let Some(exercise) = state.ex.exercises.get(state.ex.current_index) {
                render_run_result(f, result_area, result, exercise);
            }
        }
    }

    if let Some(sb_area) = sidebar_opt {
        render_sidebar(f, sb_area, state);
    }
}

/// Hauteur dynamique du panneau run_result.
pub fn run_result_height(result: &RunResult, expected: Option<&str>) -> u16 {
    if result.success || result.timeout {
        3
    } else if result.compile_error {
        if result.gcc_hint.is_some() {
            9
        } else {
            7
        }
    } else {
        const MAX: usize = 5;
        let exp_n = expected.unwrap_or("").trim().lines().count();
        let got_n = result.stdout.trim().lines().count();
        let content_n = exp_n.max(got_n);
        let content_h = content_n.min(MAX) + usize::from(content_n > MAX);
        (4 + content_h) as u16
    }
}

/// Structure représentant le résumé parsé d'un test Unity.
#[derive(Debug)]
struct UnityTestSummary {
    total_tests: usize,
    failures: usize,
    #[allow(dead_code)]
    ignored: usize,
    failed_tests: Vec<(String, Option<String>)>, // (test_name, optional error message)
}

/// Analyse le stdout d'un test Unity et extrait les résultats.
/// Format attendu : "file:line:test_name:PASS" ou "file:line:test_name:FAIL" suivi optionnellement d'une ligne d'erreur.
fn parse_unity_output(stdout: &str) -> Option<UnityTestSummary> {
    let lines: Vec<&str> = stdout.trim().lines().collect();
    if lines.is_empty() {
        return None;
    }

    let mut failed_tests: Vec<(String, Option<String>)> = Vec::new();
    let mut i = 0;

    // Parser chaque ligne de test
    while i < lines.len() {
        let line = lines[i];
        // Format : "file.c:123:test_name:PASS" ou "file.c:123:test_name:FAIL"
        if let Some(colon_pos) = line.rfind(':') {
            let status = &line[colon_pos + 1..];
            if status == "PASS" || status == "FAIL" {
                // Extraire le nom du test (avant le dernier ':')
                if let Some(prev_colon) = line[..colon_pos].rfind(':') {
                    let test_name = line[prev_colon + 1..colon_pos].to_string();
                    if status == "FAIL" {
                        // Chercher la ligne d'erreur optionnelle (ligne suivante)
                        let error_msg = if i + 1 < lines.len() && !lines[i + 1].contains(':') {
                            i += 1;
                            Some(lines[i].trim().to_string())
                        } else {
                            None
                        };
                        failed_tests.push((test_name, error_msg));
                    }
                }
            }
        }
        i += 1;
    }

    // Chercher la ligne récapitulative "N Tests M Failures X Ignored"
    for line in lines.iter().rev() {
        if line.contains("Tests") && line.contains("Failures") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let mut total = 0;
            let mut failures = 0;
            let mut ignored = 0;

            for (idx, &part) in parts.iter().enumerate() {
                if part == "Tests" && idx > 0 {
                    if let Ok(n) = parts[idx - 1].parse() {
                        total = n;
                    }
                }
                if part == "Failures" && idx > 0 {
                    if let Ok(n) = parts[idx - 1].parse() {
                        failures = n;
                    }
                }
                if part == "Ignored" && idx > 0 {
                    if let Ok(n) = parts[idx - 1].parse() {
                        ignored = n;
                    }
                }
            }

            if total > 0 {
                return Some(UnityTestSummary {
                    total_tests: total,
                    failures,
                    ignored,
                    failed_tests,
                });
            }
        }
    }

    None
}

/// Rendu du panneau résultat de compilation/exécution.
pub fn render_run_result(f: &mut Frame, area: Rect, result: &RunResult, exercise: &Exercise) {
    use crate::constants::{MSG_COMPILE_ERROR, MSG_TESTS_FAILED, MSG_TIMEOUT, MSG_WRONG_OUTPUT};
    let (title, title_color) = if result.success {
        (format!("✓ SUCCÈS ({}ms)", result.duration_ms), C_SUCCESS)
    } else if result.compile_error {
        (MSG_COMPILE_ERROR.to_string(), C_DANGER)
    } else if result.timeout {
        (MSG_TIMEOUT.to_string(), C_DANGER)
    } else {
        let is_test = matches!(exercise.validation.mode, ValidationMode::Test);
        if is_test {
            (MSG_TESTS_FAILED.to_string(), C_DANGER)
        } else {
            (MSG_WRONG_OUTPUT.to_string(), C_DANGER)
        }
    };

    let color = title_color;
    let mut lines: Vec<Line> = Vec::new();

    if result.success {
        for line in result.stdout.lines() {
            lines.push(Line::from(span!(C_SUCCESS; "{}", line)));
        }
    } else if result.compile_error {
        for line in result.stderr.lines().take(5) {
            lines.push(Line::from(span!(C_DANGER; "{}", line)));
        }
        if let Some(hint) = &result.gcc_hint {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("→ ", Style::default().fg(C_INFO)),
                Span::styled(hint.as_str(), Style::default().fg(C_INFO)),
            ]));
        }
    } else if result.timeout {
        lines.push(Line::from("Dépassement de 10s — boucle infinie ?"));
    } else if matches!(exercise.validation.mode, ValidationMode::Test) {
        // Mode test Unity — parser et afficher les résultats
        if let Some(summary) = parse_unity_output(&result.stdout) {
            // Header du résumé
            let pass_count = summary.total_tests.saturating_sub(summary.failures);
            let summary_line = format!(
                "{} Tests — {} PASS ✓ — {} FAIL ✗",
                summary.total_tests, pass_count, summary.failures
            );
            lines.push(Line::from(span!(C_ACCENT; "{}", summary_line)));
            lines.push(Line::raw(""));

            // Lister les tests échoués
            if !summary.failed_tests.is_empty() {
                for (test_name, error_msg) in &summary.failed_tests {
                    lines.push(Line::from(vec![Span::styled(
                        format!("✗ {}", test_name),
                        Style::default().fg(C_DANGER),
                    )]));
                    if let Some(err) = error_msg {
                        lines.push(Line::from(span!(C_TEXT_DIM; "  {}", err)));
                    }
                }
                lines.push(Line::raw(""));
            }

            // Résumé final Unity (brut)
            for line in result.stdout.lines().rev() {
                if line.contains("Tests") && line.contains("Failures") {
                    lines.push(Line::from(span!(C_TEXT_DIM; "{}", line)));
                    break;
                }
            }
        } else {
            // Afficher le stdout brut si parsing échoue
            for line in result.stdout.lines() {
                lines.push(Line::from(span!(C_DANGER; "{}", line)));
            }
        }
    } else if let Some(expected) = &exercise.validation.expected_output {
        const MAX: usize = 5;
        let exp_lines: Vec<&str> = expected.trim().lines().collect();
        let got_lines: Vec<&str> = result.stdout.trim().lines().collect();

        // Outer frame (titre seulement)
        let outer = Block::bordered()
            .border_type(BorderType::Rounded)
            .style(Style::default().bg(C_BG))
            .title(span!(Style::default().fg(color).add_modifier(Modifier::BOLD); "{}", title))
            .border_style(Style::default().fg(color));
        let inner = outer.inner(area);
        f.render_widget(outer, area);

        // Split côte à côte
        let [left_area, right_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(inner);

        // Card "attendu"
        let exp_content: Vec<Line> = exp_lines
            .iter()
            .take(MAX)
            .map(|l| Line::from(span!(C_SUCCESS; "{}", l)))
            .chain(if exp_lines.len() > MAX {
                vec![Line::from(
                    span!(C_TEXT_DIM; "… +{}", exp_lines.len() - MAX),
                )]
            } else {
                vec![]
            })
            .collect();
        f.render_widget(
            Paragraph::new(exp_content).block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .style(Style::default().bg(C_BG))
                    .title(span!(Style::default().fg(C_SUCCESS).add_modifier(Modifier::BOLD); "attendu"))
                    .border_style(Style::default().fg(C_SUCCESS)),
            ),
            left_area,
        );

        // Card "actuel"
        let got_content: Vec<Line> = got_lines
            .iter()
            .take(MAX)
            .map(|l| Line::from(span!(C_DANGER; "{}", l)))
            .chain(if got_lines.len() > MAX {
                vec![Line::from(
                    span!(C_TEXT_DIM; "… +{}", got_lines.len() - MAX),
                )]
            } else {
                vec![]
            })
            .collect();
        f.render_widget(
            Paragraph::new(got_content).block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .style(Style::default().bg(C_BG))
                    .title(
                        span!(Style::default().fg(C_DANGER).add_modifier(Modifier::BOLD); "actuel"),
                    )
                    .border_style(Style::default().fg(C_DANGER)),
            ),
            right_area,
        );

        return;
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(C_BG))
        .title(span!(Style::default().fg(color).add_modifier(Modifier::BOLD); "{}", title))
        .border_style(Style::default().fg(color));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

/// Construit un `Vec<Span>` pour une liste de keybinds `(key, desc)`.
/// Espacement automatique entre chaque paire. Utilisé par les status bars watch et piscine.
pub fn render_keybinds(
    binds: &[(&'static str, &'static str)],
    key_style: Style,
    dim: Style,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(binds.len() * 3);
    for (key, desc) in binds {
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(*key, key_style));
        spans.push(Span::styled(*desc, dim));
    }
    spans
}

/// Construit le message droit de la status bar watch.
/// Retourne `(message, style)`.
pub fn render_status_right_watch(state: &AppState) -> (String, Style) {
    if state.ex.consecutive_failures > 0 {
        (
            format!("✗ {}", state.ex.consecutive_failures),
            Style::default().fg(C_DANGER),
        )
    } else {
        let due = state.due_count();
        if due > 0 {
            (
                format!("révision: {}j", due),
                Style::default().fg(C_WARNING),
            )
        } else {
            (String::new(), Style::default())
        }
    }
}

/// Construit le message droit de la status bar piscine.
/// Retourne `(message, style)`.
pub fn render_status_right_piscine(state: &AppState) -> (String, Style) {
    if state.piscine.fail_count > 0 {
        (
            format!("✗ {}", state.piscine.fail_count),
            Style::default().fg(C_DANGER),
        )
    } else {
        (String::new(), Style::default())
    }
}
