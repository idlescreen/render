use crate::queue::{JobQueue, JobStatus};
use crate::runner::run_job;
use std::path::Path;

pub fn selected_pending_or_any(queue: &JobQueue, selected: usize) -> Option<usize> {
    if queue.entries.is_empty() {
        return None;
    }
    if selected < queue.entries.len() {
        return Some(selected);
    }
    Some(0)
}

pub fn run_jobs(queue: &mut JobQueue, path: &Path, all: bool) -> String {
    let mut last = String::from("no pending");
    while let Some(idx) = queue.next_pending_index() {
        last = run_job_at(queue, path, idx);
        if !all {
            break;
        }
    }
    last
}

pub fn run_job_at(queue: &mut JobQueue, path: &Path, idx: usize) -> String {
    if idx >= queue.entries.len() {
        return "bad index".into();
    }
    queue.entries[idx].status = JobStatus::Running;
    let _ = queue.save(path);
    let job = queue.entries[idx].job.clone();
    match run_job(&job) {
        Ok(msg) => {
            queue.entries[idx].status = JobStatus::Done;
            queue.entries[idx].message = msg;
            let _ = queue.save(path);
            format!("done {}", job.id)
        }
        Err(e) => {
            queue.entries[idx].status = JobStatus::Failed;
            queue.entries[idx].message = e.to_string();
            let _ = queue.save(path);
            format!("failed {}: {e}", job.id)
        }
    }
}
