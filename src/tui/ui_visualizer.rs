//! Widget MemVisualizer — overlay visualiseur mémoire avec rendu Ratatui Canvas.
//!
//! Chaque frame mémoire est un `Table` Ratatui dans un `Block::bordered()`.
//! Les flèches inter-frames sont dessinées via Canvas + `ctx.print()`.

use std::collections::{HashMap, HashSet};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{canvas::Canvas, Block, BorderType, Paragraph, Row, Table, Widget},
};

use crate::models::{VisStep, VisVar};
use crate::tui::common::{
    is_pointer_value, vis_col_widths, C_ACCENT, C_BORDER, C_SUBTEXT, C_SUCCESS, C_SURFACE, C_TEAL,
    C_TEXT, C_TEXT_DIM, C_WARNING,
};

/// Widget principal du visualiseur mémoire.
pub struct MemVisualizer<'a> {
    pub step: &'a VisStep,
    pub step_idx: usize,
    pub total_steps: usize,
}

impl Widget for MemVisualizer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let step = self.step;

        // ── Bloc extérieur ────────────────────────────────────────────────────
        let title = format!("Visualiseur {}/{}", self.step_idx + 1, self.total_steps);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                title,
                Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
            ))
            .style(Style::default().bg(C_SURFACE))
            .border_style(Style::default().fg(C_BORDER));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 6 || inner.width < 20 {
            return;
        }

        // ── Dots de navigation ─────────────────────────────────────────────────
        let mut dots = String::with_capacity(self.total_steps * 4);
        for i in 0..self.total_steps {
            if i > 0 {
                dots.push(' ');
            }
            dots.push_str(if i == self.step_idx { "●" } else { "○" });
        }

        // ── Label ─────────────────────────────────────────────────────────────
        let label = if !step.step_label.is_empty() {
            step.step_label.as_str()
        } else {
            step.label.as_str()
        };

        // ── Explication ────────────────────────────────────────────────────────
        let expl_parts: Vec<&str> = if step.explanation.is_empty() {
            vec![]
        } else {
            step.explanation
                .split(". ")
                .filter(|s| !s.is_empty())
                .collect()
        };
        let expl_lines = expl_parts.len() as u16;

        // ── Hauteur de la zone principale (frame_h = content + borders + header) ──
        let is_multi = step.call_frames.len() >= 2 && !step.arrows.is_empty();
        let max_data_rows = if is_multi {
            let target_names: HashSet<&str> = step.arrows.iter().map(|a| a.to.as_str()).collect();
            let called_count = step
                .stack
                .iter()
                .filter(|v| !target_names.contains(v.name.as_str()))
                .count();
            let calling_count = step.arrows.len();
            called_count.max(calling_count).max(1)
        } else {
            step.stack.len().max(step.heap.len()).max(1)
        } as u16;
        // border_top(1) + header_row(1) + data_rows(n) + border_bottom(1) = n + 3
        let frame_h = max_data_rows + 3;

        // ── Layout vertical ────────────────────────────────────────────────────
        let mut constraints: Vec<Constraint> = vec![
            Constraint::Length(1),       // 0 : dots
            Constraint::Length(1),       // 1 : spacer
            Constraint::Length(1),       // 2 : label
            Constraint::Length(1),       // 3 : spacer
            Constraint::Length(frame_h), // 4 : zone principale
            Constraint::Length(1),       // 5 : spacer
        ];
        let expl_idx = constraints.len(); // 6 si explication présente
        if expl_lines > 0 {
            constraints.push(Constraint::Length(expl_lines));
            constraints.push(Constraint::Length(1)); // spacer avant nav
        }
        constraints.push(Constraint::Length(1)); // nav hint (dernier)
        let nav_idx = constraints.len() - 1;

        let areas = Layout::vertical(constraints).split(inner);

        // ── Rendu ─────────────────────────────────────────────────────────────
        Line::styled(dots, Style::default().fg(C_WARNING)).render(areas[0], buf);
        Line::styled(
            label.to_owned(),
            Style::default().add_modifier(Modifier::BOLD),
        )
        .render(areas[2], buf);

        render_main_area(step, frame_h, areas[4], buf);

        if expl_lines > 0 {
            let expl_text: Vec<Line> = expl_parts
                .iter()
                .map(|p| Line::styled(p.to_owned(), Style::default().fg(C_TEXT_DIM)))
                .collect();
            Paragraph::new(expl_text).render(areas[expl_idx], buf);
        }

        Line::styled(
            "[←] préc   [→] suiv   [v] fermer",
            Style::default().fg(C_TEXT_DIM),
        )
        .render(areas[nav_idx], buf);
    }
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

fn render_main_area(step: &VisStep, frame_h: u16, area: Rect, buf: &mut Buffer) {
    let is_multi = step.call_frames.len() >= 2 && !step.arrows.is_empty();
    if is_multi {
        render_multi_frame(step, frame_h, area, buf);
    } else if !step.heap.is_empty() {
        render_stack_heap(step, area, buf);
    } else {
        render_frame_card("STACK", C_SUCCESS, &step.stack, area, buf);
    }
}

// ── Cas stack + heap ──────────────────────────────────────────────────────────

fn render_stack_heap(step: &VisStep, area: Rect, buf: &mut Buffer) {
    const SEP: u16 = 3;
    let frame_w = area.width.saturating_sub(SEP) / 2;
    let left = Rect::new(area.x, area.y, frame_w, area.height);
    let right_x = area.x + frame_w + SEP;
    let right_w = area.width.saturating_sub(frame_w).saturating_sub(SEP);
    let right = Rect::new(right_x, area.y, right_w, area.height);
    render_frame_card("STACK", C_SUCCESS, &step.stack, left, buf);
    render_frame_card("HEAP", C_TEAL, &step.heap, right, buf);
}

// ── Cas multi-frames ──────────────────────────────────────────────────────────

/// frame appelée (gauche) ──Canvas──▶ frame appelante (droite).
fn render_multi_frame(step: &VisStep, frame_h: u16, area: Rect, buf: &mut Buffer) {
    const CONNECTOR_W: u16 = 9;

    let target_names: HashSet<&str> = step.arrows.iter().map(|a| a.to.as_str()).collect();
    let arrow_map: HashMap<&str, &str> = step
        .arrows
        .iter()
        .map(|a| (a.from.as_str(), a.to.as_str()))
        .collect();

    let called_vars: Vec<VisVar> = step
        .stack
        .iter()
        .filter(|v| !target_names.contains(v.name.as_str()))
        .cloned()
        .collect();
    let calling_vars: Vec<VisVar> = step
        .stack
        .iter()
        .filter(|v| target_names.contains(v.name.as_str()))
        .cloned()
        .collect();

    let n_rows = called_vars.len().max(calling_vars.len());
    let arrow_rows: Vec<bool> = (0..n_rows)
        .map(|i| {
            called_vars
                .get(i)
                .and_then(|cv| arrow_map.get(cv.name.as_str()))
                .is_some()
        })
        .collect();

    let called_label = step
        .call_frames
        .last()
        .map(|f| f.function_name.as_str())
        .unwrap_or("fn");
    let calling_label = step
        .call_frames
        .first()
        .map(|f| f.function_name.as_str())
        .unwrap_or("main");

    // Largeurs égales : chaque frame prend (total - CONNECTOR) / 2
    let frame_w = area.width.saturating_sub(CONNECTOR_W) / 2;
    let left_area = Rect::new(area.x, area.y, frame_w, frame_h);
    let conn_area = Rect::new(area.x + frame_w, area.y, CONNECTOR_W, frame_h);
    let right_w = area
        .width
        .saturating_sub(frame_w)
        .saturating_sub(CONNECTOR_W);
    let right_area = Rect::new(area.x + frame_w + CONNECTOR_W, area.y, right_w, frame_h);

    render_frame_card(called_label, C_WARNING, &called_vars, left_area, buf);
    render_arrow_canvas(arrow_rows, conn_area, buf);
    render_frame_card(calling_label, C_SUCCESS, &calling_vars, right_area, buf);
}

// ── Widgets atomiques ─────────────────────────────────────────────────────────

/// Frame mémoire : `Block::bordered()` + `Table` (var | val).
fn render_frame_card(
    title: &str,
    title_color: ratatui::style::Color,
    vars: &[VisVar],
    area: Rect,
    buf: &mut Buffer,
) {
    let (name_w_usize, val_w_usize) = vis_col_widths(vars);
    let name_w = name_w_usize as u16;
    let val_w = val_w_usize as u16;

    let rows: Vec<Row> = vars
        .iter()
        .map(|v| {
            let val_style = if is_pointer_value(&v.value) {
                Style::default().fg(C_ACCENT)
            } else {
                Style::default().fg(C_SUBTEXT)
            };
            Row::new(vec![
                Span::styled(v.name.as_str(), Style::default().fg(C_TEXT)),
                Span::styled(v.value.as_str(), val_style),
            ])
        })
        .collect();

    let header = Row::new(["var", "val"]).style(
        Style::default()
            .fg(title_color)
            .add_modifier(Modifier::BOLD),
    );

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(C_BORDER))
        .style(Style::default().bg(C_SURFACE));

    let widths = [Constraint::Length(name_w), Constraint::Length(val_w)];

    Table::new(rows, widths)
        .block(block)
        .header(header)
        .column_spacing(2)
        .render(area, buf);
}

/// Connecteur Canvas entre deux frames avec flèches horizontales.
///
/// Utilise `ctx.print()` pour des flèches "────▶" visibles (pas braille).
/// `arrow_rows[i]` = true → flèche centrée sur la ligne de données i.
///
/// Coordonnées Canvas : `y_bounds([h, 0])` → y=0 en haut, y=h en bas.
/// Ligne de données i : y_center = border(1) + header(1) + i + 0.5 = 2.5 + i
fn render_arrow_canvas(arrow_rows: Vec<bool>, area: Rect, buf: &mut Buffer) {
    let w = area.width as f64;
    let h = area.height as f64;

    // Texte flèche qui remplit le connecteur : "────────▶" (area.width chars)
    let arrow_w = area.width as usize;
    let arrow_text = format!("{}▶", "─".repeat(arrow_w.saturating_sub(1)));

    Canvas::default()
        .background_color(C_SURFACE)
        .x_bounds([0.0, w])
        .y_bounds([h, 0.0]) // y=0 en haut
        .paint(move |ctx| {
            for (i, &has_arrow) in arrow_rows.iter().enumerate() {
                if !has_arrow {
                    continue;
                }
                // offset: border_top(1) + header_row(1) + i + centre(0.5)
                let y = 2.5 + i as f64;
                if y >= h {
                    continue;
                }
                ctx.print(
                    0.0,
                    y,
                    Span::styled(arrow_text.clone(), Style::default().fg(C_ACCENT)),
                );
            }
        })
        .render(area, buf);
}
