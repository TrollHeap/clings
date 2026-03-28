//! Command handlers — grouped by domain (data, info, progress, report, run, watch).
//! Re-exports are consumed by the binary (main.rs) but not referenced from the library target.
#![allow(unused_imports)]

mod data;
mod info;
mod persistence;
mod progress_cmds;
mod report;
mod run;
mod session;
mod sync;
mod watch;

/// Data management commands: config, new exercise, schema.
pub use data::{cmd_config, cmd_new, cmd_schema};
/// Information commands: annales (exam mapping), hints, list exercises, search, solution.
pub use info::{cmd_annales, cmd_hint, cmd_list, cmd_search, cmd_solution};
/// Persistence commands: export, import progress.
pub use persistence::{cmd_export, cmd_import};
/// Progress commands: show progress by subject, display statistics.
pub use progress_cmds::{cmd_progress, cmd_stats};
/// Reporting command: generate learning analytics by chapter.
pub use report::cmd_report;
/// Exercise execution commands: run single exercise, review mode.
pub use run::{cmd_review, cmd_run};
/// Session commands: reset progress.
pub use session::cmd_reset;
/// Sync commands: sync init, status, now.
pub use sync::{cmd_sync_init, cmd_sync_now, cmd_sync_status};
/// Watch mode command: file watcher + TUI interactive trainer.
pub use watch::cmd_watch;
