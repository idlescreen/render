use crate::duration::parse_duration_secs;
use crate::encode::EncodeBackend;
use crate::error::RenderError;
use crate::models::RenderJob;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "render",
    about = "Offline IdleScreen effect renderer (saver math → AV1)"
)]
pub struct Args {
    /// Effect name (allowlisted saver basename, e.g. beams)
    #[arg(long, short = 'e')]
    pub effect: String,

    /// Explicit path to plugin .so (skips discovery)
    #[arg(long)]
    pub plugin_path: Option<PathBuf>,

    /// RNG seed exported to plugins via RENDER_SEED / IDLE_RENDER_SEED / TRANCE_SEED
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

    /// Optional audio bed (muxed after video; looped/cut to fit)
    #[arg(long)]
    pub audio: Option<PathBuf>,

    /// Output path (.mkv recommended)
    #[arg(long, short = 'o')]
    pub output: PathBuf,

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

    /// Resume: skip encode for existing non-empty segment parts (still advances sim)
    #[arg(long)]
    pub resume: bool,

    /// AV1 CRF (0–63, lower = better/larger). Default 35.
    #[arg(long, default_value_t = 35)]
    pub crf: u8,

    /// Encoder preset (e.g. SVT-AV1: higher is faster, try 10–12 for long jobs)
    #[arg(long)]
    pub preset: Option<String>,

    /// Force ffmpeg video encoder name (default: first available AV1)
    #[arg(long)]
    pub encoder: Option<String>,
}

impl Args {
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
        };
        job.validate()?;
        let backend = if self.raw || self.dry_run {
            EncodeBackend::RawDump
        } else {
            EncodeBackend::FfmpegAv1
        };
        Ok((job, backend))
    }
}
