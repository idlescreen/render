//! IdleScreen Studio — UI that drives the **render** capability (same repo).
//!
//! Export work is done by `render` via `JobSpec` / `--job-file`. Studio only queues and launches.

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
