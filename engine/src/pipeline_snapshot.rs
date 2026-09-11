//! Snapshot flow for `--snapshot-last-only`.
//!
//! When `job.snapshot_last_only` + `job.baseline_dir` are both set, the
//! pipeline compares the just-written output against
//! `<baseline_dir>/<scenario>.last.<ext>` (or overwrites it when
//! `job.update_baselines` is set). Returns [`SnapshotOutcome`] so the caller
//! can print a summary line and exit non-zero on mismatch.

use crate::error::RenderError;
use crate::models::RenderJob;
use crate::snapshot;
use std::path::PathBuf;

/// Outcome of a snapshot compare (or update) pass.
#[derive(Debug, Clone)]
pub enum SnapshotOutcome {
    /// No baseline configured; nothing to do.
    Skipped,
    /// `--update-baselines` was set; baseline was rewritten.
    Updated { path: PathBuf },
    /// Compare passed (raw or PNG-decoded to RGBA).
    Matched { path: PathBuf },
    /// Compare failed; carries the reason.
    Mismatched { path: PathBuf, reason: String },
    /// Baseline file missing; pipeline failed this snapshot.
    MissingBaseline { path: PathBuf },
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
            if take {
                best = Some(p);
            }
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
        let bytes = match snapshot::read_baseline(current) {
            Ok(b) => b,
            Err(e) => {
                return SnapshotOutcome::Mismatched {
                    path: baseline,
                    reason: format!("read current: {e}"),
                }
            }
        };
        if let Err(e) = snapshot::write_baseline(&baseline, &bytes) {
            return SnapshotOutcome::Mismatched {
                path: baseline,
                reason: format!("write baseline: {e}"),
            };
        }
        return SnapshotOutcome::Updated { path: baseline };
    }
    match snapshot::compare(current, &baseline) {
        Ok(()) => SnapshotOutcome::Matched { path: baseline },
        Err(snapshot::SnapshotMismatch::MissingBaseline(_)) => {
            SnapshotOutcome::MissingBaseline { path: baseline }
        }
        Err(e) => SnapshotOutcome::Mismatched {
            path: baseline,
            reason: e.to_string(),
        },
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
            effect: "beams".into(),
            plugin_path: None,
            seed,
            fps: 30,
            duration: Duration::from_secs(2),
            width: 64,
            height: 64,
            output: PathBuf::from("/tmp/unused.mkv"),
            cols: None,
            rows: None,
            dry_run: false,
            segment: None,
            audio: None,
            resume: false,
            crf: 35,
            preset: None,
            encoder: None,
            prefer_hw: true,
            gpu_upscale: true,
            format,
            container,
            baseline_dir: None,
            snapshot_last_only: true,
            update_baselines: false,
            cpu_raster: true,
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
        assert!(baseline_path_for(&j2)
            .unwrap()
            .to_string_lossy()
            .ends_with(".last.mp4"));

        let mut j3 = job(OutputFormat::Raw, Container::Mkv, 0x12345678);
        j3.baseline_dir = Some(PathBuf::from("/tmp/baselines"));
        assert!(baseline_path_for(&j3)
            .unwrap()
            .to_string_lossy()
            .ends_with(".last.bgra"));
    }

    #[test]
    fn evaluate_skipped_when_no_baseline_dir() {
        let j = job(OutputFormat::Png, Container::Mkv, 1);
        assert!(matches!(
            evaluate(&j, Path::new("/tmp/x.png")),
            SnapshotOutcome::Skipped
        ));
    }
}
