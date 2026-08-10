//! Snapshot flow for `--snapshot-last-only`.
//!
//! When `job.snapshot_last_only` + `job.baseline_dir` are both set, the
//! pipeline encodes the *last* frame to a temp file then either compares
//! against `<baseline_dir>/<scenario>.last.<ext>` or overwrites that file
//! (when `job.update_baselines`). Returns the [`SnapshotOutcome`] so the
//! caller can print a summary line and exit non-zero on mismatch.

use crate::error::RenderError;
use crate::models::RenderJob;
use crate::png_writer::encode_bgra_frame_to_png;
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
    let mut s = String::from(job.effect.clone());
    s.push_str(&format!("_seed{:016x}", job.seed));
    let secs = job.duration.as_secs_f64();
    if secs.fract() == 0.0 {
        s.push_str(&format!("_d{}s", secs as u64));
    } else {
        s.push_str(&format!("_d{:.2}s", secs));
    }
    s
}

/// Encode a single BGRA frame to bytes for the chosen format, written to `out`.
pub fn write_last_frame(
    job: &RenderJob,
    width: u32,
    height: u32,
    bgra: &[u8],
    out: &std::path::Path,
) -> Result<(), RenderError> {
    match job.format {
        crate::models::OutputFormat::Png => {
            let bytes = encode_bgra_frame_to_png(width, height, bgra);
            snapshot::write_baseline(out, &bytes).map_err(|e| RenderError::Io {
                path: out.to_path_buf(),
                source: e,
            })
        }
        crate::models::OutputFormat::Raw => snapshot::write_baseline(out, bgra).map_err(|e| {
            RenderError::Io {
                path: out.to_path_buf(),
                source: e,
            }
        }),
        crate::models::OutputFormat::Mp4 => {
            // Mp4/H264 last-frame snapshot isn't supported in v1; callers should
            // not invoke this branch. Fall back to raw BGRA.
            snapshot::write_baseline(out, bgra).map_err(|e| RenderError::Io {
                path: out.to_path_buf(),
                source: e,
            })
        }
    }
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
                };
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
/// decode the final frame from the just-written output and compare/update the
/// baseline file. Otherwise returns [`SnapshotOutcome::Skipped`].
pub fn run_snapshot(job: &RenderJob) -> Result<SnapshotOutcome, RenderError> {
    if !job.snapshot_last_only || job.baseline_dir.is_none() {
        return Ok(SnapshotOutcome::Skipped);
    }
    let tmp = tempfile_path_for(job);
    let last_bgra = read_last_bgra_frame(&job.output, job.width, job.height)?;
    write_last_frame(job, job.width, job.height, &last_bgra, &tmp)?;
    let outcome = evaluate(job, &tmp);
    let _ = std::fs::remove_file(&tmp);
    Ok(outcome)
}

fn tempfile_path_for(job: &RenderJob) -> std::path::PathBuf {
    let ext = match job.format {
        crate::models::OutputFormat::Png => "png",
        crate::models::OutputFormat::Raw => "bgra",
        crate::models::OutputFormat::Mp4 => match job.container {
            crate::models::Container::Mp4 => "mp4",
            crate::models::Container::Mkv => "mkv",
        },
    };
    let mut p = std::env::temp_dir();
    p.push(format!(
        "render-snap-{}-{}.{}",
        std::process::id(),
        job.seed,
        ext
    ));
    p
}

/// Read the final BGRA frame from a raw `.bgra` file (multi-frame BGRA dump).
fn read_last_bgra_frame(
    path: &std::path::Path,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, RenderError> {
    let bytes = std::fs::read(path).map_err(|source| RenderError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let frame_bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| RenderError::Job("snapshot: dimension overflow".into()))?;
    if bytes.len() < frame_bytes {
        return Err(RenderError::Job("snapshot: file shorter than one frame".into()));
    }
    Ok(bytes[bytes.len() - frame_bytes..].to_vec())
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
}
