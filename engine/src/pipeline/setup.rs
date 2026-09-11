// SPDX-License-Identifier: Apache-2.0

//! Pipeline setup helpers: export env, plugin resolution, encoder settings,
//! frame counting, and resume fast-forward.

use crate::encode::EncodeSettings;
use crate::error::RenderError;
use crate::models::RenderJob;
use idle_runner::plugin_session::PluginSession;
use std::time::Duration;

pub fn export_seed_env(seed: u64) {
    // SAFETY: single-threaded CLI before plugin load; values are numeric strings.
    unsafe {
        std::env::set_var("RENDER_SEED", seed.to_string());
        std::env::set_var("IDLE_RENDER_SEED", seed.to_string());
        std::env::set_var("TRANCE_SEED", seed.to_string());
        std::env::set_var("IDLE_DISABLE_SANDBOX", "1");
        // Release sandbox escape requires both flags (see idle-runner sandbox.rs).
        std::env::set_var("IDLE_RENDER_PIPELINE", "1");
        std::env::set_var("TRANCE_DISABLE_SANDBOX", "1");
        std::env::set_var("IDLE_EXPORT_MODE", "1");
        std::env::set_var("TRANCE_EXPORT_MODE", "1");
    }
}

pub(super) fn resolve_plugin(job: &RenderJob) -> Result<PluginSession, RenderError> {
    // job.gpu_upscale is retained on JobSpec for file back-compat but the
    // GPU path was removed in idle-runner 3.4 — it no longer reaches plugins.
    let scale = Some(1.0_f32);
    if let Some(path) = &job.plugin_path {
        return PluginSession::load_path_with_options(path, scale)
            .map_err(|e| RenderError::Plugin(e.to_string()));
    }
    PluginSession::load_with_options(
        &job.effect,
        &idle_runner::launcher::LaunchMode::Preview,
        scale,
    )
    .map_err(|e| RenderError::Plugin(e.to_string()))
}

pub(super) fn encode_settings(job: &RenderJob) -> EncodeSettings {
    EncodeSettings {
        crf: job.crf,
        preset: job.preset.clone(),
        encoder: job.encoder.clone(),
        prefer_hw: job.prefer_hw,
    }
}

pub(super) fn frames_for_duration(duration: Duration, fps: u32) -> u64 {
    let secs = duration.as_secs_f64();
    let n = (secs * f64::from(fps)).floor() as u64;
    n.max(1)
}

/// Advance simulation without raster/encode (used when --resume skips a part).
pub(super) fn fast_forward(session: &mut PluginSession, frames: u64, fps: u32) {
    let dt = Duration::from_secs_f64(1.0 / f64::from(fps));
    for _ in 0..frames {
        session.tick(dt);
    }
}
