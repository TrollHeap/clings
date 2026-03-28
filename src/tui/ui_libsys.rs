//! Overlay portfolio libsys [b] — affiche l'état de la bibliothèque personnelle.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;
use ratatui_macros::{line, span};

use crate::tui::app::AppState;
use crate::tui::common::{C_ACCENT, C_BORDER, C_SUCCESS, C_SURFACE, C_TEXT, C_TEXT_DIM, C_WARNING};
use crate::tui::overlays::centered_popup;

/// Rendu de l'overlay portfolio libsys.
pub fn render_libsys_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    let popup = centered_popup(area, 60, 80);
    f.render_widget(Clear, popup);

    let modules = &state.overlay.libsys_portfolio;

    let mut lines: Vec<Line> = vec![Line::raw("")];

    if modules.is_empty() {
        lines.push(Line::styled(
            "  Aucune donnée — libsys_path non configuré.",
            Style::default().fg(C_TEXT_DIM),
        ));
    } else {
        let total_fns: usize = modules.iter().map(|m| m.functions.len()).sum();
        lines.push(line![
            span!(Style::default().fg(C_TEXT_DIM); "  "),
            span!(Style::default().fg(C_TEXT); "{} module{} · {} fonction{} exportée{}",
                modules.len(), if modules.len() > 1 { "s" } else { "" },
                total_fns, if total_fns > 1 { "s" } else { "" },
                if total_fns > 1 { "s" } else { "" }
            ),
        ]);
        lines.push(Line::raw(""));

        for module in modules {
            let fn_count = module.functions.len();
            let has_unlock = module.unlock_subject.is_some();

            // En-tête du module
            let module_color = if fn_count > 0 { C_SUCCESS } else { C_TEXT_DIM };
            let lock_indicator = if has_unlock && fn_count == 0 {
                " 🔒"
            } else {
                ""
            };
            lines.push(line![
                span!(Style::default().fg(module_color).add_modifier(Modifier::BOLD);
                    "  {}{}", module.name, lock_indicator),
                span!(Style::default().fg(C_TEXT_DIM); "  [{} fn]", fn_count),
            ]);

            if module.functions.is_empty() {
                if let Some(ref subject) = module.unlock_subject {
                    lines.push(line![
                        span!(Style::default().fg(C_TEXT_DIM); "    requis : "),
                        span!(Style::default().fg(C_WARNING); "{}", subject),
                    ]);
                } else {
                    lines.push(Line::styled("    (vide)", Style::default().fg(C_TEXT_DIM)));
                }
            } else {
                for func in &module.functions {
                    let hash_short = if func.commit_hash.len() >= 7 {
                        &func.commit_hash[..7]
                    } else {
                        &func.commit_hash
                    };
                    lines.push(line![
                        span!(Style::default().fg(C_SUCCESS); "    ✓ "),
                        span!(Style::default().fg(C_TEXT); "{:<22}", func.name),
                        span!(Style::default().fg(C_TEXT_DIM); " {}", hash_short),
                    ]);
                }
            }
            lines.push(Line::raw(""));
        }
    }

    lines.push(Line::styled(
        "  Appuyez sur n'importe quelle touche pour fermer",
        Style::default().fg(C_TEXT_DIM),
    ));

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(Span::styled(
                        " libsys — Portfolio ",
                        Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
                    ))
                    .style(Style::default().bg(C_SURFACE))
                    .border_style(Style::default().fg(C_BORDER)),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}
