use crate::error::StudioError;
use crate::job::StudioJob;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub job: StudioJob,
    pub status: JobStatus,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobQueue {
    pub entries: Vec<QueueEntry>,
}

impl JobQueue {
    pub fn load(path: &Path) -> Result<Self, StudioError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path).map_err(|source| StudioError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self, path: &Path) -> Result<(), StudioError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| StudioError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(path, raw).map_err(|source| StudioError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn enqueue(&mut self, job: StudioJob) {
        self.entries.push(QueueEntry {
            job,
            status: JobStatus::Pending,
            message: String::new(),
        });
    }

    pub fn next_pending_index(&self) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.status == JobStatus::Pending)
    }
}

/// Default queue path under the user's config dir or CWD.
pub fn default_queue_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("idle-studio").join("queue.json");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("idle-studio")
            .join("queue.json");
    }
    PathBuf::from("idle-studio-queue.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use idle_render::JobSpec;

    #[test]
    fn roundtrip_queue() {
        let dir = std::env::temp_dir().join(format!("idle-studio-q-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("q.json");
        let mut q = JobQueue::default();
        q.enqueue(StudioJob {
            id: "a".into(),
            spec: JobSpec {
                effect: "beams".into(),
                plugin_path: None,
                seed: 1,
                fps: 30,
                duration: "1s".into(),
                output: PathBuf::from("/tmp/x.mkv"),
                width: 64,
                height: 64,
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
            },
        });
        q.save(&path).unwrap();
        let loaded = JobQueue::load(&path).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].status, JobStatus::Pending);
        assert_eq!(loaded.entries[0].job.spec.effect, "beams");
        let _ = fs::remove_dir_all(&dir);
    }
}
