//! tmux integration configuration.

use serde::{Deserialize, Serialize};

/// Configuration intégration tmux.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct TmuxConfig {
    /// Activer/désactiver la création automatique d'un pane tmux pour l'éditeur.
    pub enabled: bool,
}

impl Default for TmuxConfig {
    fn default() -> Self {
        TmuxConfig { enabled: true }
    }
}
