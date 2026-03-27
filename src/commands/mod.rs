//! Command handlers — grouped by domain (data, info, progress, report, run, watch).

mod data;
mod info;
mod progress_cmds;
mod report;
mod run;
mod watch;

/// Data management commands: config, export, import, new exercise, reset, schema, sync.
pub use data::{
    cmd_config, cmd_export, cmd_import, cmd_new, cmd_reset, cmd_schema, cmd_sync_init,
    cmd_sync_now, cmd_sync_status,
};
/// Information commands: annales (exam mapping), hints, list exercises, search, solution.
pub use info::{cmd_annales, cmd_hint, cmd_list, cmd_search, cmd_solution};
/// Progress commands: show progress by subject, display statistics.
pub use progress_cmds::{cmd_progress, cmd_stats};
/// Reporting command: generate learning analytics by chapter.
pub use report::cmd_report;
/// Exercise execution commands: run single exercise, review mode.
pub use run::{cmd_review, cmd_run};
/// Watch mode command: file watcher + TUI interactive trainer.
pub use watch::cmd_watch;
