//! Canal d'événements : keyboard thread + notify file watcher → Msg channel

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::tui::app::Msg;

/// Lance un thread keyboard + file watcher, retourne le Receiver<Msg>.
///
/// Le keyboard thread envoie Msg::Key sur chaque touche pressée et Msg::Tick sinon.
/// Le file watcher envoie Msg::FileChanged(path) sur chaque modification.
pub fn spawn_event_reader(watch_path: PathBuf) -> mpsc::Receiver<Msg> {
    let (tx, rx) = mpsc::channel::<Msg>();

    // ── Thread keyboard ────────────────────────────────────────────────
    let tx_key = tx.clone();
    std::thread::spawn(move || {
        let tick = Duration::from_millis(100);
        loop {
            // event::poll() can fail if terminal is invalid (e.g., pipe redirected).
            // In this background thread context, we cannot propagate the error —
            // fallback to false (ignore this tick) to avoid panicking the app.
            if event::poll(tick).unwrap_or_else(|e| {
                eprintln!("[clings/events] erreur poll clavier: {e}");
                false
            }) {
                if let Ok(Event::Key(k)) = event::read() {
                    if k.kind == KeyEventKind::Press && tx_key.send(Msg::Key(k)).is_err() {
                        break;
                    }
                }
            } else if tx_key.send(Msg::Tick).is_err() {
                break;
            }
        }
    });

    // ── Thread file watcher ────────────────────────────────────────────
    let tx_file = tx;
    std::thread::spawn(move || {
        let (watcher_tx, watcher_rx) = mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(watcher_tx, notify::Config::default()) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[clings] watcher indisponible: {e}");
                return;
            }
        };
        if let Err(e) = watcher.watch(&watch_path, RecursiveMode::NonRecursive) {
            eprintln!("[clings] impossible de surveiller {:?}: {e}", watch_path);
            return;
        }
        let mut last = std::time::Instant::now();
        let debounce = Duration::from_millis(200);
        loop {
            match watcher_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Ok(ev)) => match ev.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        if last.elapsed() >= debounce {
                            last = std::time::Instant::now();
                            if tx_file.send(Msg::FileChanged).is_err() {
                                break;
                            }
                        }
                    }
                    _ => {}
                },
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                _ => {}
            }
        }
    });

    rx
}
