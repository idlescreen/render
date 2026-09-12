//! Segment planning and ffmpeg concat for long encodes.

use crate::error::RenderError;
use crate::models::RenderJob;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// One planned chunk of a segmented render.
#[derive(Debug, Clone)]
pub struct SegmentPlan {
    pub index: u32,
    pub duration: Duration,
    pub path: PathBuf,
}

/// Build segment paths next to the final output: `out.part000.mkv`, …
pub fn plan_segments(job: &RenderJob) -> Result<Vec<SegmentPlan>, RenderError> {
    let Some(seg) = job.segment else {
        return Ok(vec![SegmentPlan {
            index: 0,
            duration: job.duration,
            path: job.output.clone(),
        }]);
    };
    if seg.is_zero() || seg >= job.duration {
        return Ok(vec![SegmentPlan {
            index: 0,
            duration: job.duration,
            path: job.output.clone(),
        }]);
    }

    let stem = job
        .output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("out");
    let ext = job
        .output
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mkv");
    let parent = job.output.parent().unwrap_or_else(|| Path::new("."));

    let mut plans = Vec::new();
    let mut remaining = job.duration;
    let mut index = 0u32;
    while !remaining.is_zero() {
        let this = if remaining > seg { seg } else { remaining };
        let name = format!("{stem}.part{index:03}.{ext}");
        plans.push(SegmentPlan {
            index,
            duration: this,
            path: parent.join(name),
        });
        remaining = remaining.saturating_sub(this);
        index = index.saturating_add(1);
        if index > 10_000 {
            return Err(RenderError::Job("too many segments".into()));
        }
    }
    Ok(plans)
}

/// True when a segment file already exists and has non-zero size (usable for --resume).
pub fn segment_file_ready(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(m) => m.is_file() && m.len() > 0,
        Err(_) => false,
    }
}

/// Concat demuxer: write list file and run ffmpeg -c copy.
pub fn concat_segments(parts: &[PathBuf], output: &Path) -> Result<(), RenderError> {
    if parts.is_empty() {
        return Err(RenderError::EmptyOutput);
    }
    if parts.len() == 1 {
        if parts[0] != output {
            fs::copy(&parts[0], output).map_err(|source| RenderError::Io {
                path: output.to_path_buf(),
                source,
            })?;
        }
        return Ok(());
    }
    let list_path = output.with_extension("concat.txt");
    let mut body = String::new();
    for p in parts {
        // ffmpeg concat demuxer requires escaped single quotes in paths
        let s = p.display().to_string().replace('\'', "'\\''");
        body.push_str(&format!("file '{s}'\n"));
    }
    fs::write(&list_path, body).map_err(|source| RenderError::Io {
        path: list_path.clone(),
        source,
    })?;
    // Remove the list file on every path — including a failed ffmpeg spawn.
    struct ListGuard(PathBuf);
    impl Drop for ListGuard {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let _guard = ListGuard(list_path);
    let out = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
        ])
        .arg(&_guard.0)
        .args(["-c", "copy"])
        .arg(output)
        .output()
        .map_err(|e| RenderError::Ffmpeg(e.to_string()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(RenderError::Ffmpeg(format!("concat failed: {err}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn job(dur: u64, seg: Option<u64>) -> RenderJob {
        RenderJob {
            effect: "beams".into(),
            plugin_path: None,
            seed: 1,
            fps: 30,
            duration: Duration::from_secs(dur),
            width: 64,
            height: 64,
            output: PathBuf::from("/tmp/master.mkv"),
            cols: None,
            rows: None,
            dry_run: true,
            segment: seg.map(Duration::from_secs),
            audio: None,
            resume: false,
            crf: 35,
            preset: None,
            encoder: None,
            prefer_hw: true,
            gpu_upscale: true,
            format: crate::models::OutputFormat::Mp4,
            container: crate::models::Container::Mkv,
            baseline_dir: None,
            snapshot_last_only: false,
            update_baselines: false,
            cpu_raster: false,
        }
    }

    #[test]
    fn missing_part_not_ready() {
        assert!(!segment_file_ready(Path::new(
            "/tmp/definitely-missing-idle-render-part.mkv"
        )));
    }

    #[test]
    fn no_segment_is_single() {
        let p = plan_segments(&job(100, None)).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].path, PathBuf::from("/tmp/master.mkv"));
    }

    #[test]
    fn three_parts() {
        let p = plan_segments(&job(300, Some(100))).unwrap();
        assert_eq!(p.len(), 3);
        assert!(p[0].path.to_string_lossy().contains("part000"));
        assert!(p[2].path.to_string_lossy().contains("part002"));
    }
}
