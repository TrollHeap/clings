use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

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
    let _stdin_thread = std::thread::spawn(move || {
        use std::io::Read;
        let stdin = std::io::stdin();
        let mut buf = [0u8; 1];
        loop {
            if stdin.lock().read_exact(&mut buf).is_err() {
                break;
            }
            if key_tx.send(buf[0]).is_err() {
                break;
            }
        }
    });

    let mut last_event = Instant::now();
    let debounce = Duration::from_millis(200);

    loop {
        // Check for file changes
        match rx.recv_timeout(Duration::from_millis(50)) {
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
                eprintln!("Watch error: {e}");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(KfError::Config("File watcher disconnected".to_string()));
            }
        }

        // Check for keyboard input
        if let Ok(key) = key_rx.try_recv() {
            if let Some(action) = on_key(key) {
                return Ok(action);
            }
        }
    }
}
