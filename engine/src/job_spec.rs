//! JSON job contract for Studio (and any other driver).
//!
//! Studio writes this file; `render --job-file` runs it. Capability lives here.

use crate::duration::parse_duration_secs;
use crate::encode::EncodeBackend;
use crate::error::RenderError;
use crate::models::RenderJob;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Serializable render request (string durations, JSON-friendly).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobSpec {
    pub effect: String,
    #[serde(default)]
    pub plugin_path: Option<PathBuf>,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// Human duration: `10s`, `5m`, `2h`, `1d`, or bare seconds.
    pub duration: String,
    pub output: PathBuf,
    #[serde(default = "default_w")]
    pub width: u32,
    #[serde(default = "default_h")]
    pub height: u32,
    #[serde(default)]
    pub cols: Option<usize>,
    #[serde(default)]
    pub rows: Option<usize>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub raw: bool,
    #[serde(default)]
    pub segment: Option<String>,
    #[serde(default)]
    pub audio: Option<PathBuf>,
    #[serde(default)]
    pub resume: bool,
    #[serde(default = "default_crf")]
    pub crf: u8,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub encoder: Option<String>,
    /// Prefer hardware AV1 when auto-detecting (default true).
    #[serde(default = "default_true")]
    pub prefer_hw: bool,
    /// Request GPU upscale path (default true).
    #[serde(default = "default_true")]
    pub gpu_upscale: bool,
}

fn default_seed() -> u64 {
    0x00C0_FFEE
}
fn default_fps() -> u32 {
    30
}
fn default_w() -> u32 {
    1280
}
fn default_h() -> u32 {
    720
}
fn default_crf() -> u8 {
    35
}
fn default_true() -> bool {
    true
}

impl JobSpec {
    pub fn load_path(path: &Path) -> Result<Self, RenderError> {
        let raw = std::fs::read_to_string(path).map_err(|source| RenderError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_str(&raw).map_err(|e| {
            RenderError::Job(format!("invalid job file {}: {e}", path.display()))
        })
    }

    pub fn save_path(&self, path: &Path) -> Result<(), RenderError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|source| RenderError::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| RenderError::Job(format!("serialize job: {e}")))?;
        std::fs::write(path, raw).map_err(|source| RenderError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn into_job(self) -> Result<(RenderJob, EncodeBackend), RenderError> {
        let duration = parse_duration_secs(&self.duration)?;
        let segment = match self.segment {
            Some(s) => Some(parse_duration_secs(&s)?),
            None => None,
        };
        let job = RenderJob {
            effect: self.effect,
            plugin_path: self.plugin_path,
            seed: self.seed,
            fps: self.fps,
            duration,
            width: self.width,
            height: self.height,
            output: self.output,
            cols: self.cols,
            rows: self.rows,
            dry_run: self.dry_run,
            segment,
            audio: self.audio,
            resume: self.resume,
            crf: self.crf,
            preset: self.preset,
            encoder: self.encoder,
            prefer_hw: self.prefer_hw,
            gpu_upscale: self.gpu_upscale,
        };
        job.validate()?;
        let backend = if self.raw || job.dry_run {
            EncodeBackend::RawDump
        } else {
            EncodeBackend::FfmpegAv1
        };
        Ok((job, backend))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_json() {
        let spec = JobSpec {
            effect: "ripple".into(),
            plugin_path: None,
            seed: 1,
            fps: 30,
            duration: "10s".into(),
            output: PathBuf::from("/tmp/o.mkv"),
            width: 1280,
            height: 720,
            cols: None,
            rows: None,
            dry_run: true,
            raw: false,
            segment: Some("5s".into()),
            audio: None,
            resume: true,
            crf: 35,
            preset: Some("10".into()),
            encoder: None,
            prefer_hw: true,
            gpu_upscale: true,
        };
        let s = serde_json::to_string(&spec).expect("ser");
        let back: JobSpec = serde_json::from_str(&s).expect("de");
        assert_eq!(back.effect, "ripple");
        assert_eq!(back.segment.as_deref(), Some("5s"));
        let (job, _) = back.into_job().expect("job");
        assert_eq!(job.frame_count(), 300);
        assert!(job.resume);
    }
}
