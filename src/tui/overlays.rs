//! Overlays Ratatui — `render_*_overlay` partagés entre les modes watch et piscine.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;
use ratatui_macros::{line, span, vertical};

use crate::models::{Exercise, VisStep, VisVar};
use crate::tui::app::{AppState, ListDisplayItem};
use crate::tui::style::{
    difficulty_color, difficulty_stars, C_ACCENT, C_BG, C_BORDER, C_DANGER, C_OVERLAY, C_SUBTEXT,
    C_SUCCESS, C_SURFACE, C_TEXT, C_TEXT_DIM, C_WARNING,
};
use crate::tui::ui_visualizer::MemVisualizer;

/// Calcule la zone d'un popup centré avec des marges en pourcentage.
pub fn centered_popup(area: Rect, margin_v_pct: u16, margin_h_pct: u16) -> Rect {
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

/// Calcule la taille du popup visualiseur en fonction du contenu.
pub fn popup_size_for_vis(step: &VisStep) -> (u16, u16) {
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
pub fn is_pointer_value(val: &str) -> bool {
    val.starts_with("──▶") || val.starts_with("→") || val.starts_with("0x")
}

/// Calcule les largeurs de colonnes nom/valeur. Min 4, cap valeur à 20.
pub fn vis_col_widths(vars: &[VisVar]) -> (usize, usize) {
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

/// Fond opaque pour éviter la transparence du terminal.
pub fn render_opaque_background(f: &mut Frame, area: Rect) {
    f.render_widget(Block::default().style(Style::default().bg(C_BG)), area);
}

/// Overlay visualiseur mémoire (partagé entre watch et piscine).
pub fn render_visualizer_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    let Some(exercise) = state.ex.exercises.get(state.ex.current_index) else {
        return;
    };
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
            .ex
            .exercises
            .get(state.ex.current_index)
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
        .filter_map(|&idx| state.ex.exercises.get(idx))
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

    let mut search_list_state = state.overlay.search_list_state;
    if !state.overlay.search_results.is_empty() {
        search_list_state.select(Some(state.overlay.search_selected));
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
        &mut search_list_state,
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
pub fn render_solution_overlay(f: &mut Frame, area: Rect, exercise: &Exercise) {
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
                let Some(ex) = state.ex.exercises.get(i) else {
                    return ListItem::new(Line::raw(""));
                };
                let stars = difficulty_stars(ex.difficulty);
                let color = difficulty_color(ex.difficulty);
                let done_marker = if state.ex.completed.get(i).copied().unwrap_or(false) {
                    "✓"
                } else {
                    " "
                };
                let current_marker = if i == state.ex.current_index {
                    "►"
                } else {
                    " "
                };
                let mastery = state
                    .progress
                    .mastery_map
                    .get(&ex.subject)
                    .copied()
                    .unwrap_or(0.0);
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

    let total = state.ex.exercises.len();
    let done = state.ex.completed.iter().filter(|&&c| c).count();
    let list_title = format!(" Exercices [{}/{}] ", done, total);

    let mut list_list_state = state.overlay.list_list_state;
    list_list_state.select(Some(state.overlay.list_selected));

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
        &mut list_list_state,
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
        ("[b]", "Portfolio libsys"),
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
            .alignment(Alignment::Center),
        popup,
    );
}

pub fn render_quit_confirm_overlay(f: &mut Frame, area: Rect) {
    let popup = centered_popup(area, 38, 28);
    f.render_widget(Clear, popup);

    let lines = vec![
        Line::raw(""),
        Line::styled(
            "La session sera interrompue.",
            Style::default().fg(C_TEXT_DIM),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "[o] ",
                Style::default().fg(C_SUCCESS).add_modifier(Modifier::BOLD),
            ),
            Span::styled("retour au menu   ", Style::default().fg(C_TEXT)),
            Span::styled(
                "[autre] ",
                Style::default().fg(C_DANGER).add_modifier(Modifier::BOLD),
            ),
            Span::styled("continuer", Style::default().fg(C_TEXT)),
        ]),
    ];

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(span!(
                        Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD);
                        "Quitter la session ?"
                    ))
                    .style(Style::default().bg(C_SURFACE))
                    .border_style(Style::default().fg(C_WARNING)),
            )
            .alignment(Alignment::Center),
        popup,
    );
}
