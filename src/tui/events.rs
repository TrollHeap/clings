//! Canal d'événements crossterm + notify → Msg
//!
//! Phase 1: Stub minimal, sera complété en Phase 2.

/// Placeholder pour le canal d'événements.
/// Phase 2 branchera le notify watcher ici.
#[allow(dead_code)]
pub struct EventChannel;

#[allow(dead_code)]
impl EventChannel {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EventChannel {
    fn default() -> Self {
        Self::new()
    }
}
