//! Ratatui TUI framework integration — multiple views for watch/piscine/exam modes.

/// Application state (TEA/Elm architecture) and message types.
pub mod app;
/// Shared rendering utilities (colors, layout, help overlays, status bar).
pub mod common;
/// Event loop handling (keyboard input, file watcher, tick timer).
pub mod events;
/// Annales exam session browser and selector.
pub mod ui_annales;
/// Exam session selection screen (NSY103/UTC502 simulator).
pub mod ui_exam_selector;
/// Main launcher screen — mode selector (watch/piscine/exam/review).
pub mod ui_launcher;
/// libsys portfolio overlay — view exported library functions.
pub mod ui_libsys;
/// Exercise list overlay ([l]) — chapter headers and completion markers.
pub mod ui_list;
/// Piscine/exam mode view — linear progression with timer.
pub mod ui_piscine;
/// Single exercise run view (non-watch, non-piscine execution).
pub mod ui_run;
/// Statistics dashboard — mastery by chapter, success rates, top subjects.
pub mod ui_stats;
/// Interactive memory visualizer ([v]) — step through stack/heap snapshots.
pub mod ui_visualizer;
/// Watch mode main view — exercise description, compiler output, sidebar.
pub mod ui_watch;
