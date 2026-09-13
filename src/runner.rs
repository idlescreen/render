//! Drive the **render** capability (same repo) as a subprocess.

use crate::error::StudioError;
use crate::job::StudioJob;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Keep the last N bytes of child output — enough for diagnostics without
/// letting a verbose render grow memory unboundedly.
const OUTPUT_TAIL_CAP: usize = 64 * 1024;

fn resolve_render_bin() -> Result<PathBuf, StudioError> {
    for key in ["RENDER", "IDLESCREEN_RENDER", "IDLE_RENDER"] {
        if let Ok(p) = std::env::var(key) {
            let pb = PathBuf::from(p);
            if pb.is_file() {
                return Ok(pb);
            }
        }
    }
    for name in ["render", "idle-render"] {
        if let Ok(out) = Command::new("which").arg(name).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Ok(PathBuf::from(s));
                }
            }
        }
    }
    // Workspace builds: render binary next to studio target; standalone-repo
    // builds: ../render is a sibling checkout (see .gitignore).
    for path in [
        "target/release/render",
        "target/debug/render",
        "../target/release/render",
        "../target/debug/render",
        "../render/target/release/render",
        "../render/target/debug/render",
        "render/target/release/render",
        "render/target/debug/render",
        "target/release/idle-render",
        "target/debug/idle-render",
    ] {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Ok(p);
        }
    }
    Err(StudioError::RenderMissing)
}

/// A render subprocess with drained pipes and a pollable exit.
///
/// Pipes are drained on reader threads (a child that fills its pipe buffer
/// would deadlock a naive wait), output is tail-capped, and `kill()` gives
/// the TUI a real cancel path.
pub struct RunningJob {
    child: Child,
    job_path: PathBuf,
    out_rx: Receiver<String>,
    err_rx: Receiver<String>,
}

fn spawn_reader<R: Read + Send + 'static>(pipe: R) -> (JoinHandle<()>, Receiver<String>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut pipe = pipe;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    if buf.len() > OUTPUT_TAIL_CAP {
                        let keep = buf.split_off(buf.len() - OUTPUT_TAIL_CAP);
                        buf = keep;
                    }
                }
            }
        }
        let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
    });
    (handle, rx)
}

impl RunningJob {
    pub fn spawn(job: &StudioJob) -> Result<Self, StudioError> {
        let bin = resolve_render_bin()?;
        let job_path = job.write_job_file().map_err(StudioError::Render)?;
        let mut child = match Command::new(&bin)
            .arg("--job-file")
            .arg(&job_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                let _ = std::fs::remove_file(&job_path);
                return Err(StudioError::Render(format!("spawn {}: {e}", bin.display())));
            }
        };
        let (_oh, out_rx) = spawn_reader(child.stdout.take().expect("piped stdout"));
        let (_eh, err_rx) = spawn_reader(child.stderr.take().expect("piped stderr"));
        Ok(Self {
            child,
            job_path,
            out_rx,
            err_rx,
        })
    }

    /// `Some(result)` once the child has exited; `None` while still running.
    /// Reader threads close their channels at pipe EOF, so the full output is
    /// available as soon as exit is observed.
    pub fn poll(&mut self) -> Option<Result<String, StudioError>> {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                let _ = std::fs::remove_file(&self.job_path);
                // Readers send once at pipe EOF, which lands just after child
                // exit — bounded wait so we don't race them and drop output.
                let collect = |rx: &Receiver<String>| {
                    rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default()
                };
                let stdout = collect(&self.out_rx);
                let stderr = collect(&self.err_rx);
                let msg = format!("{stdout}{stderr}");
                if status.success() {
                    Some(Ok(msg))
                } else {
                    Some(Err(StudioError::Render(msg)))
                }
            }
            Ok(None) => None,
            Err(e) => {
                let _ = std::fs::remove_file(&self.job_path);
                Some(Err(StudioError::Render(format!("wait: {e}"))))
            }
        }
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.job_path);
    }
}

impl Drop for RunningJob {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Run one job via `render --job-file <spec.json>` (blocking; for the CLI).
pub fn run_job(job: &StudioJob) -> Result<String, StudioError> {
    let mut running = RunningJob::spawn(job)?;
    loop {
        if let Some(res) = running.poll() {
            return res;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
