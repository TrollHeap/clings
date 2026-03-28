//! SRS (Spaced Repetition System) configuration.

use serde::{Deserialize, Serialize};

use crate::constants;

/// Configuration SRS (Spaced Repetition System).
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct SrsConfig {
    /// Jours d'inactivité avant que le score décroisse de 0.5.
    pub decay_days: i64,
    /// Intervalle de révision minimal après succès (jours).
    pub base_interval_days: i64,
    /// Intervalle de révision maximal après succès répétés (jours).
    pub max_interval_days: i64,
    /// Multiplicateur d'intervalle appliqué après succès (ex. 2.5x).
    pub interval_multiplier: f64,
}

impl Default for SrsConfig {
    fn default() -> Self {
        SrsConfig {
            decay_days: constants::MASTERY_DECAY_DAYS,
            base_interval_days: constants::SRS_BASE_INTERVAL_DAYS,
            max_interval_days: constants::SRS_MAX_INTERVAL_DAYS,
            interval_multiplier: constants::SRS_INTERVAL_MULTIPLIER,
        }
    }
}
