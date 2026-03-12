//! Display module — re-exports all TUI rendering functions.

mod annales;
mod exercise;
mod keybinds;
mod list;
mod progress;
mod stats;
mod visualizer;

pub use annales::*;
pub use exercise::*;
pub use keybinds::*;
pub use list::*;
pub use progress::*;
pub use stats::*;
pub(crate) use visualizer::handle_esc_sequence;
pub use visualizer::*;

use std::sync::OnceLock;

use colored::{ColoredString, Colorize};

use crate::constants::{
    ANSI_CLEAR_SCREEN, HEADER_WIDTH, MASTERY_BAR_GREEN_THRESHOLD, MASTERY_BAR_YELLOW_THRESHOLD,
    MASTERY_MAX,
};
use crate::models::Difficulty;

pub use crate::models::AnnaleSession;

static GCC_RE: OnceLock<regex::Regex> = OnceLock::new();

pub(super) fn gcc_re() -> &'static regex::Regex {
    GCC_RE.get_or_init(|| {
        regex::Regex::new(r"^[^:]+:(\d+):\d+: (error|warning|note): (.+)$")
            .expect("static regex pattern is valid")
    })
}

/// Render difficulty as colored star string.
pub fn difficulty_stars(d: Difficulty) -> ColoredString {
    match d {
        Difficulty::Easy => "★☆☆☆☆".green(),
        Difficulty::Medium => "★★☆☆☆".yellow(),
        Difficulty::Hard => "★★★☆☆".red(),
        Difficulty::Advanced => "★★★★☆".magenta(),
        Difficulty::Expert => "★★★★★".cyan(),
    }
}

/// Create a visual mastery bar with block characters.
pub fn mastery_bar(score: f64) -> String {
    let filled = (score / MASTERY_MAX * 10.0).round() as usize;
    let empty = 10 - filled.min(10);
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    let colored = if score >= MASTERY_BAR_GREEN_THRESHOLD {
        bar.green().to_string()
    } else if score >= MASTERY_BAR_YELLOW_THRESHOLD {
        bar.yellow().to_string()
    } else {
        bar.red().to_string()
    };
    format!("{} {:.1}", colored, score)
}

/// Clear screen and move cursor to top.
pub fn clear_screen() {
    print!("{ANSI_CLEAR_SCREEN}");
}

/// Direction d'une touche fléchée ANSI (séquence ESC [ A/B/C/D).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArrowKey {
    Up,
    Down,
    Right,
    Left,
}

/// Parse une séquence de 3 octets ESC-[ en `ArrowKey`, si reconnue.
pub(super) fn try_parse_arrow(buf: &[u8]) -> Option<ArrowKey> {
    match buf {
        [0x1b, b'[', b'A'] => Some(ArrowKey::Up),
        [0x1b, b'[', b'B'] => Some(ArrowKey::Down),
        [0x1b, b'[', b'C'] => Some(ArrowKey::Right),
        [0x1b, b'[', b'D'] => Some(ArrowKey::Left),
        _ => None,
    }
}

/// Colorise un pourcentage entier : vert ≥ 75, jaune ≥ 25, rouge < 25.
pub(super) fn color_pct(pct: u32) -> ColoredString {
    let s = format!("{}%", pct);
    if pct >= 75 {
        s.green()
    } else if pct >= 25 {
        s.yellow()
    } else {
        s.red()
    }
}

/// Confirmation after `clings export`.
pub fn show_export_done(path: Option<&std::path::Path>, count: usize) {
    match path {
        Some(p) => println!(
            "  {} {} sujets exportés → {}",
            "✓".bold().green(),
            count,
            p.display().to_string().bold()
        ),
        None => eprintln!(
            "  {} {} sujets exportés (stdout)",
            "✓".bold().green(),
            count
        ),
    }
}

/// Confirmation after `clings import`.
pub fn show_import_done(count: usize, overwrite: bool) {
    let mode = if overwrite {
        "écrasement"
    } else {
        "fusion max"
    };
    println!(
        "  {} {} sujets importés ({})",
        "✓".bold().green(),
        count,
        mode.dimmed()
    );
}

// ─── Box-drawing helpers ────────────────────────────────────────────

/// Visible text width between the ║ chars (HEADER_WIDTH - 2 - 2 side spaces = 52).
pub(super) const INNER_W: usize = HEADER_WIDTH - 4;

pub(super) fn hr() -> String {
    "─".repeat(HEADER_WIDTH)
}

pub(super) fn header_box(title: &str) -> String {
    let pad = HEADER_WIDTH.saturating_sub(title.chars().count() + 6);
    let left = pad / 2;
    let right = pad - left;
    format!(
        "╔{} {} {}╗",
        "═".repeat(left + 1),
        title,
        "═".repeat(right + 1)
    )
}

pub(super) fn footer_box() -> String {
    format!("╚{}╝", "═".repeat(HEADER_WIDTH - 2))
}

/// Word-wrap `text` to at most `width` visible chars per line.
pub(super) fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Display the main banner.
pub(super) fn show_banner() {
    println!("  {}", header_box("CLINGS").bold().cyan());
    let subtitle = "NSY103 — Programmation système Linux";
    let subtitle_len = subtitle.chars().count();
    let inner = HEADER_WIDTH - 2; // 54 chars between ║ and ║
    let total_pad = inner.saturating_sub(subtitle_len);
    let lp = total_pad / 2;
    let rp = total_pad - lp;
    println!(
        "  {}{}{}{}{}",
        "║".cyan(),
        " ".repeat(lp),
        subtitle.dimmed(),
        " ".repeat(rp),
        "║".cyan()
    );
    println!("  {}", footer_box().cyan());
    println!();
}

/// Report authoring validation results.
pub fn show_authoring_result(path: &std::path::Path, errors: &[crate::authoring::ValidationError]) {
    println!();
    if errors.is_empty() {
        println!(
            "  {} {}",
            "✓".bold().green(),
            path.display().to_string().bold()
        );
        println!(
            "  {}",
            "Validation réussie — aucune erreur détectée.".green()
        );
    } else {
        println!(
            "  {} {} ({} erreur(s))",
            "✗".bold().red(),
            path.display().to_string().bold(),
            errors.len()
        );
        for e in errors {
            println!("    {} {}", "•".red(), e);
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Difficulty;

    // ── mastery_bar ─────────────────────────────────────────────────────

    #[test]
    fn mastery_bar_zero_score() {
        let result = mastery_bar(0.0);
        // Score 0 → 0 filled blocks, 10 empty, then " 0.0"
        assert!(
            result.ends_with(" 0.0"),
            "should end with ' 0.0', got: {result}"
        );
        // The raw bar before coloring has 10 empty blocks
        assert!(result.contains("░░░░░░░░░░"), "should have 10 empty blocks");
        assert!(!result.contains('█'), "should have no filled blocks");
    }

    #[test]
    fn mastery_bar_full_score() {
        let result = mastery_bar(5.0);
        assert!(
            result.ends_with(" 5.0"),
            "should end with ' 5.0', got: {result}"
        );
        assert!(
            result.contains("██████████"),
            "should have 10 filled blocks"
        );
        assert!(!result.contains('░'), "should have no empty blocks");
    }

    #[test]
    fn mastery_bar_mid_score() {
        // score 2.5 → 5 filled, 5 empty
        let result = mastery_bar(2.5);
        assert!(
            result.ends_with(" 2.5"),
            "should end with ' 2.5', got: {result}"
        );
        assert!(result.contains("█████"), "should contain 5 filled blocks");
        assert!(result.contains("░░░░░"), "should contain 5 empty blocks");
    }

    #[test]
    fn mastery_bar_score_above_threshold_green() {
        // score >= 4.0 renders green — we verify the numeric label
        let result = mastery_bar(4.0);
        assert!(
            result.ends_with(" 4.0"),
            "should end with ' 4.0', got: {result}"
        );
    }

    // ── wrap_text ────────────────────────────────────────────────────────

    #[test]
    fn wrap_text_short_line_unchanged() {
        let lines = wrap_text("hello world", 80);
        assert_eq!(lines, vec!["hello world"]);
    }

    #[test]
    fn wrap_text_empty_input() {
        let lines = wrap_text("", 80);
        assert!(lines.is_empty(), "empty input should produce no lines");
    }

    #[test]
    fn wrap_text_splits_at_width() {
        // "one two" = 7 chars; width=4 forces split after "one"
        let lines = wrap_text("one two", 4);
        assert_eq!(lines, vec!["one", "two"]);
    }

    #[test]
    fn wrap_text_long_sentence() {
        // width=11: "alpha beta"=10 fits, "gamma delta"=11 fits, "epsilon"=7 fits
        let text = "alpha beta gamma delta epsilon";
        let lines = wrap_text(text, 11);
        assert_eq!(lines[0], "alpha beta");
        assert_eq!(lines[1], "gamma delta");
        assert_eq!(lines[2], "epsilon");
    }

    #[test]
    fn wrap_text_single_word_longer_than_width() {
        // A word longer than width still goes on its own line
        let lines = wrap_text("superlongword", 5);
        assert_eq!(lines, vec!["superlongword"]);
    }

    // ── hr ──────────────────────────────────────────────────────────────

    #[test]
    fn hr_has_correct_width() {
        let line = hr();
        // hr() = "─".repeat(HEADER_WIDTH) where HEADER_WIDTH = 56
        // Each "─" is one Unicode char (3 bytes), visible width 1
        assert_eq!(line.chars().count(), HEADER_WIDTH);
    }

    // ── header_box ──────────────────────────────────────────────────────

    #[test]
    fn header_box_contains_title() {
        let title = "TEST";
        let result = header_box(title);
        assert!(
            result.contains(title),
            "header_box should contain the title"
        );
        assert!(result.starts_with('╔'), "should start with ╔");
        assert!(result.ends_with('╗'), "should end with ╗");
    }

    #[test]
    fn header_box_empty_title() {
        let result = header_box("");
        assert!(result.starts_with('╔'));
        assert!(result.ends_with('╗'));
    }

    // ── footer_box ──────────────────────────────────────────────────────

    #[test]
    fn footer_box_structure() {
        let result = footer_box();
        assert!(result.starts_with('╚'), "should start with ╚");
        assert!(result.ends_with('╝'), "should end with ╝");
        // Inner content is (HEADER_WIDTH - 2) '═' chars
        let inner: String = result.chars().skip(1).take_while(|&c| c == '═').collect();
        assert_eq!(inner.chars().count(), HEADER_WIDTH - 2);
    }

    // ── difficulty_stars ────────────────────────────────────────────────

    #[test]
    fn difficulty_stars_contains_correct_count() {
        // Each variant must contain the right number of filled (★) and empty (☆) stars
        let easy = difficulty_stars(Difficulty::Easy).to_string();
        let medium = difficulty_stars(Difficulty::Medium).to_string();
        let hard = difficulty_stars(Difficulty::Hard).to_string();
        let advanced = difficulty_stars(Difficulty::Advanced).to_string();
        let expert = difficulty_stars(Difficulty::Expert).to_string();

        assert_eq!(easy.chars().filter(|&c| c == '★').count(), 1);
        assert_eq!(easy.chars().filter(|&c| c == '☆').count(), 4);

        assert_eq!(medium.chars().filter(|&c| c == '★').count(), 2);
        assert_eq!(medium.chars().filter(|&c| c == '☆').count(), 3);

        assert_eq!(hard.chars().filter(|&c| c == '★').count(), 3);
        assert_eq!(hard.chars().filter(|&c| c == '☆').count(), 2);

        assert_eq!(advanced.chars().filter(|&c| c == '★').count(), 4);
        assert_eq!(advanced.chars().filter(|&c| c == '☆').count(), 1);

        assert_eq!(expert.chars().filter(|&c| c == '★').count(), 5);
        assert_eq!(expert.chars().filter(|&c| c == '☆').count(), 0);
    }

    #[test]
    fn difficulty_stars_total_five_per_variant() {
        for d in [
            Difficulty::Easy,
            Difficulty::Medium,
            Difficulty::Hard,
            Difficulty::Advanced,
            Difficulty::Expert,
        ] {
            let s = difficulty_stars(d).to_string();
            let star_count = s.chars().filter(|&c| c == '★' || c == '☆').count();
            assert_eq!(
                star_count, 5,
                "Difficulty {d:?} should have exactly 5 star chars"
            );
        }
    }
}
