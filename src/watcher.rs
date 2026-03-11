use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    Skip,
    /// User wants to quit
    Quit,
    /// Navigate to next exercise (j)
    Next,
    /// Navigate to previous exercise (k)
    Prev,
}

/// Watch a file for modifications while also listening for keyboard commands.
/// The `on_change` callback is called on each file save.
/// The `on_key` callback is called when a key is pressed (non-blocking).
pub fn watch_file_interactive<F, K>(
    path: &Path,
    mut on_change: F,
    mut on_key: K,
) -> Result<WatchAction>
where
    F: FnMut() -> WatchAction,
    K: FnMut(u8) -> Option<WatchAction>,
{
    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;

    watcher.watch(path, RecursiveMode::NonRecursive)?;

    // Set up non-blocking stdin
    let (key_tx, key_rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let stdin_thread = std::thread::spawn(move || {
        use std::io::Read;
        let stdin = std::io::stdin();
        let mut buf = [0u8; 1];
        loop {
            if stop_clone.load(Ordering::Relaxed) {
                break;
            }
            if stdin.lock().read_exact(&mut buf).is_err() {
                break;
            }
            if key_tx.send(buf[0]).is_err() {
                break;
            }
        }
    });

    let mut last_event = Instant::now();
    let debounce = Duration::from_millis(DEBOUNCE_INTERVAL_MS);

    let result = loop {
        // Check for file changes
        match rx.recv_timeout(Duration::from_millis(KEY_CHECK_TIMEOUT_MS)) {
            Ok(Ok(event)) => match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    if last_event.elapsed() >= debounce {
                        last_event = Instant::now();
                        match on_change() {
                            WatchAction::Continue => {}
                            action => break Ok(action),
                        }
                    }
                }
                _ => {}
            },
            Ok(Err(e)) => {
                eprintln!("Watch error: {e}");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err(KfError::Config("File watcher disconnected".to_string()));
            }
        }

        // Check for keyboard input
        if let Ok(key) = key_rx.try_recv() {
            if let Some(action) = on_key(key) {
                break Ok(action);
            }
        }
    };

    stop.store(true, Ordering::Relaxed);
    // The stdin thread is blocked on read_exact; detach it instead of joining.
    // It will exit on the next keypress (stop == true) or when key_rx is dropped
    // (causing key_tx.send to fail).
    drop(stdin_thread);

    result
}
