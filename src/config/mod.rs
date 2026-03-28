//! User configuration loaded from `~/.clings/clings.toml`.
//!
//! At startup, `init()` is called once. All code reads config via `get()`.
//! Missing fields fall back to the compile-time constants in `constants.rs`.

mod loader;
mod srs;
mod sync;
mod tests;
mod tmux;
mod ui;

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

pub use loader::{load, set_value};
pub use srs::SrsConfig;
pub use sync::SyncConfig;
pub use tmux::TmuxConfig;
pub use ui::UiConfig;

static CONFIG: OnceLock<ClingConfig> = OnceLock::new();

/// Configuration utilisateur top-level chargée depuis ~/.clings/clings.toml.
/// Les sections manquantes utilisent les défauts de constants.rs.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ClingConfig {
    /// Configuration SRS : decay, intervals, multiplier.
    pub srs: SrsConfig,
    /// Configuration UI : éditeur par défaut, largeur pane tmux.
    pub ui: UiConfig,
    /// Configuration tmux : auto-open editor pane dans tmux.
    pub tmux: TmuxConfig,
    /// Configuration sync Git : remote URL, branch, enabled flag.
    pub sync: SyncConfig,
    /// Chemin vers le repo libsys pour exports (défaut : $HOME/Developer/TOOLS/libsys).
    #[serde(default)]
    pub libsys_path: Option<std::path::PathBuf>,
}

/// Initialise la configuration globale. Doit être appelée une fois au démarrage.
/// Double-init est un no-op (OnceLock garantit une seule initialisation).
pub fn init(cfg: ClingConfig) {
    CONFIG.set(cfg).ok();
}

/// Accède à la configuration globale. Retourne le défaut si `init()` n'a pas été appelée.
pub fn get() -> &'static ClingConfig {
    CONFIG.get_or_init(ClingConfig::default)
}
