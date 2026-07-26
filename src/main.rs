//! render — offline IdleScreen exporter.

use clap::Parser;
use idle_render::cli::Args;
use idle_render::pipeline::run_pipeline;
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
        eprintln!(
            "dry-run: effect={} seed={} fps={} frames={} segments={} crf={} resume={} {}x{} -> {}",
            job.effect,
            job.seed,
            job.fps,
            job.frame_count(),
            job.segment_count(),
            job.crf,
            job.resume,
            job.width,
            job.height,
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
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("render: {e}");
            ExitCode::from(1)
        }
    }
}
