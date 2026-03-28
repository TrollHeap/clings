//! clings library — re-exports for integration tests and public API.
#![allow(
    unused_imports,
    dead_code,
    clippy::new_without_default,
    clippy::items_after_test_module
)]
// TODO: enable #![warn(missing_docs)] once documentation coverage is complete.
// Currently ~51 public items lack documentation (cf. quality scan 2026-03-27).

mod authoring;
mod chapters;
mod commands;
/// User configuration (srs, ui, tmux, sync).
pub mod config;
/// Global constants (timing, thresholds, UI dimensions, compiler settings).
pub mod constants;
mod error;
mod exam;
/// Exercise loading and parsing from TOML.
pub mod exercises;
/// libsys portfolio management (library exports by learner).
pub mod libsys;
/// SRS (Spaced Repetition System) algorithm and mastery scoring.
pub mod mastery;
/// Core data types (Exercise, Subject, ValidationMode, Difficulty, etc.).
pub mod models;
mod piscine;
mod progress;
mod reporting;
/// C code compilation and execution engine (runner.rs).
pub mod runner;
mod search;
/// Git-based progress synchronization across machines.
pub mod sync;
mod tmux;
/// Ratatui TUI framework integration (watch, piscine, exam, list views).
pub mod tui;
mod watcher;
