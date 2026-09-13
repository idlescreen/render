use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum StudioError {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("render failed: {0}")]
    Render(String),
    #[error("render binary not found (set RENDER / IDLE_RENDER or PATH)")]
    RenderMissing,
    #[error("queue error: {0}")]
    Queue(String),
}
