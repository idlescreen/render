#[cfg(test)]
mod job_props {
    use crate::job::StudioJob;
    use idle_render::JobSpec;
    use proptest::prelude::*;
    use std::path::PathBuf;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn job_file_roundtrips_effect(
            effect in "[a-z]{3,12}",
            seed: u64,
            fps in 1u32..=120,
        ) {
            let j = StudioJob {
                id: "t".into(),
                spec: JobSpec {
                    effect: effect.clone(),
                    plugin_path: None,
                    seed,
                    fps,
                    duration: "10s".into(),
                    output: PathBuf::from("/tmp/out.mkv"),
                    width: 1280,
                    height: 720,
                    cols: None,
                    rows: None,
                    dry_run: true,
                    raw: false,
                    segment: None,
                    audio: None,
                    resume: false,
                    crf: 35,
                    preset: None,
                    encoder: None,
                    prefer_hw: true,
                    gpu_upscale: true,
                    format: None,
                    container: None,
                    baseline_dir: None,
                    snapshot_last_only: false,
                    update_baselines: false,
                    cpu_raster: false,
                },
            };
            let path = j.write_job_file().expect("write");
            let loaded = JobSpec::load_path(&path).expect("load");
            let _ = std::fs::remove_file(&path);
            prop_assert_eq!(loaded.effect, effect);
            prop_assert!(loaded.dry_run);
        }
    }
}
