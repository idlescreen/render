use crate::duration::parse_duration_secs;
use crate::encode::EncodeBackend;
use crate::error::RenderError;
use crate::job_spec::JobSpec;
use crate::models::{Container, OutputFormat, RenderJob};
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "render",
    about = "IdleScreen offline export capability (saver math → AV1)"
)]
pub struct Args {
    /// JSON job file (Studio / automation). When set, flag fields below are ignored.
    #[arg(long)]
    pub job_file: Option<PathBuf>,

    /// Effect name (allowlisted saver basename, e.g. ripple)
    #[arg(long, short = 'e', required_unless_present = "job_file")]
    pub effect: Option<String>,

    /// Explicit path to plugin .so (skips discovery)
    #[arg(long)]
    pub plugin_path: Option<PathBuf>,

    /// RNG seed
    #[arg(long, default_value_t = 0x00C0_FFEEu64)]
    pub seed: u64,

    /// Output timeline fps
    #[arg(long, default_value_t = 30)]
    pub fps: u32,

    /// Duration: 10s, 5m, 2h, 1d (or bare seconds)
    #[arg(long, default_value = "10s")]
    pub duration: String,

    /// Optional segment length for long encodes (e.g. 1h)
    #[arg(long)]
    pub segment: Option<String>,

    /// Optional audio bed (muxed after video)
    #[arg(long)]
    pub audio: Option<PathBuf>,

    /// Output path (.mkv recommended). Optional with `--dry-run` (plan only).
    #[arg(
        long,
        short = 'o',
        required_unless_present_any = ["job_file", "dry_run"]
    )]
    pub output: Option<PathBuf>,

    /// Pixel width
    #[arg(long, default_value_t = 1280)]
    pub width: u32,

    /// Pixel height
    #[arg(long, default_value_t = 720)]
    pub height: u32,

    /// Optional simulation grid columns
    #[arg(long)]
    pub cols: Option<usize>,

    /// Optional simulation grid rows
    #[arg(long)]
    pub rows: Option<usize>,

    /// Validate and print plan only
    #[arg(long)]
    pub dry_run: bool,

    /// Write raw BGRA dump instead of AV1 (debug/tests). Alias for `--format raw`.
    #[arg(long)]
    pub raw: bool,

    /// Resume: skip encode for existing non-empty segment parts
    #[arg(long)]
    pub resume: bool,

    /// AV1 quality 0–63 (CRF / CQ). Default 35.
    #[arg(long, default_value_t = 35)]
    pub crf: u8,

    /// Encoder preset (SVT numeric or NVENC p1–p7)
    #[arg(long)]
    pub preset: Option<String>,

    /// Force ffmpeg video encoder name
    #[arg(long)]
    pub encoder: Option<String>,

    /// Force software AV1 only
    #[arg(long)]
    pub no_hw_encode: bool,

    /// Force CPU upscale
    #[arg(long)]
    pub no_gpu_upscale: bool,

    /// Output family: `mp4` (video, default), `png` (per-frame), `raw` (stdout BGRA).
    #[arg(long, value_enum, default_value_t = CliFormat::Mp4)]
    pub format: CliFormat,

    /// Container for video output: `mkv` (AV1 default) or `mp4` (H.264 default).
    #[arg(long, value_enum, default_value_t = CliContainer::Mkv)]
    pub container: CliContainer,

    /// Stream raw BGRA + 16-byte header to stdout. Equivalent to `--format raw`.
    #[arg(long)]
    pub stdout_raw: bool,

    /// Snapshot compare directory (when set, --snapshot-last-only compares last frame).
    #[arg(long)]
    pub baseline_dir: Option<PathBuf>,

    /// Overwrite baseline files instead of comparing (dev only).
    #[arg(long)]
    pub update_baselines: bool,

    /// Only compare final frame against baseline (skips all-but-last encode work).
    #[arg(long)]
    pub snapshot_last_only: bool,

    /// Force CPU rendering path (deterministic; bypass GPU variance for tests).
    #[arg(long)]
    pub cpu_raster: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, PartialEq, Eq)]
pub enum CliFormat {
    Mp4,
    Png,
    Raw,
}

impl From<CliFormat> for OutputFormat {
    fn from(v: CliFormat) -> Self {
        match v {
            CliFormat::Mp4 => OutputFormat::Mp4,
            CliFormat::Png => OutputFormat::Png,
            CliFormat::Raw => OutputFormat::Raw,
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, PartialEq, Eq)]
pub enum CliContainer {
    Mkv,
    Mp4,
}

impl From<CliContainer> for Container {
    fn from(v: CliContainer) -> Self {
        match v {
            CliContainer::Mkv => Container::Mkv,
            CliContainer::Mp4 => Container::Mp4,
        }
    }
}

impl Args {
    pub fn into_job(self) -> Result<(RenderJob, EncodeBackend), RenderError> {
        if let Some(path) = self.job_file {
            return JobSpec::load_path(&path)?.into_job();
        }
        let effect = self
            .effect
            .ok_or_else(|| RenderError::Job("--effect required without --job-file".into()))?;
        let output = match self.output {
            Some(p) => p,
            None if self.dry_run => PathBuf::from("dry-run.mkv"),
            None => {
                return Err(RenderError::Job(
                    "--output required without --job-file (unless --dry-run)".into(),
                ));
            }
        };
        let duration = parse_duration_secs(&self.duration)?;
        let segment = match self.segment {
            Some(s) => Some(parse_duration_secs(&s)?),
            None => None,
        };
        // `--raw` and `--stdout-raw` both pin format to Raw.
        let format: OutputFormat = if self.raw || self.stdout_raw {
            OutputFormat::Raw
        } else {
            self.format.into()
        };
        let job = RenderJob {
            effect,
            plugin_path: self.plugin_path,
            seed: self.seed,
            fps: self.fps,
            duration,
            width: self.width,
            height: self.height,
            output,
            cols: self.cols,
            rows: self.rows,
            dry_run: self.dry_run,
            segment,
            audio: self.audio,
            resume: self.resume,
            crf: self.crf,
            preset: self.preset,
            encoder: self.encoder,
            prefer_hw: !self.no_hw_encode,
            gpu_upscale: !self.no_gpu_upscale,
            format,
            container: self.container.into(),
            baseline_dir: self.baseline_dir,
            snapshot_last_only: self.snapshot_last_only,
            update_baselines: self.update_baselines,
            cpu_raster: self.cpu_raster,
        };
        job.validate()?;
        let backend = match job.format {
            OutputFormat::Png => EncodeBackend::PngSequence,
            OutputFormat::Raw => EncodeBackend::StdoutRaw,
            OutputFormat::Mp4 if job.dry_run => EncodeBackend::RawDump,
            OutputFormat::Mp4 if matches!(job.container, Container::Mp4) => {
                EncodeBackend::FfmpegH264
            }
            OutputFormat::Mp4 => EncodeBackend::FfmpegAv1,
        };
        Ok((job, backend))
    }
}
