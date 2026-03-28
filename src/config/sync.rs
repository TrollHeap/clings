//! Git-based progress synchronization configuration.

use serde::{Deserialize, Serialize};

use crate::constants;

/// Configuration synchronisation Git pour progrès cross-machines.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SyncConfig {
    /// Activer/désactiver la sync Git des snapshots de progression.
    pub enabled: bool,
    /// URL remote Git (ex. 'https://github.com/user/clings-progress.git').
    pub remote: String,
    /// Branche Git pour les snapshots. Défaut : 'main'.
    pub branch: String,
    /// Hostname machine pour les commits sync. Si vide, utilise hostname système.
    pub hostname: String,
}

impl Default for SyncConfig {
    fn default() -> Self {
        SyncConfig {
            enabled: false,
            remote: String::new(),
            branch: constants::SYNC_DEFAULT_BRANCH.to_string(),
            hostname: String::new(),
        }
    }
}
