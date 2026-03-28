//! UI and editor configuration.

use serde::{Deserialize, Serialize};

use crate::constants;

/// Configuration UI et éditeur.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct UiConfig {
    /// Commande éditeur par défaut (ex. 'nvim', 'vim'). Utilisé dans tmux split.
    pub editor: String,
    /// Largeur du pane tmux éditeur (caractères). Défaut : 50.
    pub tmux_pane_width: u8,
}

impl Default for UiConfig {
    fn default() -> Self {
        UiConfig {
            editor: constants::TMUX_EDITOR.to_string(),
            tmux_pane_width: constants::TMUX_PANE_WIDTH_DEFAULT,
        }
    }
}
