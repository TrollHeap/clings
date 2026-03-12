//! Interactive memory visualizer — step-through stack/heap diagram for pointer exercises.

use std::io::Write;

use colored::Colorize;

use crate::constants::{ANSI_ESC_BYTE, HEADER_WIDTH};
use crate::models::{Exercise, VisVar};

use super::{footer_box, header_box, try_parse_arrow, wrap_text, ArrowKey, INNER_W};

/// Handles accumulating ESC sequences and navigating visualizer steps.
///
/// Returns `Some(())` if the key was consumed (caller should `return None`),
/// `None` if the key was not an ESC sequence and should be processed normally.
pub(crate) fn handle_esc_sequence(
    key: u8,
    escape_buf: &mut Vec<u8>,
    vis_active: bool,
    vis_step: &mut usize,
    vis_lines: &mut usize,
    visualizer_steps_len: usize,
    show_vis: &mut impl FnMut(usize) -> usize,
) -> Option<()> {
    if !escape_buf.is_empty() {
        escape_buf.push(key);
        // Invalid sequence: ESC followed by something other than '[' → clear and process normally
        if escape_buf.len() == 2 && escape_buf[1] != b'[' {
            escape_buf.clear();
        } else {
            if escape_buf.len() == 3 {
                let seq = std::mem::take(escape_buf);
                if vis_active {
                    let n = visualizer_steps_len;
                    match try_parse_arrow(seq.as_slice()) {
                        Some(ArrowKey::Right) => {
                            *vis_step = (*vis_step + 1).min(n.saturating_sub(1));
                            print!("\x1b[{}A\x1b[J", *vis_lines);
                            let _ = std::io::stdout().flush();
                            *vis_lines = show_vis(*vis_step);
                        }
                        Some(ArrowKey::Left) => {
                            *vis_step = vis_step.saturating_sub(1);
                            print!("\x1b[{}A\x1b[J", *vis_lines);
                            let _ = std::io::stdout().flush();
                            *vis_lines = show_vis(*vis_step);
                        }
                        _ => {}
                    }
                }
            }
            return Some(());
        }
    }
    if key == ANSI_ESC_BYTE {
        escape_buf.push(key);
        return Some(());
    }
    None
}

/// Width of each column in the two-column memory visualizer layout.
const COL_W: usize = 26;

/// Render a single variable line.
fn fmt_var(v: &VisVar) -> String {
    format!("{}: {}", v.name, v.value)
}

/// Render a row of the visualizer with two equal columns (plain strings, colored inside).
/// Both args must be plain (no ANSI) so format! can count visible chars correctly.
fn vis_row(left: &str, right: &str) {
    let lp = format!("{:<COL_W$}", left).green();
    let rp = format!("{:<COL_W$}", right).cyan();
    println!("  {} {} {}{}", "║".yellow(), lp, rp, "║".yellow());
}

/// Like vis_row but right column is dimmed (e.g. heap vide).
fn vis_row_dim_right(left: &str, right: &str) {
    let lp = format!("{:<COL_W$}", left).green();
    let rp = format!("{:<COL_W$}", right).dimmed();
    println!("  {} {} {}{}", "║".yellow(), lp, rp, "║".yellow());
}

/// Render one visualizer step to stdout and return the number of lines printed.
///
/// Caller uses the count to erase and redraw on navigation:
/// `print!("\x1b[{n}A\x1b[J")`.
fn render_step(step: usize, total: usize, s: &crate::models::VisStep) -> usize {
    let mut lines = 0usize;

    let title = format!("Visualiseur — {}/{}", step + 1, total);
    println!("  {}", header_box(&title).yellow());
    lines += 1;

    // Step progress dots — printed char-by-char to avoid ANSI padding issues
    print!("  {} ", "║".yellow());
    let mut visible_len = 0usize;
    for i in 0..total {
        if i > 0 {
            print!(" ");
            visible_len += 1;
        }
        if i == step {
            print!("{}", "●".yellow());
        } else {
            print!("{}", "○".dimmed());
        }
        visible_len += 1;
    }
    let step_info = format!("  Etape {}/{}", step + 1, total);
    print!("{}", step_info);
    visible_len += step_info.len();
    let pad = INNER_W.saturating_sub(visible_len);
    print!("{}", " ".repeat(pad));
    println!(" {}", "║".yellow());
    lines += 1;

    // Label row — prefer step_label (more descriptive), fallback to label
    let label_plain = if !s.step_label.is_empty() {
        s.step_label.clone()
    } else {
        s.label.clone()
    };
    let padded_label = format!("{:<INNER_W$}", label_plain);
    println!(
        "  {} {} {}",
        "║".yellow(),
        padded_label.bold(),
        "║".yellow()
    );
    lines += 1;

    // Separator
    println!(
        "  {}",
        format!("╠{}╣", "═".repeat(HEADER_WIDTH - 2)).yellow()
    );
    lines += 1;

    // Column headers (plain strings → colored inside vis_row)
    vis_row("STACK", "HEAP");
    lines += 1;

    // Variables
    let max_rows = s.stack.len().max(s.heap.len()).max(1);
    for i in 0..max_rows {
        let left = s.stack.get(i).map(fmt_var).unwrap_or_default();
        if s.heap.is_empty() && i == 0 {
            vis_row_dim_right(&left, "(vide)");
        } else {
            let right = s.heap.get(i).map(fmt_var).unwrap_or_default();
            vis_row(&left, &right);
        }
    }
    lines += max_rows;

    // Explanation separator
    println!(
        "  {}",
        format!("╠{}╣", "═".repeat(HEADER_WIDTH - 2)).yellow()
    );
    lines += 1;

    // Explanation — word-wrapped, padded, then dimmed
    const WRAP_W: usize = HEADER_WIDTH - 6; // 50
    if !s.explanation.is_empty() {
        let exp_lines = wrap_text(&s.explanation, WRAP_W);
        for line in &exp_lines {
            let padded = format!("{:<INNER_W$}", line);
            println!("  {} {} {}", "║".yellow(), padded.dimmed(), "║".yellow());
        }
        lines += exp_lines.len();
    }

    // Navigation hints — ASCII arrows to avoid raw-mode multi-byte issues
    let nav = "[<] prec   [>] suiv   [v] fermer";
    let padded_nav = format!("{:<INNER_W$}", nav);
    println!(
        "  {} {} {}",
        "║".yellow(),
        padded_nav.dimmed(),
        "║".yellow()
    );
    lines += 1;

    println!("  {}", footer_box().yellow());
    lines += 1;

    println!();
    lines += 1;

    lines
}

/// Display a visualizer step for the given exercise.
/// Returns the number of terminal lines printed, so the caller can erase them
/// with `\x1b[{n}A\x1b[J` before redrawing on navigation.
pub fn show_visualizer(exercise: &Exercise, step: usize) -> usize {
    let steps = &exercise.visualizer.steps;
    if steps.is_empty() {
        return 0;
    }
    let step = step.min(steps.len() - 1);
    render_step(step, steps.len(), &steps[step])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_var_nominal() {
        let v = VisVar {
            name: "x".to_string(),
            value: "42".to_string(),
        };
        assert_eq!(fmt_var(&v), "x: 42");
    }

    #[test]
    fn fmt_var_empty_fields() {
        let v = VisVar {
            name: String::new(),
            value: String::new(),
        };
        assert_eq!(fmt_var(&v), ": ");
    }
}
