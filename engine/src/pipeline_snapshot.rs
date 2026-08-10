//! Snapshot flow for `--snapshot-last-only`.
//!
//! When `job.snapshot_last_only` + `job.baseline_dir` are both set, the
//! pipeline compares the just-written output against
//! `<baseline_dir>/<scenario>.last.<ext>` (or overwrites it when
//! `job.update_baselines` is set). Returns [`SnapshotOutcome`] so the caller
//! can print a summary line and exit non-zero on mismatch.
//!
//! ## F7 follow-up: baseline-overwrite env gate
//!
//! `--update-baselines` is a fail-open CLI flag: a typo in CI would silently
//! rewrite the checked-in baseline PNGs that the snapshot regression tests
//! byte-compare against. To close that hole, the overwrite path in
//! [`evaluate`] requires `RENDER_FORCE_UPDATE_BASELINES=1` to be set in the
//! process environment. With the env var unset, the flag is inert and the
//! snapshot returns [`SnapshotOutcome::Refused`] (fail-closed). This matches
//! the existing `IDLE_ALLOW_UNSIGNED_PLUGINS` opt-in convention.

use crate::error::RenderError;
use crate::models::RenderJob;
use crate::snapshot;
use std::path::PathBuf;

/// Name of the env var that authorizes baseline overwrites.
const FORCE_UPDATE_ENV: &str = "RENDER_FORCE_UPDATE_BASELINES";

/// Return true iff the operator explicitly authorized baseline overwrites
/// via `RENDER_FORCE_UPDATE_BASELINES=1`. The CLI flag is inert otherwise.
fn force_update_baselines_authorized() -> bool {
    std::env::var(FORCE_UPDATE_ENV).ok().as_deref() == Some("1")
}

/// Outcome of a snapshot compare (or update) pass.
#[derive(Debug, Clone)]
pub enum SnapshotOutcome {
    /// No baseline configured; nothing to do.
    Skipped,
    /// `--update-baselines` was set (and authorized); baseline was rewritten.
    Updated { path: PathBuf },
    /// Compare passed (raw or PNG-decoded to RGBA).
    Matched { path: PathBuf },
    /// Compare failed; carries the reason.
    Mismatched { path: PathBuf, reason: String },
    /// Baseline file missing; pipeline failed this snapshot.
    MissingBaseline { path: PathBuf },
    /// `--update-baselines` was set but the env gate refused it. Fail-closed.
    Refused { path: PathBuf, reason: String },
}

/// Build the scenario baseline file path: `<dir>/<scenario>.last.<ext>`.
pub fn baseline_path_for(job: &RenderJob) -> Option<PathBuf> {
    let dir = job.baseline_dir.as_ref()?;
    let scenario = scenario_name(job);
    let ext = match job.format {
        crate::models::OutputFormat::Png => "png",
        crate::models::OutputFormat::Raw => "bgra",
        crate::models::OutputFormat::Mp4 => match job.container {
            crate::models::Container::Mp4 => "mp4",
            crate::models::Container::Mkv => "mkv",
        },
    };
    Some(dir.join(format!("{scenario}.last.{ext}")))
}

/// Derive a stable scenario name from job fields (effect + seed + duration).
fn scenario_name(job: &RenderJob) -> String {
    let mut s = job.effect.clone();
    s.push_str(&format!("_seed{:016x}", job.seed));
    let secs = job.duration.as_secs_f64();
    if secs.fract() == 0.0 {
        s.push_str(&format!("_d{}s", secs as u64));
    } else {
        s.push_str(&format!("_d{:.2}s", secs));
    }
    s
}

/// Pick the lexicographically largest `frame*.png` in `dir` (PngSequence
/// uses zero-padded frame numbers, so lex order = chronological order).
fn last_png_in_dir(dir: &std::path::Path) -> Result<std::path::PathBuf, RenderError> {
    let mut best: Option<std::path::PathBuf> = None;
    for entry in std::fs::read_dir(dir).map_err(|source| RenderError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let p = match entry {
            Ok(e) => e.path(),
            Err(_) => continue,
        };
        if p.extension().and_then(|s| s.to_str()) == Some("png") {
            let take = match &best {
                None => true,
                Some(cur) => p.file_name() > cur.file_name(),
            };
            if take { best = Some(p); }
        }
    }
    best.ok_or_else(|| RenderError::Job("snapshot: no PNG files in output dir".into()))
}

/// Run compare or update against the baseline file. Returns the outcome;
/// does NOT mutate any error state. Mismatches are returned, not raised.
pub fn evaluate(job: &RenderJob, current: &std::path::Path) -> SnapshotOutcome {
    let Some(baseline) = baseline_path_for(job) else {
        return SnapshotOutcome::Skipped;
    };
    if job.update_baselines {
        // F7 gate: a typo'd `--update-baselines` in CI would silently rewrite
        // the checked-in baseline PNGs. Require explicit opt-in via env var.
        if !force_update_baselines_authorized() {
            return SnapshotOutcome::Refused {
                path: baseline,
                reason: format!(
                    "--update-baselines refused: set {FORCE_UPDATE_ENV}=1 to authorize baseline overwrite"
                ),
            };
        }
        let bytes = match snapshot::read_baseline(current) {
            Ok(b) => b,
            Err(e) => return SnapshotOutcome::Mismatched { path: baseline, reason: format!("read current: {e}") },
        };
        if let Err(e) = snapshot::write_baseline(&baseline, &bytes) {
            return SnapshotOutcome::Mismatched { path: baseline, reason: format!("write baseline: {e}") };
        }
        return SnapshotOutcome::Updated { path: baseline };
    }
    match snapshot::compare(current, &baseline) {
        Ok(()) => SnapshotOutcome::Matched { path: baseline },
        Err(snapshot::SnapshotMismatch::MissingBaseline(_)) => SnapshotOutcome::MissingBaseline { path: baseline },
        Err(e) => SnapshotOutcome::Mismatched { path: baseline, reason: e.to_string() },
    }
}

/// Snapshot pass: when `--snapshot-last-only` + `--baseline-dir` are set,
/// compare the just-written output (or its last frame) against the baseline.
/// Otherwise returns [`SnapshotOutcome::Skipped`].
pub fn run_snapshot(job: &RenderJob) -> Result<SnapshotOutcome, RenderError> {
    if !job.snapshot_last_only || job.baseline_dir.is_none() {
        return Ok(SnapshotOutcome::Skipped);
    }
    // For PngSequence the output is a directory; the snapshot compares against
    // the largest frame file inside. For Raw/Mp4 the output is a file we tail.
    let current_path = match job.format {
        crate::models::OutputFormat::Png => last_png_in_dir(&job.output)?,
        _ => job.output.clone(),
    };
    Ok(evaluate(job, &current_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Container, OutputFormat};
    use std::path::Path;
    use std::time::Duration;

    fn job(format: OutputFormat, container: Container, seed: u64) -> RenderJob {
        RenderJob {
            effect: "beams".into(), plugin_path: None, seed,
            fps: 30, duration: Duration::from_secs(2),
            width: 64, height: 64, output: PathBuf::from("/tmp/unused.mkv"),
            cols: None, rows: None, dry_run: false, segment: None,
            audio: None, resume: false, crf: 35, preset: None,
            encoder: None, prefer_hw: true, gpu_upscale: true,
            format, container, baseline_dir: None,
            snapshot_last_only: true, update_baselines: false, cpu_raster: true,
        }
    }

    #[test]
    fn baseline_path_picks_ext_by_format() {
        let mut j = job(OutputFormat::Png, Container::Mkv, 0xDEAD_BEEF);
        j.baseline_dir = Some(PathBuf::from("/tmp/baselines"));
        let p = baseline_path_for(&j).expect("path");
        assert!(p.to_string_lossy().ends_with(".last.png"));
        assert!(p.to_string_lossy().contains("beams_seed"));

        let mut j2 = job(OutputFormat::Mp4, Container::Mp4, 0xC0FFEE00);
        j2.baseline_dir = Some(PathBuf::from("/tmp/baselines"));
        assert!(baseline_path_for(&j2).unwrap().to_string_lossy().ends_with(".last.mp4"));

        let mut j3 = job(OutputFormat::Raw, Container::Mkv, 0x12345678);
        j3.baseline_dir = Some(PathBuf::from("/tmp/baselines"));
        assert!(baseline_path_for(&j3).unwrap().to_string_lossy().ends_with(".last.bgra"));
    }

    #[test]
    fn evaluate_skipped_when_no_baseline_dir() {
        let j = job(OutputFormat::Png, Container::Mkv, 1);
        assert!(matches!(evaluate(&j, Path::new("/tmp/x.png")), SnapshotOutcome::Skipped));
    }

    #[test]
    fn update_baselines_refused_without_env() {
        // F7: --update-baselines alone must not silently overwrite.
        let mut j = job(OutputFormat::Raw, Container::Mkv, 0xFEED_FACE);
        j.baseline_dir = Some(PathBuf::from("/tmp/baselines"));
        j.update_baselines = true;
        // Make sure the gate is closed for this test regardless of caller env.
        std::env::remove_var("RENDER_FORCE_UPDATE_BASELINES");
        match evaluate(&j, Path::new("/tmp/current.bgra")) {
            SnapshotOutcome::Refused { reason, .. } => {
                assert!(
                    reason.contains("RENDER_FORCE_UPDATE_BASELINES"),
                    "refusal reason must name the env var, got: {reason}"
                );
            }
            other => panic!("expected Refused, got {other:?}"),
        }
    }

    #[test]
    fn update_baselines_allowed_with_env_var() {
        // F7 pass-case: with RENDER_FORCE_UPDATE_BASELINES=1, the overwrite
        // path executes. We write a small current file into a tmpdir so the
        // overwrite path can complete without IO failure, and assert that
        // the outcome is `Updated` (not `Refused`).
        let tmp = tempfile::tempdir().expect("tmp");
        let current = tmp.path().join("current.bgra");
        std::fs::write(&current, [1u8, 2, 3, 4]).expect("write current");
        let mut j = job(OutputFormat::Raw, Container::Mkv, 0xC0DE_BEEF);
        j.baseline_dir = Some(tmp.path().join("baselines"));
        j.update_baselines = true;
        std::env::set_var("RENDER_FORCE_UPDATE_BASELINES", "1");
        let out = evaluate(&j, &current);
        std::env::remove_var("RENDER_FORCE_UPDATE_BASELINES");
        assert!(matches!(out, SnapshotOutcome::Updated { .. }), "expected Updated, got {out:?}");
    }
}
