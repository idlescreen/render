//! IdleScreen Studio — UI that drives the **render** capability.
//!
//! Export work is done by `render` via `JobSpec` / `--job-file`. Studio only queues and launches.
//! `render` resolves via the `./render` sibling symlink (see .gitignore) or PATH.

pub mod error;
pub mod job;
pub mod queue;
pub mod runner;
pub mod tui;

pub use error::StudioError;
pub use job::StudioJob;
pub use queue::{JobQueue, JobStatus};
pub use runner::run_job;

#[cfg(test)]
mod proptests;
