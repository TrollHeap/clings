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

// ── Badge backgrounds ──────────────────────────────────────────────────
const C_BADGE_COMPLETE: Color = Color::Rgb(30, 45, 30);
const C_BADGE_FIXBUG: Color = Color::Rgb(45, 25, 30);
const C_BADGE_FILLBLANK: Color = Color::Rgb(45, 35, 20);
const C_BADGE_REFACTOR: Color = Color::Rgb(20, 40, 42);

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

/// Badge coloré pour le type d'exercice.
pub fn exercise_type_badge(t: ExerciseType) -> Span<'static> {
    match t {
        ExerciseType::Complete => span!(
            Style::default()
                .fg(C_SUCCESS)
                .bg(C_BADGE_COMPLETE)
                .add_modifier(Modifier::BOLD);
            " COMPLETE "
        ),
        ExerciseType::FixBug => span!(
            Style::default()
                .fg(C_DANGER)
                .bg(C_BADGE_FIXBUG)
                .add_modifier(Modifier::BOLD);
            " FIX_BUG "
        ),
        ExerciseType::FillBlank => span!(
            Style::default()
                .fg(C_WARNING)
                .bg(C_BADGE_FILLBLANK)
                .add_modifier(Modifier::BOLD);
            " FILL_BLANK "
        ),
        ExerciseType::Refactor => span!(
            Style::default()
                .fg(C_INFO)
                .bg(C_BADGE_REFACTOR)
                .add_modifier(Modifier::BOLD);
            " REFACTOR "
        ),
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
    if has_help && state.overlay.help_active {
        return Some(Line::styled("[Esc/?] fermer", dim));
    }
    if state.overlay.solution_active {
        return Some(Line::styled("[Esc/s] fermer solution", dim));
    }
    if state.overlay.list_active {
        return Some(Line::styled(
            "[↑↓/jk] nav  [Tab/S-Tab] chapitre  [Entrée] aller  [Esc/l/q] fermer",
            dim,
        ));
    }
    if state.overlay.search_active {
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

    let (desc_area, result_area_opt) = if let Some(result) = &state.run_result {
        let exercise = &state.exercises[state.current_index];
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

/// Rendu du panneau résultat de compilation/exécution.
pub fn render_run_result(
    f: &mut Frame,
    area: Rect,
    result: &RunResult,
    exercise: &crate::models::Exercise,
) {
    use crate::constants::{MSG_COMPILE_ERROR, MSG_TESTS_FAILED, MSG_TIMEOUT, MSG_WRONG_OUTPUT};
    let (title, title_color) = if result.success {
        (format!("✓ SUCCÈS ({}ms)", result.duration_ms), C_SUCCESS)
    } else if result.compile_error {
        (MSG_COMPILE_ERROR.to_string(), C_DANGER)
    } else if result.timeout {
        (MSG_TIMEOUT.to_string(), C_DANGER)
    } else {
        let is_test = matches!(
            exercise.validation.mode,
            ValidationMode::Test | ValidationMode::Both
        );
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

/// Calcule la taille du popup visualiseur en fonction du contenu.
pub fn popup_size_for_vis(step: &crate::models::VisStep) -> (u16, u16) {
    let max_rows = step.stack.len().max(step.heap.len()).max(1) as u16;
    let h_pct = (max_rows * 7 + 25).clamp(40, 72);
    let is_dual = !step.heap.is_empty() || (step.call_frames.len() >= 2 && !step.arrows.is_empty());
    let w_pct = if is_dual { 78u16 } else { 65u16 };
    (w_pct, h_pct)
}

// ── Helpers rendu ASCII box-drawing pour le visualiseur ───────────────────────

/// Détecte si une valeur représente un pointeur (pour le style C_ACCENT).
fn is_pointer_value(val: &str) -> bool {
    val.starts_with("──▶") || val.starts_with("→") || val.starts_with("0x")
}

/// Calcule les largeurs de colonnes nom/valeur. Min 4, cap valeur à 20.
fn vis_col_widths(vars: &[crate::models::VisVar]) -> (usize, usize) {
    let name_w = vars
        .iter()
        .map(|v| v.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);
    let val_w = vars
        .iter()
        .map(|v| v.value.chars().count())
        .max()
        .unwrap_or(0)
        .max(4)
        .min(20);
    (name_w, val_w)
}

/// Remplit `s` à droite jusqu'à `w` caractères d'affichage.
fn pad_to(s: String, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s
    } else {
        let mut r = s;
        for _ in n..w {
            r.push(' ');
        }
        r
    }
}

/// Tronque `s` à `w` caractères (ajoute … si tronqué).
fn trunc_to(s: &str, w: usize) -> String {
    if s.chars().count() > w {
        let mut r: String = s.chars().take(w.saturating_sub(1)).collect();
        r.push('…');
        r
    } else {
        s.to_string()
    }
}

/// Header d'une section : ╭─ TITLE ───────────────────╮
fn vis_section_header(
    title: &str,
    title_color: Color,
    name_w: usize,
    val_w: usize,
) -> Line<'static> {
    // Largeur totale ligne : name_w + val_w + 11
    // Inner (entre ╭ et ╮) : name_w + val_w + 9
    // ╭─ (2) + " TITLE " (title_len) + ─×n + ╮ (1) = name_w + val_w + 11
    // n = name_w + val_w + 8 - title_len
    let title_display = format!(" {} ", title);
    let title_len = title_display.chars().count();
    let inner_w = name_w + val_w + 9;
    let n_dashes = inner_w.saturating_sub(1 + title_len);
    Line::from(vec![
        Span::styled("╭─".to_string(), Style::default().fg(C_BORDER)),
        Span::styled(
            title_display,
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}╮", "─".repeat(n_dashes)),
            Style::default().fg(C_BORDER),
        ),
    ])
}

/// Ligne de données : │  name     │  value     │
fn vis_section_data_row(var: &crate::models::VisVar, name_w: usize, val_w: usize) -> Line<'static> {
    let name_d = pad_to(trunc_to(&var.name, name_w), name_w);
    let val_d = pad_to(trunc_to(&var.value, val_w), val_w);
    let val_color = if is_pointer_value(&var.value) {
        C_ACCENT
    } else {
        C_SUBTEXT
    };
    Line::from(vec![
        Span::styled("│  ".to_string(), Style::default().fg(C_BORDER)),
        Span::styled(name_d, Style::default().fg(C_TEXT)),
        Span::styled("  │  ".to_string(), Style::default().fg(C_BORDER)),
        Span::styled(val_d, Style::default().fg(val_color)),
        Span::styled("  │".to_string(), Style::default().fg(C_BORDER)),
    ])
}

/// Ligne vide (padding dans le layout côte-à-côte).
fn vis_section_empty_row(name_w: usize, val_w: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled("│  ".to_string(), Style::default().fg(C_BORDER)),
        Span::raw(" ".repeat(name_w)),
        Span::styled("  │  ".to_string(), Style::default().fg(C_BORDER)),
        Span::raw(" ".repeat(val_w)),
        Span::styled("  │".to_string(), Style::default().fg(C_BORDER)),
    ])
}

/// Séparateur entre deux lignes de données : ├──────┼──────┤
fn vis_section_sep(name_w: usize, val_w: usize) -> Line<'static> {
    Line::from(Span::styled(
        format!("├{}┼{}┤", "─".repeat(name_w + 4), "─".repeat(val_w + 4)),
        Style::default().fg(C_BORDER),
    ))
}

/// Footer de section : ╰──────┴──────╯
fn vis_section_footer(name_w: usize, val_w: usize) -> Line<'static> {
    Line::from(Span::styled(
        format!("╰{}┴{}╯", "─".repeat(name_w + 4), "─".repeat(val_w + 4)),
        Style::default().fg(C_BORDER),
    ))
}

/// Construit les lignes d'une section mémoire (header + n_rows + footer).
fn render_vis_section(
    title: &str,
    title_color: Color,
    vars: &[crate::models::VisVar],
    name_w: usize,
    val_w: usize,
    n_rows: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(2 * n_rows + 2);
    lines.push(vis_section_header(title, title_color, name_w, val_w));
    for i in 0..n_rows {
        if i > 0 {
            lines.push(vis_section_sep(name_w, val_w));
        }
        if let Some(var) = vars.get(i) {
            lines.push(vis_section_data_row(var, name_w, val_w));
        } else {
            lines.push(vis_section_empty_row(name_w, val_w));
        }
    }
    lines.push(vis_section_footer(name_w, val_w));
    lines
}

/// Section alignée avec `Option<&VisVar>` — les `None` produisent des lignes vides.
fn build_aligned_section(
    title: &str,
    title_color: Color,
    vars: &[Option<&crate::models::VisVar>],
    name_w: usize,
    val_w: usize,
) -> Vec<Line<'static>> {
    let n_rows = vars.len().max(1);
    let mut lines = Vec::with_capacity(2 * n_rows + 2);
    lines.push(vis_section_header(title, title_color, name_w, val_w));
    for (i, opt) in vars.iter().enumerate() {
        if i > 0 {
            lines.push(vis_section_sep(name_w, val_w));
        }
        match opt {
            Some(var) => lines.push(vis_section_data_row(var, name_w, val_w)),
            None => lines.push(vis_section_empty_row(name_w, val_w)),
        }
    }
    lines.push(vis_section_footer(name_w, val_w));
    lines
}

/// Layout multi-frames : frame appelée (gauche) ──▶ frame appelante (droite).
/// Activé quand `call_frames.len() >= 2` ET `arrows.len() > 0`.
/// Les vars de `stack` sont partitionnées en :
///   - `called_vars` : vars sources des flèches (frame active, ex. swap)
///   - `calling_vars` : vars cibles des flèches (frame appelante, ex. main)
fn render_vis_frames(step: &crate::models::VisStep) -> Vec<Line<'static>> {
    use std::collections::{HashMap, HashSet};

    let target_names: HashSet<&str> = step.arrows.iter().map(|a| a.to.as_str()).collect();
    let arrow_map: HashMap<&str, &str> = step
        .arrows
        .iter()
        .map(|a| (a.from.as_str(), a.to.as_str()))
        .collect();

    let called_vars: Vec<&crate::models::VisVar> = step
        .stack
        .iter()
        .filter(|v| !target_names.contains(v.name.as_str()))
        .collect();
    let calling_vars: Vec<&crate::models::VisVar> = step
        .stack
        .iter()
        .filter(|v| target_names.contains(v.name.as_str()))
        .collect();

    let called_label = step
        .call_frames
        .last()
        .map(|f| f.function_name.as_str())
        .unwrap_or("swap");
    let calling_label = step
        .call_frames
        .first()
        .map(|f| f.function_name.as_str())
        .unwrap_or("main");

    let sn_w = called_vars
        .iter()
        .map(|v| v.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);
    let sv_w = called_vars
        .iter()
        .map(|v| v.value.chars().count())
        .max()
        .unwrap_or(0)
        .max(4)
        .min(20);
    let hn_w = calling_vars
        .iter()
        .map(|v| v.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);
    let hv_w = calling_vars
        .iter()
        .map(|v| v.value.chars().count())
        .max()
        .unwrap_or(0)
        .max(4)
        .min(20);

    let n_rows = called_vars.len().max(calling_vars.len()).max(1);

    // Pour chaque ligne de called_vars, trouver la var cible dans calling_vars (ou None).
    let right_aligned: Vec<Option<&crate::models::VisVar>> = (0..n_rows)
        .map(|i| {
            called_vars
                .get(i)
                .and_then(|cv| arrow_map.get(cv.name.as_str()))
                .and_then(|tgt| calling_vars.iter().find(|v| v.name == *tgt).copied())
        })
        .collect();
    let arrow_rows: Vec<bool> = right_aligned.iter().map(|o| o.is_some()).collect();

    let called_owned: Vec<crate::models::VisVar> =
        called_vars.iter().map(|v| (*v).clone()).collect();
    let left = render_vis_section(called_label, C_WARNING, &called_owned, sn_w, sv_w, n_rows);
    let right = build_aligned_section(calling_label, C_SUCCESS, &right_aligned, hn_w, hv_w);

    let total_lines = 2 * n_rows + 1;
    left.into_iter()
        .zip(right)
        .enumerate()
        .map(|(i, (l, r))| {
            let has_arrow = i > 0
                && i < total_lines
                && i % 2 == 1
                && arrow_rows.get((i - 1) / 2).copied().unwrap_or(false);
            let gap: Span<'static> = if has_arrow {
                Span::styled(" ──▶ ", Style::default().fg(C_ACCENT))
            } else {
                Span::raw("     ")
            };
            let mut spans = l.spans;
            spans.push(gap);
            spans.extend(r.spans);
            Line::from(spans)
        })
        .collect()
}

/// Génère les lignes du tableau mémoire ASCII pour un step.
/// Dispatch : multi-frames > stack+heap côte-à-côte > stack seul.
fn render_vis_table(step: &crate::models::VisStep) -> Vec<Line<'static>> {
    if step.call_frames.len() >= 2 && !step.arrows.is_empty() {
        return render_vis_frames(step);
    }
    let (sn_w, sv_w) = vis_col_widths(&step.stack);
    if step.heap.is_empty() {
        let n_rows = step.stack.len().max(1);
        render_vis_section("STACK", C_SUCCESS, &step.stack, sn_w, sv_w, n_rows)
    } else {
        let (hn_w, hv_w) = vis_col_widths(&step.heap);
        let n_rows = step.stack.len().max(step.heap.len()).max(1);
        let left = render_vis_section("STACK", C_SUCCESS, &step.stack, sn_w, sv_w, n_rows);
        let right = render_vis_section("HEAP", C_TEAL, &step.heap, hn_w, hv_w, n_rows);
        left.into_iter()
            .zip(right)
            .map(|(l, r)| {
                let mut spans = l.spans;
                spans.push(Span::raw("     "));
                spans.extend(r.spans);
                Line::from(spans)
            })
            .collect()
    }
}

/// Overlay visualiseur mémoire (partagé entre watch et piscine).
pub fn render_visualizer_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    let exercise = &state.exercises[state.current_index];
    let steps = &exercise.visualizer.steps;

    if steps.is_empty() {
        return;
    }

    let step_idx = state.overlay.vis_step.min(steps.len() - 1);
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

    lines.extend(render_vis_table(step));

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
    let query_display = format!("{}{}", state.overlay.search_query, cursor);
    let overlay_title = if state.overlay.search_subject_filter {
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
        .overlay
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

    let count = state.overlay.search_results.len();
    let list_title = if state.overlay.search_query.is_empty() {
        format!(" {count} exercices ")
    } else {
        format!(" {count} résultats ")
    };

    let mut list_state = ListState::default();
    if !state.overlay.search_results.is_empty() {
        list_state.select(Some(state.overlay.search_selected));
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

/// Overlay liste d'exercices — navigation j/k, Tab/Shift-Tab chapitres, Enter pour jump.
pub fn render_list_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    use crate::tui::app::ListDisplayItem;

    let popup = centered_popup(area, 10, 8);
    f.render_widget(Clear, popup);

    // Split: list (fill) | hint bar (1 line)
    let [list_area, hint_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(popup);

    let items: Vec<ListItem> = state
        .overlay
        .list_display_items
        .iter()
        .map(|item| match item {
            ListDisplayItem::ChapterHeader {
                chapter_number,
                title,
                exercise_count,
                done_count,
            } => ListItem::new(line![
                span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD);
                    "── Ch.{} : {} [{}/{}] ──", chapter_number, title, done_count, exercise_count),
            ]),
            ListDisplayItem::Exercise { exercise_index } => {
                let i = *exercise_index;
                let ex = &state.exercises[i];
                let stars = difficulty_stars(ex.difficulty);
                let color = difficulty_color(ex.difficulty);
                let done_marker = if state.completed.get(i).copied().unwrap_or(false) {
                    "✓"
                } else {
                    " "
                };
                let current_marker = if i == state.current_index { "►" } else { " " };
                let mastery = state.mastery_map.get(&ex.subject).copied().unwrap_or(0.0);
                let title_end = ex
                    .title
                    .char_indices()
                    .nth(30)
                    .map(|(bi, _)| bi)
                    .unwrap_or(ex.title.len());
                let subj_end = ex
                    .subject
                    .char_indices()
                    .nth(16)
                    .map(|(bi, _)| bi)
                    .unwrap_or(ex.subject.len());
                ListItem::new(line![
                    span!(C_SUCCESS; "{}", done_marker),
                    span!(C_ACCENT; "{}", current_marker),
                    span!(C_TEXT; " {:<32}", &ex.title[..title_end]),
                    span!(C_SUBTEXT; "{:<18}", &ex.subject[..subj_end]),
                    span!(Style::default().fg(color); "{}", stars),
                    span!(C_OVERLAY; " [{:.1}]", mastery),
                ])
            }
        })
        .collect();

    let total = state.exercises.len();
    let done = state.completed.iter().filter(|&&c| c).count();
    let list_title = format!(" Exercices [{}/{}] ", done, total);

    let mut list_state = ListState::default();
    list_state.select(Some(state.overlay.list_selected));

    f.render_stateful_widget(
        List::new(items)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD); "{}", list_title))
                    .style(Style::default().bg(C_SURFACE))
                    .border_style(Style::default().fg(C_BORDER)),
            )
            .highlight_style(Style::default().bg(C_OVERLAY).add_modifier(Modifier::BOLD)),
        list_area,
        &mut list_state,
    );

    // Hint bar
    f.render_widget(
        Paragraph::new(
            "[↑↓/jk] nav  [Tab/S-Tab] chapitre  [g/G] début/fin  [Entrée] aller  [Esc/l/q] fermer",
        )
        .style(Style::default().fg(C_TEXT_DIM)),
        hint_area,
    );
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
    if state.consecutive_failures > 0 {
        (
            format!("✗ {}", state.consecutive_failures),
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
    if state.piscine_fail_count > 0 {
        (
            format!("✗ {}", state.piscine_fail_count),
            Style::default().fg(C_DANGER),
        )
    } else {
        (String::new(), Style::default())
    }
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
        ("[l]", "Liste des exercices"),
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

/// Modal de succès — affiché après validation correcte, attend confirmation avant d'avancer.
pub fn render_success_overlay(f: &mut Frame, area: Rect) {
    let popup = centered_popup(area, 35, 28);
    f.render_widget(Clear, popup);

    let lines = vec![
        Line::raw(""),
        Line::from(
            span!(Style::default().fg(C_SUCCESS).add_modifier(Modifier::BOLD); "  ✓  L'exercice est validé !"),
        ),
        Line::raw(""),
        line![
            span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD); "  [Entrée]"),
            span!(C_TEXT_DIM; "   Exercice suivant →"),
        ],
        line![
            span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD); "  [Échap] "),
            span!(C_TEXT_DIM; "   Rester ici"),
        ],
        Line::raw(""),
    ];

    f.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(span!(Style::default().fg(C_SUCCESS).add_modifier(Modifier::BOLD); "Succès"))
                .style(Style::default().bg(C_SURFACE))
                .border_style(Style::default().fg(C_SUCCESS)),
        ),
        popup,
    );
}

/// Renders an opaque background to prevent terminal transparency.
pub fn render_opaque_background(f: &mut Frame, area: Rect) {
    f.render_widget(Block::default().style(Style::default().bg(C_BG)), area);
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
