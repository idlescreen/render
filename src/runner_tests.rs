use super::*;
use crate::job::StudioJob;
use idle_render::JobSpec;
use std::sync::Mutex;

// Tests inject `render` via the RENDER env var — process-global, so all
// env-mutating tests in this module share one lock (same race class as
// the daemon's TEST_ENV_LOCK).
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn job() -> StudioJob {
    StudioJob::new(
        "t".into(),
        JobSpec {
            effect: "beams".into(),
            plugin_path: None,
            seed: 1,
            fps: 30,
            duration: "1s".into(),
            output: PathBuf::from("/tmp/x.mkv"),
            width: 64,
            height: 64,
            cols: None,
            rows: None,
            dry_run: true,
            raw: false,
            segment: None,
            audio: None,
            resume: false,
            crf: 35,
            preset: None,
            encoder: None,
            prefer_hw: true,
            gpu_upscale: true,
            format: None,
            container: None,
            baseline_dir: None,
            snapshot_last_only: false,
            update_baselines: false,
            cpu_raster: false,
        },
    )
}

/// Temp executable that ignores `--job-file` and runs `body`.
fn fake_render(body: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "idle-studio-fake-render-{}-{}",
        std::process::id(),
        body.len()
    ));
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn run_job_success_and_failure() {
    let _g = ENV_LOCK.lock().unwrap();
    std::env::set_var("RENDER", "/bin/true");
    assert!(run_job(&job()).is_ok());
    std::env::set_var("RENDER", "/bin/false");
    assert!(run_job(&job()).is_err());
    std::env::remove_var("RENDER");
}

#[test]
fn unexecutable_render_errors_closed() {
    let _g = ENV_LOCK.lock().unwrap();
    let p = std::env::temp_dir().join(format!("idle-studio-noexec-{}", std::process::id()));
    std::fs::write(&p, "not a binary").unwrap(); // 0644 — spawn fails
    std::env::set_var("RENDER", &p);
    assert!(RunningJob::spawn(&job()).is_err());
    std::env::remove_var("RENDER");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn kill_terminates_running_child() {
    let _g = ENV_LOCK.lock().unwrap();
    let fake = fake_render("exec sleep 30");
    std::env::set_var("RENDER", &fake);
    let mut r = RunningJob::spawn(&job()).unwrap();
    assert!(r.poll().is_none(), "sleep 30 should still be running");
    r.kill();
    assert!(matches!(r.poll(), Some(Err(_))));
    std::env::remove_var("RENDER");
    let _ = std::fs::remove_file(&fake);
}

#[test]
fn reader_caps_output_at_tail() {
    let big = vec![b'x'; OUTPUT_TAIL_CAP * 3];
    let (_h, rx) = spawn_reader(std::io::Cursor::new(big));
    let s = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(s.len(), OUTPUT_TAIL_CAP);
}

#[test]
fn flood_of_output_does_not_deadlock() {
    // A child writing far more than the pipe buffer must still exit —
    // the reader threads drain concurrently.
    let _g = ENV_LOCK.lock().unwrap();
    let fake = fake_render("yes | head -c 300000; exit 0");
    std::env::set_var("RENDER", &fake);
    let out = run_job(&job()).unwrap();
    assert_eq!(out.len(), OUTPUT_TAIL_CAP);
    std::env::remove_var("RENDER");
    let _ = std::fs::remove_file(&fake);
}
