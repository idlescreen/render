//! Property tests for parse/plan protocol logic.

#[cfg(test)]
mod duration_props {
    use crate::duration::parse_duration_secs;
    use proptest::prelude::*;
    use std::time::Duration;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn seconds_suffix_roundtrip(n in 1u64..=10_000) {
            let d = parse_duration_secs(&format!("{n}s")).expect("parse");
            prop_assert_eq!(d, Duration::from_secs(n));
        }

        #[test]
        fn bare_number_is_seconds(n in 1u64..=10_000) {
            let d = parse_duration_secs(&n.to_string()).expect("parse");
            prop_assert_eq!(d, Duration::from_secs(n));
        }

        #[test]
        fn zero_always_errors(unit in prop::sample::select(vec!["", "s", "m", "h", "d"])) {
            let raw = if unit.is_empty() { "0".into() } else { format!("0{unit}") };
            prop_assert!(parse_duration_secs(&raw).is_err());
        }
    }
}

#[cfg(test)]
mod segment_props {
    use crate::models::RenderJob;
    use crate::segment::plan_segments;
    use proptest::prelude::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn job(total: u64, seg: u64) -> RenderJob {
        RenderJob {
            effect: "beams".into(),
            plugin_path: None,
            seed: 1,
            fps: 30,
            duration: Duration::from_secs(total.max(1)),
            width: 64,
            height: 64,
            output: PathBuf::from("/tmp/master.mkv"),
            cols: None,
            rows: None,
            dry_run: true,
            segment: Some(Duration::from_secs(seg.max(1))),
            audio: None,
            resume: false,
            crf: 35,
            preset: None,
            encoder: None,
            prefer_hw: true,
            gpu_upscale: true,
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        #[test]
        fn plan_len_matches_segment_count(total in 1u64..=50_000, seg in 1u64..=10_000) {
            let j = job(total, seg);
            let plans = plan_segments(&j).expect("plan");
            prop_assert_eq!(plans.len() as u64, j.segment_count());
            let sum: u64 = plans.iter().map(|p| p.duration.as_secs()).sum();
            // saturating segments may overshoot last part only by covering total
            prop_assert!(sum >= j.duration.as_secs() || plans.len() == 1);
        }
    }
}
