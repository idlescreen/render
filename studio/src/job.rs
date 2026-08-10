//! Studio queue entry = render [`JobSpec`] + id.
//!
//! Studio does not reimplement export. It stores jobs and asks `render` to run them.

use idle_render::JobSpec;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One queued item for the Director UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StudioJob {
    pub id: String,
    #[serde(flatten)]
    pub spec: JobSpec,
}

impl StudioJob {
    pub fn new(id: String, spec: JobSpec) -> Self {
        Self { id, spec }
    }

    /// Write a temp job-file and return its path (caller deletes).
    pub fn write_job_file(&self) -> Result<PathBuf, String> {
        let path = std::env::temp_dir().join(format!("idle-studio-{}.json", self.id));
        self.spec
            .save_path(&path)
            .map_err(|e| e.to_string())?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_roundtrip() {
        let j = StudioJob {
            id: "1".into(),
            spec: JobSpec {
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
                dry_run: false,
                raw: false,
                segment: Some("5s".into()),
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
        let s = serde_json::to_string(&j).expect("ser");
        let back: StudioJob = serde_json::from_str(&s).expect("de");
        assert_eq!(back.id, "1");
        assert_eq!(back.spec.effect, "ripple");
        assert_eq!(back.spec.segment.as_deref(), Some("5s"));
    }
}
