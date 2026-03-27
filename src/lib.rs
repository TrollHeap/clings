//! clings library — re-exports for integration tests.
#![allow(
    unused_imports,
    dead_code,
    clippy::new_without_default,
    clippy::items_after_test_module
)]

mod authoring;
mod chapters;
mod commands;
pub mod config;
pub mod constants;
mod error;
mod exam;
pub mod exercises;
pub mod mastery;
pub mod models;
mod piscine;
mod progress;
mod reporting;
pub mod runner;
mod search;
pub mod sync;
mod tmux;
pub mod tui;
mod watcher;
