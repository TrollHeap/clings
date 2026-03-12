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
            if event::poll(tick).unwrap_or(false) {
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
            Err(_) => return,
        };
        if watcher
            .watch(&watch_path, RecursiveMode::NonRecursive)
            .is_err()
        {
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
                            if tx_file.send(Msg::FileChanged(watch_path.clone())).is_err() {
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
