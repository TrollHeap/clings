//! TEA message types, commands, and display item types for the TUI layer.

use std::fmt;

/// Item dans la liste d'affichage de l'overlay `[l]` — header de chapitre ou exercice.
#[derive(Debug, Clone)]
pub enum ListDisplayItem {
    ChapterHeader {
        chapter_number: u8,
        title: &'static str,
        exercise_count: usize,
        done_count: usize,
    },
    Exercise {
        exercise_index: usize,
    },
}

/// Messages du dispatching événementiel TEA (Terminal Event Architecture).
/// Traités par `update_watch()` / `update_piscine()`.
#[derive(Debug)]
pub enum Msg {
    /// Touche clavier pressée (modifiée, avec shift/ctrl/alt).
    Key(ratatui::crossterm::event::KeyEvent),
    /// Fichier utilisateur sauvegardé — compile+valide automatiquement en watch mode.
    FileChanged,
    /// Tick timer périodique (ex. pour les animations, mise à jour du timer piscine).
    Tick,
    /// Terminal redimensionné — layout sera recalculé au prochain dessin.
    Resize(u16, u16),
}

impl fmt::Display for Msg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Msg::Key(ke) => write!(f, "Key({:?})", ke.code),
            Msg::FileChanged => write!(f, "FileChanged"),
            Msg::Tick => write!(f, "Tick"),
            Msg::Resize(w, h) => write!(f, "Resize({w}x{h}"),
        }
    }
}

/// Commande sémantique produite par un `KeyEvent` hors overlay.
/// Voir `key_to_cmd` pour le mapping KeyCode → Command.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Quit,
    CompileRun,
    ShowHint,
    ToggleSolution,
    OpenSearch,
    OpenList,
    OpenVisualizer,
    OpenLibsys,
    ShowHelp,
    NavNext,
    NavPrev,
    ScrollDown,
    ScrollUp,
}

/// Overlay exclusif actif — un seul à la fois (sauf modaux `nav_confirm` et `success`).
#[derive(Default, Debug, Clone, PartialEq)]
pub enum ActiveOverlay {
    #[default]
    None,
    Help,
    List,
    Search,
    Solution,
    Visualizer,
    Libsys,
}
