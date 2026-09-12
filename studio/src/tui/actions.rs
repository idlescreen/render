use crate::job::StudioJob;
use crate::queue::{JobQueue, JobStatus};
use crate::runner::RunningJob;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub fn selected_pending_or_any(queue: &JobQueue, selected: usize) -> Option<usize> {
    if queue.entries.is_empty() {
        return None;
    }
    if selected < queue.entries.len() {
        return Some(selected);
    }
    Some(0)
}

/// Events a render worker reports back to the TUI loop. Keyed by job id —
/// indices go stale if the operator edits the queue mid-run.
pub enum RunEvent {
    Started(String),
    Finished(String, Result<String, String>),
    AllDone,
}

/// One worker thread draining a list of jobs sequentially. `cancel` makes the
/// TUI's `x` key kill the in-flight render child instead of blocking the UI.
pub struct RunWorker {
    pub rx: Receiver<RunEvent>,
    pub cancel: Arc<AtomicBool>,
}

pub fn spawn_runner(jobs: Vec<StudioJob>) -> RunWorker {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    thread::spawn(move || {
        'jobs: for job in jobs {
            let id = job.id.clone();
            let _ = tx.send(RunEvent::Started(id.clone()));
            let mut running = match RunningJob::spawn(&job) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(RunEvent::Finished(id, Err(e.to_string())));
                    continue;
                }
            };
            loop {
                if flag.load(Ordering::Relaxed) {
                    running.kill();
                    let _ = tx.send(RunEvent::Finished(id, Err("cancelled".into())));
                    break 'jobs;
                }
                match running.poll() {
                    Some(res) => {
                        let _ = tx.send(RunEvent::Finished(id, res.map_err(|e| e.to_string())));
                        break;
                    }
                    None => thread::sleep(Duration::from_millis(100)),
                }
            }
        }
        let _ = tx.send(RunEvent::AllDone);
    });
    RunWorker { rx, cancel }
}

/// Apply one worker event to the queue (status/message by job id) and save.
/// Returns a status-line message for Finished/AllDone, None for Started.
pub fn apply_run_event(queue: &mut JobQueue, path: &Path, ev: RunEvent) -> Option<String> {
    match ev {
        RunEvent::Started(id) => {
            if let Some(e) = queue.entries.iter_mut().find(|e| e.job.id == id) {
                e.status = JobStatus::Running;
                let _ = queue.save(path);
            }
            None
        }
        RunEvent::Finished(id, res) => {
            let Some(e) = queue.entries.iter_mut().find(|e| e.job.id == id) else {
                return Some(format!("{id} finished after being deleted"));
            };
            let msg = match &res {
                Ok(_) => {
                    e.status = JobStatus::Done;
                    format!("done {id}")
                }
                Err(m) => {
                    e.status = JobStatus::Failed;
                    format!("failed {id}: {}", m.chars().take(80).collect::<String>())
                }
            };
            e.message = res.unwrap_or_else(|e| e);
            let _ = queue.save(path);
            Some(msg)
        }
        RunEvent::AllDone => Some("run finished".into()),
    }
}
