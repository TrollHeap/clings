//! Fonctions TUI partagées entre ui_watch, ui_piscine, ui_list et ui_stats.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Wrap,
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

/// Calcule la taille du popup visualiseur en fonction du contenu.
pub fn popup_size_for_vis(step: &crate::models::VisStep) -> (u16, u16) {
    let max_rows = step.stack.len().max(step.heap.len()).max(1) as u16;
    // Table: n_rows + 3 lignes (border top + header + border bottom)
    // Overhead overlay: ~12 lignes (dots, label, explication, nav, spacers)
    let expl_lines: u16 = if step.explanation.is_empty() {
        0
    } else {
        step.explanation
            .split(". ")
            .filter(|s| !s.is_empty())
            .count() as u16
    };
    // inner_needed = fixed(7) + frame_h(max_rows+3) + expl(n+1 si n>0) + popup_border(2)
    let inner_h = 12 + max_rows + if expl_lines > 0 { expl_lines + 1 } else { 0 };
    // Échelle en %, référence ~32 lignes terminal
    let h_pct = (inner_h * 100 / 32).clamp(45, 82);
    let is_dual = !step.heap.is_empty() || (step.call_frames.len() >= 2 && !step.arrows.is_empty());
    let w_pct = if is_dual { 82u16 } else { 65u16 };
    (w_pct, h_pct)
}

// ── Helpers visualiseur mémoire ───────────────────────────────────────────────

/// Détecte si une valeur représente un pointeur (pour le style C_ACCENT).
pub(crate) fn is_pointer_value(val: &str) -> bool {
    val.starts_with("──▶") || val.starts_with("→") || val.starts_with("0x")
}

/// Calcule les largeurs de colonnes nom/valeur. Min 4, cap valeur à 20.
pub(crate) fn vis_col_widths(vars: &[crate::models::VisVar]) -> (usize, usize) {
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
        .clamp(4, 20);
    (name_w, val_w)
}

/// Overlay visualiseur mémoire (partagé entre watch et piscine).
pub fn render_visualizer_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    use crate::tui::ui_visualizer::MemVisualizer;

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
    f.render_widget(
        MemVisualizer {
            step,
            step_idx,
            total_steps: steps.len(),
        },
        popup,
    );
}

/// Overlay de recherche fuzzy (touche `/` depuis watch).
pub fn render_search_overlay(f: &mut Frame, area: Rect, state: &mut AppState) {
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

    if !state.overlay.search_results.is_empty() {
        state
            .overlay
            .search_list_state
            .select(Some(state.overlay.search_selected));
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
        &mut state.overlay.search_list_state,
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
pub fn render_list_overlay(f: &mut Frame, area: Rect, state: &mut AppState) {
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

    state
        .overlay
        .list_list_state
        .select(Some(state.overlay.list_selected));

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
        &mut state.overlay.list_list_state,
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

/// Modal de confirmation avant de changer d'exercice.
///
/// `going_next` : true = suivant, false = précédent.
pub fn render_nav_confirm_overlay(f: &mut Frame, area: Rect, going_next: bool) {
    let popup = centered_popup(area, 38, 32);
    f.render_widget(Clear, popup);

    let direction = if going_next { "suivant" } else { "précédent" };
    let lines = vec![
        Line::raw(""),
        Line::styled(
            format!("→ exercice {direction}"),
            Style::default().fg(C_WARNING).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(
            "Votre code actuel sera remplacé.",
            Style::default().fg(C_TEXT_DIM),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "[o] ",
                Style::default().fg(C_SUCCESS).add_modifier(Modifier::BOLD),
            ),
            Span::styled("confirmer   ", Style::default().fg(C_TEXT)),
            Span::styled(
                "[autre] ",
                Style::default().fg(C_DANGER).add_modifier(Modifier::BOLD),
            ),
            Span::styled("rester", Style::default().fg(C_TEXT)),
        ]),
    ];

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(span!(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD); "Changer d'exercice ?"))
                    .style(Style::default().bg(C_SURFACE))
                    .border_style(Style::default().fg(C_WARNING)),
            )
            .alignment(ratatui::layout::Alignment::Center),
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
