use std::path::PathBuf;

/// Fallible render outcomes.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("invalid duration: {0}")]
    Duration(String),
    #[error("invalid job: {0}")]
    Job(String),
    #[error("plugin load failed: {0}")]
    Plugin(String),
    #[error("font/raster unavailable: {0}")]
    Raster(String),
    #[error("ffmpeg not found on PATH")]
    FfmpegMissing,
    #[error("ffmpeg failed: {0}")]
    Ffmpeg(String),
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("encode cancelled or wrote zero frames")]
    EmptyOutput,
}
