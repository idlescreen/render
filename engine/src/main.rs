//! render — offline IdleScreen exporter.

use clap::Parser;
use idle_render::cli::Args;
use idle_render::models::{Container, OutputFormat};
use idle_render::pipeline::run_pipeline;
use idle_render::pipeline_snapshot::SnapshotOutcome;
use std::process::ExitCode;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let (job, backend) = match args.into_job() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("render: {e}");
            return ExitCode::from(2);
        }
    };

    if job.dry_run {
        let container = match (job.format, job.container) {
            (OutputFormat::Mp4, Container::Mkv) => "mkv/av1",
            (OutputFormat::Mp4, Container::Mp4) => "mp4/h264",
            (OutputFormat::Png, _) => "png-sequence",
            (OutputFormat::Raw, _) => "stdout-raw",
        };
        eprintln!(
            "dry-run: effect={} seed={} fps={} frames={} segments={} crf={} resume={} hw={} gpu_upscale={} {}x{} format={} -> {}",
            job.effect,
            job.seed,
            job.fps,
            job.frame_count(),
            job.segment_count(),
            job.crf,
            job.resume,
            job.prefer_hw,
            job.gpu_upscale,
            job.width,
            job.height,
            container,
            job.output.display()
        );
    }

    match run_pipeline(&job, backend) {
        Ok(r) => {
            eprintln!(
                "render: wrote {} frame(s) in {} segment(s) (resumed {}) to {}{}",
                r.frames,
                r.segments,
                r.resumed_segments,
                r.output.display(),
                if r.dry_run { " (dry-run)" } else { "" }
            );
            match &r.snapshot {
                SnapshotOutcome::Skipped => {}
                SnapshotOutcome::Matched { path } => eprintln!("snapshot: matched {}", path.display()),
                SnapshotOutcome::Updated { path } => eprintln!("snapshot: updated {}", path.display()),
                SnapshotOutcome::MissingBaseline { path } => {
                    eprintln!("snapshot: missing baseline at {} (run --update-baselines to seed)", path.display());
                    return ExitCode::from(3);
                }
                SnapshotOutcome::Mismatched { path, reason } => {
                    eprintln!("snapshot: mismatch at {} ({})", path.display(), reason);
                    return ExitCode::from(4);
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("render: {e}");
            ExitCode::from(1)
        }
    }
}
