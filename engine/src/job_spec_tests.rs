// SPDX-License-Identifier: Apache-2.0

//! JobSpec parse/serialize/convert tests.

use super::*;

fn sample_spec() -> JobSpec {
    JobSpec {
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
        format: Some("png".into()),
        container: Some("mp4".into()),
        baseline_dir: Some(PathBuf::from("/tmp/baselines")),
        snapshot_last_only: true,
        update_baselines: false,
        cpu_raster: true,
    }
}

#[test]
fn roundtrip_json() {
    let s = serde_json::to_string(&sample_spec()).expect("ser");
    let back: JobSpec = serde_json::from_str(&s).expect("de");
    assert_eq!(back.effect, "ripple");
    assert_eq!(back.segment.as_deref(), Some("5s"));
    assert_eq!(back.format.as_deref(), Some("png"));
    assert_eq!(back.container.as_deref(), Some("mp4"));
    assert!(back.snapshot_last_only);
    assert!(back.cpu_raster);
    let (job, backend) = back.into_job().expect("job");
    assert_eq!(job.frame_count(), 300);
    assert!(job.resume);
    assert_eq!(job.format, OutputFormat::Png);
    assert_eq!(job.container, Container::Mp4);
    assert!(matches!(backend, EncodeBackend::PngSequence));
}

#[test]
fn raw_flag_overrides_format_to_raw() {
    let mut spec = sample_spec();
    spec.effect = "beams".into();
    spec.seed = 0xDEAD_BEEF;
    spec.duration = "2s".into();
    spec.raw = true;
    spec.format = Some("mp4".into());
    spec.segment = None;
    spec.resume = false;
    spec.preset = None;
    let (job, backend) = spec.into_job().expect("job");
    assert_eq!(job.format, OutputFormat::Raw);
    assert!(matches!(backend, EncodeBackend::StdoutRaw));
}
