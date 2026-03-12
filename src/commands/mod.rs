//! Command handlers — grouped by domain.

mod data;
mod info;
mod progress_cmds;
mod run;
mod watch;

pub use data::{cmd_config, cmd_export, cmd_import, cmd_new, cmd_reset};
pub use info::{cmd_annales, cmd_hint, cmd_list, cmd_search, cmd_solution};
pub use progress_cmds::{cmd_progress, cmd_stats};
pub use run::{cmd_review, cmd_run};
pub use watch::cmd_watch;
