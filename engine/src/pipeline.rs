use crate::audio::mux_audio_bed;
use crate::encode::{encode_raw_bgra_to_file, EncodeBackend, EncodeSettings};
use crate::error::RenderError;
use crate::models::RenderJob;
use crate::paths::ensure_parent_dir;
use crate::pipeline_snapshot::SnapshotOutcome;
use crate::segment::{concat_segments, plan_segments, segment_file_ready};
use idle_runner::plugin_session::PluginSession;
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod setup;
pub use setup::export_seed_env;
use setup::{encode_settings, fast_forward, frames_for_duration, resolve_plugin};

/// Outcome of a finished (or dry-run) pipeline.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub frames: u64,
    pub output: PathBuf,
    pub dry_run: bool,
    pub segments: u32,
    /// Segments skipped via --resume (existing part files).
    pub resumed_segments: u32,
    /// Snapshot compare result, when `--baseline-dir` was set.
    pub snapshot: SnapshotOutcome,
}

/// Export seed env vars so plugins using `idle_api::LcgRng::from_env_or_random` match.
#[allow(clippy::too_many_arguments)]
fn encode_one(
    session: &mut PluginSession,
    cols: usize,
    rows: usize,
    width: u32,
    height: u32,
    fps: u32,
    frames_total: u64,
    frame_offset: u64,
    output: &std::path::Path,
    backend: EncodeBackend,
    settings: &EncodeSettings,
) -> Result<u64, RenderError> {
    ensure_parent_dir(output)?;
    let dt = Duration::from_secs_f64(1.0 / f64::from(fps));
    let started = Instant::now();
    let mut index = 0u64;
    let iter = std::iter::from_fn(|| {
        if index >= frames_total {
            return None;
        }
        session.tick(dt);
        // Arc drops after write; session may mutate pixel_buf on next frame.
        let pixels = session.render(cols, rows, width, height);
        index += 1;
        let global = frame_offset + index;
        if global.is_multiple_of(30) || index == frames_total {
            let elapsed = started.elapsed().as_secs_f64().max(1e-6);
            let fps_eff = global as f64 / elapsed;
            let remain = frames_total.saturating_sub(index);
            let eta = remain as f64 / fps_eff.max(1e-6);
            tracing::info!(
                frame = global,
                fps = format!("{fps_eff:.1}"),
                eta_s = format!("{eta:.0}"),
                "render progress"
            );
        }
        Some(Ok(pixels))
    });
    encode_raw_bgra_to_file(backend, settings, width, height, fps, output, iter)
}

/// Run the offline simulation and encode loop (optional segments + audio).
pub fn run_pipeline(
    job: &RenderJob,
    backend: EncodeBackend,
) -> Result<PipelineResult, RenderError> {
    job.validate()?;
    export_seed_env(job.seed);

    let plans = plan_segments(job)?;
    let frames_total = job.frame_count();
    if job.dry_run {
        return Ok(PipelineResult {
            frames: frames_total,
            output: job.output.clone(),
            dry_run: true,
            segments: plans.len() as u32,
            resumed_segments: 0,
            snapshot: SnapshotOutcome::Skipped,
        });
    }

    ensure_parent_dir(&job.output)?;
    let settings = encode_settings(job);
    let mut session = resolve_plugin(job)?;
    let (cols, rows) = match (job.cols, job.rows) {
        (Some(c), Some(r)) => (c, r),
        _ => session.grid_for_pixels(job.width, job.height),
    };
    if cols == 0 || rows == 0 {
        return Err(RenderError::Job("grid cols/rows resolved to zero".into()));
    }
    session.init(cols, rows);
    session.set_simulation_rate(job.fps as f32);

    let mut written = 0u64;
    let mut frame_offset = 0u64;
    let mut resumed_segments = 0u32;
    let mut part_paths = Vec::new();
    for plan in &plans {
        let part_frames = frames_for_duration(plan.duration, job.fps);
        if job.resume && segment_file_ready(&plan.path) {
            tracing::info!(
                segment = plan.index,
                path = %plan.path.display(),
                "resume: skip encode, fast-forward sim"
            );
            fast_forward(&mut session, part_frames, job.fps);
            written += part_frames;
            frame_offset += part_frames;
            part_paths.push(plan.path.clone());
            resumed_segments += 1;
            continue;
        }
        let n = encode_one(
            &mut session,
            cols,
            rows,
            job.width,
            job.height,
            job.fps,
            part_frames,
            frame_offset,
            &plan.path,
            backend,
            &settings,
        )?;
        written += n;
        frame_offset += part_frames;
        part_paths.push(plan.path.clone());
        tracing::info!(
            segment = plan.index,
            frames = n,
            path = %plan.path.display(),
            "segment done"
        );
    }

    if part_paths.len() != 1 || part_paths[0] != job.output {
        concat_segments(&part_paths, &job.output)?;
    }

    if let Some(audio) = &job.audio {
        mux_audio_bed(&job.output, audio, &job.output)?;
    }

    let snapshot = crate::pipeline_snapshot::run_snapshot(job)?;
    Ok(PipelineResult {
        frames: written,
        output: job.output.clone(),
        dry_run: false,
        segments: plans.len() as u32,
        resumed_segments,
        snapshot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample_job(dur_secs: u64, seg: Option<u64>) -> RenderJob {
        RenderJob {
            effect: "beams".into(),
            plugin_path: None,
            seed: 42,
            fps: 30,
            duration: Duration::from_secs(dur_secs),
            width: 64,
            height: 64,
            output: PathBuf::from("/tmp/unused.mkv"),
            cols: None,
            rows: None,
            dry_run: true,
            segment: seg.map(Duration::from_secs),
            audio: None,
            resume: false,
            crf: 35,
            preset: None,
            encoder: None,
            prefer_hw: true,
            gpu_upscale: true,
            format: crate::models::OutputFormat::Mp4,
            container: crate::models::Container::Mkv,
            baseline_dir: None,
            snapshot_last_only: false,
            update_baselines: false,
            cpu_raster: false,
        }
    }

    #[test]
    fn dry_run_reports_frame_count_and_segments() {
        let job = sample_job(120, Some(60));
        let r = run_pipeline(&job, EncodeBackend::RawDump).expect("dry");
        assert_eq!(r.frames, 3600);
        assert_eq!(r.segments, 2);
        assert!(r.dry_run);
        assert_eq!(r.resumed_segments, 0);
    }
}
