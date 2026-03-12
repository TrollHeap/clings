//! File watcher and keyboard input handler for watch mode.
//!
//! Uses the `notify` crate with 200ms debounce for file change detection.
//! Keyboard input is handled via crossterm `event::poll`. Returns `WatchAction` variants.

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyEvent};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::constants::{DEBOUNCE_INTERVAL_MS, KEY_CHECK_TIMEOUT_MS};
use crate::error::{KfError, Result};

/// Action returned by the watch callback or keyboard input.
pub enum WatchAction {
    /// Exercise solved — advance to next
    Advance,
    /// Continue watching for changes
    Continue,
    /// User wants to skip
    #[allow(dead_code)]
    Skip,
    /// User wants to quit
    Quit,
    /// Navigate to next exercise (j)
    #[allow(dead_code)]
    Next,
    /// Navigate to previous exercise (k)
    #[allow(dead_code)]
    Prev,
}

/// Watch a file for modifications while also listening for keyboard commands.
/// The `on_change` callback is called on each file save.
/// The `on_key` callback is called when a key is pressed.
pub fn watch_file_interactive<F, K>(
    path: &Path,
    mut on_change: F,
    mut on_key: K,
) -> Result<WatchAction>
where
    F: FnMut() -> WatchAction,
    K: FnMut(KeyEvent) -> Option<WatchAction>,
{
    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;

    watcher.watch(path, RecursiveMode::NonRecursive)?;

    let mut last_event = Instant::now();
    let debounce = Duration::from_millis(DEBOUNCE_INTERVAL_MS);
    let key_timeout = Duration::from_millis(KEY_CHECK_TIMEOUT_MS);

    loop {
        // Check for file changes (non-blocking)
        match rx.try_recv() {
            Ok(Ok(event)) => match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    if last_event.elapsed() >= debounce {
                        last_event = Instant::now();
                        match on_change() {
                            WatchAction::Continue => {}
                            action => return Ok(action),
                        }
                    }
                }
                _ => {}
            },
            Ok(Err(e)) => {
                eprintln!("Erreur de surveillance : {e}");
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err(KfError::Config("File watcher disconnected".to_string()));
            }
        }

        // Poll for keyboard events with timeout
        if crossterm::event::poll(key_timeout)? {
            if let Event::Key(key_event) = crossterm::event::read()? {
                if let Some(action) = on_key(key_event) {
                    return Ok(action);
                }
            }
        }
    }
}
