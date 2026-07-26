use crate::duration::parse_duration_secs;
use crate::encode::EncodeBackend;
use crate::error::RenderError;
use crate::job_spec::JobSpec;
use crate::models::RenderJob;
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

    /// Write raw BGRA dump instead of AV1 (debug/tests)
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
