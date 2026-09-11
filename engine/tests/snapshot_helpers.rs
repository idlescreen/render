//! Helper functions for snapshot tests.

use idle_render::cli::Args;
use idle_render::models::{Container, OutputFormat, RenderJob};
use idle_render::pipeline_snapshot::baseline_path_for;
use idle_render::EncodeBackend;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Path to a plugin .so file by name.
pub fn plugin_path(name: &str) -> PathBuf {
    // CI / dev box: plugins live at $REPO_ROOT/idle-saver-{name}/target/release/.
    let repo_root = std::env::var("RENDER_REPO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            // CARGO_MANIFEST_DIR = render/engine; repo = render/
            let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            p.parent().unwrap().to_path_buf()
        });
    let candidate = repo_root
        .join(format!("../idle-saver-{name}"))
        .join("target")
        .join("release")
        .join(format!("libscreensaver_{name}.so"));
    // Resolve the `..` so validate()'s parent-dir guard accepts it.
    std::fs::canonicalize(&candidate).unwrap_or(candidate)
}

/// Return true if the plugin is missing (skip test).
pub fn skip_if_missing(p: &Path) -> bool {
    if !p.is_file() {
        eprintln!(
            "skipping snapshot test: plugin not built at {}",
            p.display()
        );
        true
    } else {
        false
    }
}

pub type Scenario = (
    /*name*/ &'static str,
    /*effect*/ &'static str,
    /*seed*/ u64,
    /*fps*/ u32,
    /*dur*/ Duration,
    /*w*/ u32,
    /*h*/ u32,
    /*format*/ OutputFormat,
    /*container*/ Container,
);

/// Build the canonical 6 scenarios (per Sprint 02 spec table).
pub fn six_scenarios() -> Vec<Scenario> {
    vec![
        (
            "beams_steady",
            "beams",
            0xDEAD_BEEF,
            30,
            Duration::from_secs(2),
            64,
            64,
            OutputFormat::Png,
            Container::Mkv,
        ),
        (
            "beams_warmup",
            "beams",
            0xC0FFEE00,
            60,
            Duration::from_secs(1),
            64,
            64,
            OutputFormat::Png,
            Container::Mkv,
        ),
        (
            "ripple_5s",
            "ripple",
            0x12345678,
            30,
            Duration::from_secs(5),
            64,
            64,
            OutputFormat::Png,
            Container::Mkv,
        ),
        (
            "cosmos_quick",
            "cosmos",
            0xCAFEBABE,
            30,
            Duration::from_secs(1),
            64,
            64,
            OutputFormat::Png,
            Container::Mkv,
        ),
        (
            "storm_burst",
            "storm",
            0x0BADF00D,
            60,
            Duration::from_secs(2),
            64,
            64,
            OutputFormat::Png,
            Container::Mkv,
        ),
        // Scenario 6 uses small dims to keep baseline under GitHub's 100MB cap
        // (the cell renderer expands small grids to ~960x960 internally).
        (
            "raw_dump_byte_eq",
            "beams",
            0xABCDEF01,
            30,
            Duration::from_secs(1),
            16,
            16,
            OutputFormat::Raw,
            Container::Mkv,
        ),
    ]
}

/// Build a RenderJob and pick the correct encode backend for a scenario.
#[allow(clippy::too_many_arguments)]
pub fn make_job(
    scenario: &str,
    effect: &str,
    seed: u64,
    fps: u32,
    dur: Duration,
    w: u32,
    h: u32,
    fmt: OutputFormat,
    container: Container,
    baseline: &std::path::Path,
    out: &std::path::Path,
) -> (RenderJob, EncodeBackend) {
    let plugin = plugin_path(effect);
    let job = RenderJob {
        effect: effect.into(),
        plugin_path: Some(plugin),
        seed,
        fps,
        duration: dur,
        width: w,
        height: h,
        output: out.to_path_buf(),
        cols: None,
        rows: None,
        dry_run: false,
        segment: None,
        audio: None,
        resume: false,
        crf: 35,
        preset: None,
        encoder: None,
        prefer_hw: false,
        gpu_upscale: false,
        format: fmt,
        container,
        baseline_dir: Some(baseline.to_path_buf()),
        snapshot_last_only: true,
        update_baselines: false,
        cpu_raster: true,
    };
    let _ = scenario;
    job.validate().expect("validate");
    // Format=Raw maps to file dump (RawDump); stdout variant is opt-in via CLI.
    let backend = match job.format {
        OutputFormat::Png => EncodeBackend::PngSequence,
        OutputFormat::Raw => EncodeBackend::RawDump,
        OutputFormat::Mp4 if matches!(job.container, Container::Mp4) => EncodeBackend::FfmpegH264,
        OutputFormat::Mp4 => EncodeBackend::FfmpegAv1,
    };
    (job, backend)
}

/// Helper for CLI argument parsing test.
pub fn bin_cli_check() -> String {
    // Build args using clap::Parser::try_parse_from with a fake argv.
    let args = <Args as clap::Parser>::try_parse_from([
        "render",
        "-e",
        "beams",
        "--plugin-path",
        "/tmp/x.so",
        "--duration",
        "1s",
        "--seed",
        "1",
        "--format",
        "png",
        "-o",
        "/tmp/out",
        "--baseline-dir",
        "/tmp/baselines",
        "--snapshot-last-only",
        "--cpu-raster",
    ])
    .expect("parse args");
    let (job, _backend) = args.into_job().expect("into_job");
    let scenario_path = baseline_path_for(&job).expect("baseline");
    format!("{}", scenario_path.extension().unwrap().to_string_lossy())
}
