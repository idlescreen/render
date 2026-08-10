//! IdleScreen **render** — offline export capability (library + `render` CLI).
//!
//! Studio is the UI; it drives this crate via [`JobSpec`] / `render --job-file`.
//! Frame resize (upscale) is an internal step of the pipeline, not a separate product.

pub mod audio;
pub mod cli;
pub mod duration;
pub mod encode;
pub mod encode_select;
pub mod error;
pub mod job_spec;
pub mod models;
pub mod paths;
pub mod pipeline;
pub mod pipeline_snapshot;
pub mod png_writer;
pub mod segment;
pub mod snapshot;

pub use duration::parse_duration_secs;
pub use encode::{encode_raw_bgra_to_file, EncodeBackend, EncodeSettings};
pub use encode_select::{detect_av1_encoder, is_hardware_encoder};
pub use error::RenderError;
pub use job_spec::JobSpec;
pub use models::RenderJob;
pub use pipeline::{run_pipeline, PipelineResult};
pub use png_writer::{encode_bgra_frame_to_png, encode_bgra_frame_to_png_result};

#[cfg(test)]
mod proptests;
