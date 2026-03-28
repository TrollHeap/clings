//! Palette Catppuccin Mocha, constantes de dimensions et helpers visuels purs.
//! Aucune dépendance sur `Frame` ou `AppState`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui_macros::span;

use crate::models::{Difficulty, ExerciseType};

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
        ExerciseType::LibraryExport => span!(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(166, 227, 161))
                .add_modifier(Modifier::BOLD);
            " LIBSYS "
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mastery_bar_string_width() {
        let bar = mastery_bar_string(2.5, 10);
        assert_eq!(bar.chars().count(), 10);
    }
}
