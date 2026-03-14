//! Fonctions TUI partagées entre ui_watch, ui_piscine, ui_list et ui_stats.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui::Frame;
use ratatui_macros::{line, span, vertical};

use crate::models::{Difficulty, ExerciseType, ValidationMode};
use crate::runner::RunResult;
use crate::tui::app::AppState;

/// Largeur minimale du terminal pour afficher la sidebar de progression.
pub const BODY_SIDEBAR_THRESHOLD: u16 = 110;
/// Largeur fixe de la sidebar de progression (colonnes).
pub const SIDEBAR_WIDTH: u16 = 38;
/// Séparateur horizontal (36 × ─) — const str évite l'allocation par frame.
pub const SEPARATOR: &str = "────────────────────────────────────";

/// Barre pleine (10 × █) — tranche statique pour mastery_bar sans allocation.
const FULL_BAR: &str = "██████████";
/// Barre vide (10 × ░) — tranche statique pour mastery_bar sans allocation.
const EMPTY_BAR: &str = "░░░░░░░░░░";

// ── Palette Catppuccin Mocha ──────────────────────────────────────────────────
pub const C_BG: Color = Color::Rgb(30, 30, 46); // Base
pub const C_SURFACE: Color = Color::Rgb(24, 24, 37); // Mantle
pub const C_BORDER: Color = Color::Rgb(69, 71, 90); // Surface1
pub const C_TEXT_DIM: Color = Color::Rgb(147, 153, 178); // Overlay2
pub const C_ACCENT: Color = Color::Rgb(137, 180, 250); // Blue
pub const C_OVERLAY: Color = Color::Rgb(108, 112, 134); // Overlay0
pub const C_SUBTEXT: Color = Color::Rgb(186, 194, 222); // Subtext1
pub const C_TEXT: Color = Color::Rgb(205, 214, 244); // Text

// ── Couleurs sémantiques ───────────────────────────────────────────────────────
pub const C_SUCCESS: Color = Color::Rgb(166, 227, 161); // Green
pub const C_WARNING: Color = Color::Rgb(250, 179, 135); // Peach
pub const C_DANGER: Color = Color::Rgb(243, 139, 168); // Red
pub const C_INFO: Color = Color::Rgb(137, 220, 235); // Sky
pub const C_MAUVE: Color = Color::Rgb(203, 166, 247); // Mauve (Advanced)
pub const C_TEAL: Color = Color::Rgb(148, 226, 213); // Teal (Expert, heap)
pub const C_YELLOW: Color = Color::Rgb(249, 226, 175); // Yellow (streak)

/// Étoiles pleines (★ × 5) — tranche statique pour étoiles de difficulté sans allocation.
const FULL_STARS: &str = "★★★★★";
/// Étoiles vides (☆ × 5) — tranche statique pour étoiles de difficulté sans allocation.
const EMPTY_STARS: &str = "☆☆☆☆☆";

/// Badge coloré pour le stage d'échafaudage courant (S0–S4).
/// Couleur croissante : S0 neutre → S4 mauve+bold.
pub fn stage_badge(stage: u8) -> Span<'static> {
    match stage {
        0 => span!(C_TEXT_DIM; "[S0]"),
        1 => span!(C_WARNING; "[S1]"),
        2 => span!(C_INFO; "[S2]"),
        3 => span!(C_SUCCESS; "[S3]"),
        _ => span!(Style::default().fg(C_MAUVE).add_modifier(Modifier::BOLD); "[S4]"),
    }
}

/// Retourne `(floor, threshold, next_stage)` pour afficher la progression vers le stage suivant.
/// Retourne `None` si le score est déjà au stage maximum (S4, mastery ≥ 4.0).
/// Seuils alignés avec `runner::mastery_to_stage` : S0<1.0 S1<2.0 S2<3.0 S3<4.0 S4≥4.0.
pub fn next_stage_threshold(score: f64) -> Option<(f64, f64, u8)> {
    if score < 1.0 {
        Some((0.0, 1.0, 1))
    } else if score < 2.0 {
        Some((1.0, 2.0, 2))
    } else if score < 3.0 {
        Some((2.0, 3.0, 3))
    } else if score < 4.0 {
        Some((3.0, 4.0, 4))
    } else {
        None
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
        Difficulty::Easy => C_SUCCESS,
        Difficulty::Medium => C_WARNING,
        Difficulty::Hard => C_DANGER,
        Difficulty::Advanced => C_MAUVE,
        Difficulty::Expert => C_TEAL,
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

/// Ligne d'étoiles colorées : étoiles pleines en couleur de difficulté, vides en C_BORDER.
pub fn difficulty_stars_line(d: Difficulty) -> Line<'static> {
    let (filled, color): (usize, Color) = match d {
        Difficulty::Easy => (1, C_SUCCESS),
        Difficulty::Medium => (2, C_WARNING),
        Difficulty::Hard => (3, C_DANGER),
        Difficulty::Advanced => (4, C_MAUVE),
        Difficulty::Expert => (5, C_TEAL),
    };
    let empty = 5 - filled;
    Line::from(vec![
        Span::styled(&FULL_STARS[..filled * 3], Style::default().fg(color)),
        Span::styled(&EMPTY_STARS[..empty * 3], Style::default().fg(C_BORDER)),
    ])
}

/// Badge coloré pour le type d'exercice.
pub fn exercise_type_badge(t: ExerciseType) -> Span<'static> {
    match t {
        ExerciseType::Complete => {
            span!(Style::default().fg(C_SUCCESS).add_modifier(Modifier::BOLD); " COMPLETE ")
        }
        ExerciseType::FixBug => {
            span!(Style::default().fg(C_DANGER).add_modifier(Modifier::BOLD); " FIX_BUG ")
        }
        ExerciseType::FillBlank => {
            span!(Style::default().fg(C_WARNING).add_modifier(Modifier::BOLD); " FILL_BLANK ")
        }
        ExerciseType::Refactor => {
            span!(Style::default().fg(C_INFO).add_modifier(Modifier::BOLD); " REFACTOR ")
        }
    }
}

/// Couleur gradient pour un score de maîtrise (0.0–5.0).
pub fn mastery_color(score: f64) -> Color {
    if score < 1.0 {
        C_DANGER
    } else if score < 2.5 {
        C_WARNING
    } else if score < 4.0 {
        C_SUCCESS
    } else {
        C_TEAL
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

/// Génère la chaîne barre de maîtrise (█░ format) pour usage dans les cellules de table.
/// Utilise des tranches statiques (max 10) — zéro allocation intermédiaire.
pub fn mastery_bar_string(score: f64, width: usize) -> String {
    debug_assert!(width <= 10, "mastery_bar_string: width > 10 non supporté");
    let filled = (score.clamp(0.0, 5.0) / 5.0 * width as f64).round() as usize;
    let empty = width - filled;
    // Chaque █/░ = 3 octets UTF-8
    let mut s = String::with_capacity(width * 3);
    s.push_str(&FULL_BAR[..filled * 3]);
    s.push_str(&EMPTY_BAR[..empty * 3]);
    s
}

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
    let exercise = &state.exercises[state.current_index];
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

    if state.hint_index > 0 && !exercise.hints.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("── Indices ──", Style::default().fg(C_TEAL)));
        for (i, hint) in exercise.hints[..state.hint_index].iter().enumerate() {
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

    let content_length = lines.len();
    let scroll = state.description_scroll;
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
    if state.compile_pending {
        return Some(Line::styled("⏳ Compilation en cours…", dim));
    }
    if has_help && state.help_active {
        return Some(Line::styled("[Esc/?] fermer", dim));
    }
    if state.solution_active {
        return Some(Line::styled("[Esc/s] fermer solution", dim));
    }
    if state.search_active {
        return Some(Line::styled(
            "[↑↓/jk] nav  [Entrée] aller  [Esc] fermer",
            dim,
        ));
    }
    if let Some(status) = &state.status_msg {
        return Some(Line::styled(status.clone(), dim));
    }
    None
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

    let (desc_area, result_area_opt) = if let Some(result) = &state.run_result {
        let h = run_result_height(result);
        let [desc, res] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(h)]).areas(content_area);
        (desc, Some(res))
    } else {
        (content_area, None)
    };

    render_description_panel(f, desc_area, state);

    if let Some(result_area) = result_area_opt {
        if let Some(result) = &state.run_result {
            let exercise = &state.exercises[state.current_index];
            render_run_result(f, result_area, result, exercise);
        }
    }

    if let Some(sb_area) = sidebar_opt {
        render_sidebar(f, sb_area, state);
    }
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
        (format!("✓ SUCCÈS ({}ms)", result.duration_ms), C_SUCCESS)
    } else if result.compile_error {
        ("✗ ERREUR DE COMPILATION".to_string(), C_DANGER)
    } else if result.timeout {
        ("✗ TIMEOUT".to_string(), C_DANGER)
    } else {
        let is_test = matches!(
            exercise.validation.mode,
            ValidationMode::Test | ValidationMode::Both
        );
        if is_test {
            ("✗ TESTS ÉCHOUÉS".to_string(), C_DANGER)
        } else {
            ("✗ SORTIE INCORRECTE".to_string(), C_DANGER)
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
    } else if result.timeout {
        lines.push(Line::from("Dépassement de 10s — boucle infinie ?"));
    } else if let Some(expected) = &exercise.validation.expected_output {
        let exp_lines: Vec<&str> = expected.trim().lines().collect();
        let got_lines: Vec<&str> = result.stdout.trim().lines().collect();
        let max_len = exp_lines.len().max(got_lines.len());
        for i in 0..max_len.min(4) {
            match (exp_lines.get(i), got_lines.get(i)) {
                (Some(e), Some(g)) if *e == *g => {
                    lines.push(Line::from(span!(C_SUCCESS; "  {}", e)));
                }
                (Some(e), Some(g)) => {
                    lines.push(Line::from(span!(C_DANGER; "- {}", e)));
                    lines.push(Line::from(span!(C_WARNING; "+ {}", g)));
                }
                (Some(e), None) => {
                    lines.push(Line::from(span!(C_DANGER; "- {}", e)));
                }
                (None, Some(g)) => {
                    lines.push(Line::from(span!(C_WARNING; "+ {}", g)));
                }
                (None, None) => {}
            }
        }
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(C_BG))
        .title(span!(Style::default().fg(color).add_modifier(Modifier::BOLD); "{}", title))
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
    lines.push(Line::styled(dots, Style::default().fg(C_WARNING)));
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

    lines.push(line![
        span!(Style::default().fg(C_SUCCESS).add_modifier(Modifier::BOLD); "{:<25}", "STACK"),
        span!(C_OVERLAY; " │ "),
        span!(Style::default().fg(C_TEAL).add_modifier(Modifier::BOLD); "HEAP"),
    ]);

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
        lines.push(line![
            span!(C_SUCCESS; "{:<25}", left),
            span!(C_OVERLAY; " │ "),
            span!(C_TEAL; "{}", right),
        ]);
    }

    lines.push(Line::raw(""));

    if !step.explanation.is_empty() {
        for part in step.explanation.split(". ") {
            lines.push(Line::styled(part, Style::default().fg(C_TEXT_DIM)));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "[←] préc   [→] suiv   [v] fermer",
        Style::default().fg(C_TEXT_DIM),
    ));

    let title = format!("Visualiseur {}/{}", step_idx + 1, steps.len());
    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD); "{}", title))
                    .style(Style::default().bg(C_SURFACE))
                    .border_style(Style::default().fg(C_BORDER)),
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
    let [query_area, results_area, hint_area] = vertical![==3, *=1, ==1].areas(popup);

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
                .border_type(BorderType::Rounded)
                .title(span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD); "{}", overlay_title))
                .style(Style::default().bg(C_SURFACE))
                .border_style(Style::default().fg(C_ACCENT)),
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
            ListItem::new(line![
                span!(C_TEXT; "{:<30}", &ex.title[..title_end]),
                span!(C_SUBTEXT; "{:<18}", &ex.subject[..subj_end]),
                span!(Style::default().fg(color); "{}", stars),
            ])
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
                    .border_type(BorderType::Rounded)
                    .title(list_title)
                    .style(Style::default().bg(C_SURFACE))
                    .border_style(Style::default().fg(C_BORDER)),
            )
            .highlight_style(Style::default().bg(C_OVERLAY).add_modifier(Modifier::BOLD)),
        results_area,
        &mut list_state,
    );

    // Hint bar
    f.render_widget(
        Paragraph::new(
            "[↑↓/jk] nav  [g/G] début/fin  [Entrée] aller  [Tab] filtre sujet  [Esc] fermer",
        )
        .style(Style::default().fg(C_TEXT_DIM)),
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
                    .border_type(BorderType::Rounded)
                    .title(span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD); "Solution — [Esc/s] fermer"))
                    .style(Style::default().bg(C_SURFACE))
                    .border_style(Style::default().fg(C_BORDER)),
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
            lines.push(line![
                span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD); "  {:<10}", key),
                Span::raw("  "),
                span!(C_TEXT_DIM; "{}", *desc),
            ]);
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "  Appuyez sur n'importe quelle touche pour fermer",
        Style::default().fg(C_TEXT_DIM),
    ));

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD); "Aide — raccourcis"))
                    .style(Style::default().bg(C_SURFACE))
                    .border_style(Style::default().fg(C_BORDER)),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mastery_bar_string_width() {
        let bar = mastery_bar_string(2.5, 10);
        assert_eq!(bar.chars().count(), 10);
    }
}
