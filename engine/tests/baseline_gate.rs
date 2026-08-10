//! Integration tests for the F7 follow-up: baseline-overwrite env gate.
//!
//! Two assertions (per AXIOMS §1.6 — pass AND fail proof):
//!
//! 1. `env_var_authorizes_overwrite` — with `RENDER_FORCE_UPDATE_BASELINES=1`
//!    set in the environment, `job.update_baselines = true` rewrites the
//!    baseline file and the snapshot outcome is `Updated` (the happy path).
//!
//! 2. `env_var_missing_refuses_overwrite` — without the env var, the same
//!    job returns `SnapshotOutcome::Refused` with a reason that names the
//!    required variable. This is the fail-closed property: a typo'd CLI flag
//!    in CI must not silently rewrite checked-in baselines.

use idle_render::models::{Container, OutputFormat, RenderJob};
use idle_render::pipeline::run_pipeline;
use idle_render::pipeline_snapshot::SnapshotOutcome;
use idle_render::EncodeBackend;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::tempdir;

fn plugin_path(name: &str) -> PathBuf {
    let repo_root = std::env::var("RENDER_REPO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            p.parent().unwrap().to_path_buf()
        });
    let candidate = repo_root
        .join(format!("../idle-saver-{name}"))
        .join("target")
        .join("release")
        .join(format!("libscreensaver_{name}.so"));
    std::fs::canonicalize(&candidate).unwrap_or(candidate)
}

fn build_raw_job(baseline_dir: &std::path::Path, out: &std::path::Path, seed: u64) -> RenderJob {
    let plugin = plugin_path("beams");
    RenderJob {
        effect: "beams".into(),
        plugin_path: Some(plugin),
        seed,
        fps: 30,
        duration: Duration::from_secs(1),
        width: 16,
        height: 16,
        output: out.to_path_buf(),
        cols: None,
        rows: None,
        dry_run: false,
        segment: None,
        audio: None,
        resume: false,
        crf: 35,
        preset: None,
        encoder: None,
        prefer_hw: false,
        gpu_upscale: false,
        format: OutputFormat::Raw,
        container: Container::Mkv,
        baseline_dir: Some(baseline_dir.to_path_buf()),
        snapshot_last_only: true,
        update_baselines: true,
        cpu_raster: true,
    }
}

#[test]
fn env_var_authorizes_overwrite() {
    std::env::set_var("IDLE_ALLOW_UNSIGNED_PLUGINS", "1");
    std::env::set_var("RENDER_FORCE_UPDATE_BASELINES", "1");
    let tmp = tempdir().expect("tmp");
    let baseline_dir = tmp.path().join("baselines");
    std::fs::create_dir_all(&baseline_dir).expect("baselines");
    let work = tmp.path().join("work");
    std::fs::create_dir_all(&work).expect("work");
    let out = work.join("out.bgra");

    let so = plugin_path("beams");
    if !so.is_file() {
        eprintln!("skipping env_var_authorizes_overwrite: plugin not built at {}", so.display());
        return;
    }

    let job = build_raw_job(&baseline_dir, &out, 0xAA00_BB00);
    let result = run_pipeline(&job, EncodeBackend::RawDump).expect("pipeline ok");
    std::env::remove_var("RENDER_FORCE_UPDATE_BASELINES");
    match result.snapshot {
        SnapshotOutcome::Updated { path } => {
            assert!(path.is_file(), "baseline must exist after authorized overwrite");
        }
        other => panic!("expected Updated, got {other:?}"),
    }
}

#[test]
fn env_var_missing_refuses_overwrite() {
    std::env::set_var("IDLE_ALLOW_UNSIGNED_PLUGINS", "1");
    std::env::remove_var("RENDER_FORCE_UPDATE_BASELINES");
    let tmp = tempdir().expect("tmp");
    let baseline_dir = tmp.path().join("baselines");
    std::fs::create_dir_all(&baseline_dir).expect("baselines");
    let work = tmp.path().join("work");
    std::fs::create_dir_all(&work).expect("work");
    let out = work.join("out.bgra");

    let so = plugin_path("beams");
    if !so.is_file() {
        eprintln!("skipping env_var_missing_refuses_overwrite: plugin not built at {}", so.display());
        return;
    }

    let job = build_raw_job(&baseline_dir, &out, 0xBB00_CC00);
    let result = run_pipeline(&job, EncodeBackend::RawDump).expect("pipeline ok");
    match result.snapshot {
        SnapshotOutcome::Refused { reason, path } => {
            assert!(
                reason.contains("RENDER_FORCE_UPDATE_BASELINES"),
                "refusal reason must name the env var, got: {reason}"
            );
            assert!(
                !path.exists(),
                "baseline must NOT be written when gate refuses; found {}",
                path.display()
            );
        }
        other => panic!("expected Refused (fail-closed), got {other:?}"),
    }
}
