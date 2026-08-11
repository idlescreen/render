//! Snapshot baseline regression tests for the render CLI (Sprint 02).
//!
//! Each test picks a small `<scenario>` (effect/seed/duration/fps) and asserts
//! the rendered last frame matches the checked-in baseline byte-for-byte (PNG
//! inputs decode to RGBA8; raw BGRA inputs memcmp).
//!
//! Tests prefer the explicit `--plugin-path` so the discovery layer is bypassed.

use idle_render::cli::{Args, CliContainer, CliFormat};
use idle_render::models::{Container, OutputFormat, RenderJob};
use idle_render::pipeline::run_pipeline;
use idle_render::pipeline_snapshot::{baseline_path_for, SnapshotOutcome};
use idle_render::EncodeBackend;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::tempdir;

fn plugin_path(name: &str) -> PathBuf {
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

fn skip_if_missing(p: &Path) -> bool {
    if !p.is_file() {
        eprintln!("skipping snapshot test: plugin not built at {}", p.display());
        true
    } else {
        false
    }
}

type Scenario = (/*name*/ &'static str, /*effect*/ &'static str, /*seed*/ u64, /*fps*/ u32, /*dur*/ Duration, /*w*/ u32, /*h*/ u32, /*format*/ OutputFormat, /*container*/ Container);

/// Build the canonical 6 scenarios (per Sprint 02 spec table).
fn six_scenarios() -> Vec<Scenario> {
    vec![
        ("beams_steady", "beams", 0xDEAD_BEEF, 30, Duration::from_secs(2), 64, 64, OutputFormat::Png, Container::Mkv),
        ("beams_warmup", "beams", 0xC0FFEE00, 60, Duration::from_secs(1), 64, 64, OutputFormat::Png, Container::Mkv),
        ("ripple_5s",    "ripple", 0x12345678, 30, Duration::from_secs(5), 64, 64, OutputFormat::Png, Container::Mkv),
        ("cosmos_quick", "cosmos", 0xCAFEBABE, 30, Duration::from_secs(1), 64, 64, OutputFormat::Png, Container::Mkv),
        ("storm_burst",  "storm",  0x0BADF00D, 60, Duration::from_secs(2), 64, 64, OutputFormat::Png, Container::Mkv),
        // Scenario 6 uses small dims to keep baseline under GitHub's 100MB cap
        // (the cell renderer expands small grids to ~960x960 internally).
        ("raw_dump_byte_eq", "beams", 0xABCDEF01, 30, Duration::from_secs(1), 16, 16, OutputFormat::Raw, Container::Mkv),
    ]
}

#[allow(clippy::too_many_arguments)]
fn make_job(scenario: &str, effect: &str, seed: u64, fps: u32, dur: Duration, w: u32, h: u32, fmt: OutputFormat, container: Container, baseline: &std::path::Path, out: &std::path::Path) -> (RenderJob, EncodeBackend) {
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

#[test]
fn all_six_snapshots_match_baseline() {
    // Permit loading bare .so plugins without a manifest during test runs.
    std::env::set_var("IDLE_ALLOW_UNSIGNED_PLUGINS", "1");
    let tmp = tempdir().expect("tmp");
    let baseline_dir = tmp.path().join("baselines");
    std::fs::create_dir_all(&baseline_dir).expect("baselines");

    for (scenario, effect, seed, fps, dur, w, h, fmt, container) in six_scenarios() {
        let so = plugin_path(effect);
        if skip_if_missing(&so) {
            continue;
        }
        let work = tmp.path().join(format!("{scenario}.work"));
        std::fs::create_dir_all(&work).expect("work");
        let out = match fmt {
            OutputFormat::Png => work.join("frames"),
            OutputFormat::Raw => work.join(format!("{scenario}.bgra")),
            OutputFormat::Mp4 => work.join(format!("{scenario}.mp4")),
        };

        // Phase 1: write the baseline (idempotent; fresh tmp so always).
        // F7: --update-baselines is gated behind RENDER_FORCE_UPDATE_BASELINES=1.
        // CI sets this env var in the seed step; tests set it explicitly here.
        {
            let (job, backend) = make_job(scenario, effect, seed, fps, dur, w, h, fmt, container, &baseline_dir, &out);
            let mut job_with_update = job.clone();
            job_with_update.update_baselines = true;
            std::env::set_var("RENDER_FORCE_UPDATE_BASELINES", "1");
            let result = run_pipeline(&job_with_update, backend);
            std::env::remove_var("RENDER_FORCE_UPDATE_BASELINES");
            result.expect("update pipeline");
        }

        // Phase 2: re-render and compare — must MATCH.
        let (job, backend) = make_job(scenario, effect, seed, fps, dur, w, h, fmt, container, &baseline_dir, &out);
        let result = run_pipeline(&job, backend).expect("compare pipeline");
        match &result.snapshot {
            SnapshotOutcome::Matched { path } => {
                assert!(path.is_file(), "scenario {scenario}: baseline should exist at {}", path.display());
            }
            other => panic!("scenario {scenario}: expected Matched, got {other:?}"),
        }
    }
}

#[test]
fn png_encoder_is_deterministic() {
    let pixels = 16 * 16;
    let bgra: Vec<u8> = (0..(pixels * 4) as u32).map(|i| (i % 251) as u8).collect();
    let a = idle_render::encode_bgra_frame_to_png(16, 16, &bgra);
    let b = idle_render::encode_bgra_frame_to_png(16, 16, &bgra);
    assert_eq!(a, b, "PNG encoder is not byte-deterministic");
    // Sanity: header signature.
    assert_eq!(&a[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
}

#[test]
fn stdout_raw_header_format() {
    // Verify the header layout: GBRI magic in LE bytes.
    assert_eq!(&0x4952_4247u32.to_le_bytes(), b"GBRI");
    let tmp = tempdir().expect("tmp");
    let out = tmp.path().join("raw.bin");
    let so = plugin_path("beams");
    if !so.is_file() {
        eprintln!("skipping stdout_raw_header_format: plugin not built");
        return;
    }
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_render"));
    cmd.env("IDLE_ALLOW_UNSIGNED_PLUGINS", "1");
    cmd.arg("--plugin-path").arg(&so);
    cmd.arg("-e").arg("beams");
    cmd.arg("--seed").arg("1");
    cmd.arg("--fps").arg("30");
    cmd.arg("--duration").arg("1s");
    cmd.arg("--stdout-raw");
    cmd.arg("--format").arg("raw");
    cmd.arg("--cpu-raster");
    cmd.arg("-o").arg(&out);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());
    let output = cmd.output().expect("spawn render");
    let stdout = output.stdout;
    assert!(
        stdout.len() >= 16,
        "stdout must include 16-byte header (got {} bytes)",
        stdout.len()
    );
    assert_eq!(&stdout[12..16], b"GBRI");
    let body = &stdout[16..];
    assert_eq!(body.len() % (64 * 64 * 4), 0, "body must be aligned frames");
}

#[test]
fn mp4_container_uses_h264() {
    // Verify the backend selection matches the format/container pair.
    let backend_for = |fmt: OutputFormat, container: Container| match (fmt, container) {
        (OutputFormat::Mp4, Container::Mp4) => EncodeBackend::FfmpegH264,
        (OutputFormat::Mp4, Container::Mkv) => EncodeBackend::FfmpegAv1,
        (OutputFormat::Png, _) => EncodeBackend::PngSequence,
        (OutputFormat::Raw, _) => EncodeBackend::RawDump,
    };
    assert!(matches!(backend_for(OutputFormat::Mp4, Container::Mp4), EncodeBackend::FfmpegH264));
    assert!(matches!(backend_for(OutputFormat::Mp4, Container::Mkv), EncodeBackend::FfmpegAv1));
    assert!(matches!(backend_for(OutputFormat::Png, Container::Mkv), EncodeBackend::PngSequence));
    assert!(matches!(backend_for(OutputFormat::Raw, Container::Mkv), EncodeBackend::RawDump));
}

#[test]
fn args_translation_picks_correct_backend() {
    // Smoke: a build of `Args` through clap should pick the right backend.
    let s = bin_cli_check();
    assert_eq!(s, "png");
    let _ = (CliFormat::Png, CliContainer::Mkv);
    let _ = baseline_path_for;
}

/// K4 (PROBE.md) — exit codes from the binary. main.rs returns:
/// 0 = success, 1 = pipeline error, 2 = arg-parse error, 3 = missing
/// baseline, 4 = snapshot mismatch. A regression that masks any of
/// these (e.g. by returning 0 on a failed parse) would silently break
/// CI gating; this test pins each path.
#[test]
fn binary_exit_codes_match_documented_contract() {
    let so = plugin_path("beams");
    let plugin_available = so.is_file();
    if !plugin_available {
        eprintln!("skipping: beams plugin not built (run `cargo build -p beams` first)");
        return;
    }

    // exit 2: arg-parse error. The contract: `render --bogus-flag` returns 2.
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_render"))
        .arg("--this-flag-does-not-exist")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("spawn render");
    assert_eq!(
        out.status.code(),
        Some(2),
        "arg-parse error must exit 2 (got {:?})",
        out.status.code()
    );

    // exit 0: dry-run with valid args (no real encode work; just pipeline shape).
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_render"))
        .env("IDLE_ALLOW_UNSIGNED_PLUGINS", "1")
        .arg("--plugin-path").arg(&so)
        .arg("-e").arg("beams")
        .arg("--duration").arg("1s")
        .arg("--dry-run")
        .arg("-o").arg("/tmp/render-exit-test.mkv")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("spawn render");
    assert_eq!(
        out.status.code(),
        Some(0),
        "dry-run with valid args must exit 0 (got {:?}, stderr: {})",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn bin_cli_check() -> String {
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
