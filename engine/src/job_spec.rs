//! JSON job contract for Studio (and any other driver).
//!
//! Studio writes this file; `render --job-file` runs it. Capability lives here.

use crate::duration::parse_duration_secs;
use crate::encode::EncodeBackend;
use crate::error::RenderError;
use crate::models::{Container, OutputFormat, RenderJob};
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
    /// Output family (`mp4` / `png` / `raw`). Default `mp4`.
    #[serde(default)]
    pub format: Option<String>,
    /// Container (`mp4` / `mkv`). Default `mkv`.
    #[serde(default)]
    pub container: Option<String>,
    /// Optional baseline directory for snapshot comparison.
    #[serde(default)]
    pub baseline_dir: Option<PathBuf>,
    /// Only compare final frame against baseline.
    #[serde(default)]
    pub snapshot_last_only: bool,
    /// Overwrite baseline files instead of comparing.
    #[serde(default)]
    pub update_baselines: bool,
    /// Force CPU raster (deterministic; bypass GPU variance).
    #[serde(default)]
    pub cpu_raster: bool,
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

fn parse_format(s: &str) -> Result<OutputFormat, RenderError> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "mp4" | "video" => OutputFormat::Mp4,
        "png" | "pngs" | "png-sequence" | "png_sequence" => OutputFormat::Png,
        "raw" | "stdout" | "stdout-raw" | "stdout_raw" => OutputFormat::Raw,
        other => return Err(RenderError::Job(format!("unknown format '{other}'"))),
    })
}
fn parse_container(s: &str) -> Result<Container, RenderError> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "mkv" | "matroska" => Container::Mkv,
        "mp4" => Container::Mp4,
        other => return Err(RenderError::Job(format!("unknown container '{other}'"))),
    })
}

impl JobSpec {
    pub fn load_path(path: &Path) -> Result<Self, RenderError> {
        let raw = std::fs::read_to_string(path).map_err(|source| RenderError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_str(&raw)
            .map_err(|e| RenderError::Job(format!("invalid job file {}: {e}", path.display())))
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
        // Atomic: tmp + rename so a crash can't leave a half-written job file
        // for `render --job-file` to choke on.
        let tmp = path.with_extension("job.tmp");
        std::fs::write(&tmp, raw).map_err(|source| RenderError::Io {
            path: tmp.clone(),
            source,
        })?;
        std::fs::rename(&tmp, path).map_err(|source| RenderError::Io {
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
        let format = if self.raw {
            OutputFormat::Raw
        } else {
            match &self.format {
                Some(s) => parse_format(s)?,
                None => OutputFormat::Mp4,
            }
        };
        let container = match &self.container {
            Some(s) => parse_container(s)?,
            None => Container::Mkv,
        };
        // `raw` in JSON also toggles the stdout-raw flag (legacy alias).
        let stdout_raw = self.raw;
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
            format,
            container,
            baseline_dir: self.baseline_dir,
            snapshot_last_only: self.snapshot_last_only,
            update_baselines: self.update_baselines,
            cpu_raster: self.cpu_raster,
        };
        job.validate()?;
        let backend = match job.format {
            OutputFormat::Png => EncodeBackend::PngSequence,
            OutputFormat::Raw if stdout_raw => EncodeBackend::StdoutRaw,
            OutputFormat::Raw => EncodeBackend::RawDump,
            OutputFormat::Mp4 if job.dry_run => EncodeBackend::RawDump,
            OutputFormat::Mp4 if matches!(job.container, Container::Mp4) => {
                EncodeBackend::FfmpegH264
            }
            OutputFormat::Mp4 => EncodeBackend::FfmpegAv1,
        };
        Ok((job, backend))
    }
}

#[cfg(test)]
#[path = "job_spec_tests.rs"]
mod tests;
