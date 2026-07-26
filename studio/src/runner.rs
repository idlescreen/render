//! Drive the **render** capability (same repo) as a subprocess.

use crate::error::StudioError;
use crate::job::StudioJob;
use std::path::PathBuf;
use std::process::Command;

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
    // Workspace builds: render binary next to studio target
    for path in [
        "target/release/render",
        "target/debug/render",
        "../target/release/render",
        "../target/debug/render",
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

/// Run one job via `render --job-file <spec.json>`.
pub fn run_job(job: &StudioJob) -> Result<String, StudioError> {
    let bin = resolve_render_bin()?;
    let job_path = job
        .write_job_file()
        .map_err(StudioError::Render)?;
    let output = Command::new(&bin)
        .arg("--job-file")
        .arg(&job_path)
        .output()
        .map_err(|e| StudioError::Render(format!("spawn {}: {e}", bin.display())))?;
    let _ = std::fs::remove_file(&job_path);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let msg = format!("{stdout}{stderr}");
    if !output.status.success() {
        return Err(StudioError::Render(msg));
    }
    Ok(msg)
}
