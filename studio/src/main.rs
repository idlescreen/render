//! idle-studio — UI that drives the **render** capability (same repo).

use clap::{Parser, Subcommand};
use idle_render::JobSpec;
use idle_studio::job::StudioJob;
use idle_studio::queue::{default_queue_path, JobQueue, JobStatus};
use idle_studio::runner::run_job;
use idle_studio::tui::run_tui;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "idle-studio",
    about = "IdleScreen Studio — UI for the render capability"
)]
struct Args {
    /// Queue file (JSON)
    #[arg(long, global = true)]
    queue: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Interactive Director TUI
    Tui,
    /// Add a job to the queue (stored as render JobSpec)
    Enqueue {
        #[arg(long, short = 'e')]
        effect: String,
        #[arg(long, short = 'o')]
        output: PathBuf,
        #[arg(long, default_value = "10s")]
        duration: String,
        #[arg(long, default_value_t = 0x00C0_FFEEu64)]
        seed: u64,
        #[arg(long, default_value_t = 30)]
        fps: u32,
        #[arg(long, default_value_t = 1280)]
        width: u32,
        #[arg(long, default_value_t = 720)]
        height: u32,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        segment: Option<String>,
        #[arg(long)]
        audio: Option<PathBuf>,
        #[arg(long)]
        resume: bool,
        #[arg(long, default_value_t = 35)]
        crf: u8,
        #[arg(long)]
        preset: Option<String>,
        #[arg(long)]
        no_hw_encode: bool,
        #[arg(long)]
        no_gpu_upscale: bool,
    },
    /// List queue entries
    List,
    /// Run the next pending job (or all with --all)
    Run {
        #[arg(long)]
        all: bool,
    },
}

fn main() -> ExitCode {
    let args = Args::parse();
    let path = args.queue.unwrap_or_else(default_queue_path);

    if matches!(args.cmd, Cmd::Tui) {
        return match run_tui(&path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("idle-studio: {e}");
                ExitCode::from(1)
            }
        };
    }

    let mut queue = match JobQueue::load(&path) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("idle-studio: {e}");
            return ExitCode::from(2);
        }
    };

    match args.cmd {
        Cmd::Tui => ExitCode::SUCCESS,
        Cmd::Enqueue {
            effect,
            output,
            duration,
            seed,
            fps,
            width,
            height,
            dry_run,
            id,
            segment,
            audio,
            resume,
            crf,
            preset,
            no_hw_encode,
            no_gpu_upscale,
        } => {
            let id = id.unwrap_or_else(|| format!("job-{}", queue.entries.len() + 1));
            let spec = JobSpec {
                effect,
                plugin_path: None,
                seed,
                fps,
                duration,
                output,
                width,
                height,
                cols: None,
                rows: None,
                dry_run,
                raw: false,
                segment,
                audio,
                resume,
                crf,
                preset,
                encoder: None,
                prefer_hw: !no_hw_encode,
                gpu_upscale: !no_gpu_upscale,
            };
            queue.enqueue(StudioJob::new(id.clone(), spec));
            match queue.save(&path) {
                Ok(()) => {
                    eprintln!("enqueued {id} → {}", path.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("idle-studio: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Cmd::List => {
            for (i, e) in queue.entries.iter().enumerate() {
                println!(
                    "[{i}] {} {:?} {} -> {}",
                    e.job.id,
                    e.status,
                    e.job.spec.effect,
                    e.job.spec.output.display()
                );
            }
            ExitCode::SUCCESS
        }
        Cmd::Run { all } => {
            let mut code = ExitCode::SUCCESS;
            loop {
                let Some(idx) = queue.next_pending_index() else {
                    if !all {
                        eprintln!("no pending jobs");
                    }
                    break;
                };
                queue.entries[idx].status = JobStatus::Running;
                let _ = queue.save(&path);
                let job = queue.entries[idx].job.clone();
                match run_job(&job) {
                    Ok(msg) => {
                        queue.entries[idx].status = JobStatus::Done;
                        queue.entries[idx].message = msg;
                        eprintln!("done {}", job.id);
                    }
                    Err(e) => {
                        queue.entries[idx].status = JobStatus::Failed;
                        queue.entries[idx].message = e.to_string();
                        eprintln!("failed {}: {e}", job.id);
                        code = ExitCode::from(1);
                    }
                }
                let _ = queue.save(&path);
                if !all {
                    break;
                }
            }
            code
        }
    }
}
