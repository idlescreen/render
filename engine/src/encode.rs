use crate::encode_select::{detect_av1_encoder, is_hardware_encoder, push_quality_args};
use crate::error::RenderError;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub use crate::encode_select::{EncodeSettings, HW_AV1_CANDIDATES, SW_AV1_CANDIDATES};

/// Legacy alias used in docs/tests.
pub const AV1_CANDIDATES: &[&str] = &[
    "av1_nvenc",
    "av1_qsv",
    "av1_amf",
    "libsvtav1",
    "libaom-av1",
    "librav1e",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeBackend {
    /// Pipe BGRA frames to ffmpeg AV1.
    FfmpegAv1,
    /// Write a single raw BGRA dump (tests / no ffmpeg).
    RawDump,
}

fn frame_bytes<B>(buf: &B) -> &[u8]
where
    B: std::ops::Deref,
    B::Target: AsRef<[u8]>,
{
    (*buf).as_ref()
}

/// Encode a sequence of BGRA frames by spawning ffmpeg (or raw dump).
pub fn encode_raw_bgra_to_file<I, B>(
    backend: EncodeBackend,
    settings: &EncodeSettings,
    width: u32,
    height: u32,
    fps: u32,
    output: &Path,
    frames: I,
) -> Result<u64, RenderError>
where
    I: Iterator<Item = Result<B, RenderError>>,
    B: std::ops::Deref,
    B::Target: AsRef<[u8]>,
{
    match backend {
        EncodeBackend::RawDump => write_raw_dump(output, frames),
        EncodeBackend::FfmpegAv1 => write_ffmpeg_av1(settings, width, height, fps, output, frames),
    }
}

fn write_raw_dump<I, B>(output: &Path, frames: I) -> Result<u64, RenderError>
where
    I: Iterator<Item = Result<B, RenderError>>,
    B: std::ops::Deref,
    B::Target: AsRef<[u8]>,
{
    let mut file = std::fs::File::create(output).map_err(|source| RenderError::Io {
        path: output.to_path_buf(),
        source,
    })?;
    let mut n = 0u64;
    for frame in frames {
        let buf = frame?;
        file.write_all(frame_bytes(&buf))
            .map_err(|source| RenderError::Io {
                path: output.to_path_buf(),
                source,
            })?;
        n += 1;
    }
    if n == 0 {
        return Err(RenderError::EmptyOutput);
    }
    Ok(n)
}

fn write_ffmpeg_av1<I, B>(
    settings: &EncodeSettings,
    width: u32,
    height: u32,
    fps: u32,
    output: &Path,
    frames: I,
) -> Result<u64, RenderError>
where
    I: Iterator<Item = Result<B, RenderError>>,
    B: std::ops::Deref,
    B::Target: AsRef<[u8]>,
{
    let encoder = match &settings.encoder {
        Some(name) => name.clone(),
        None => detect_av1_encoder(settings.prefer_hw)?,
    };
    tracing::info!(
        encoder = %encoder,
        hw = is_hardware_encoder(&encoder),
        "encode backend"
    );

    let size = format!("{width}x{height}");
    let fps_s = fps.to_string();
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "bgra".into(),
        "-s".into(),
        size,
        "-r".into(),
        fps_s,
        "-i".into(),
        "-".into(),
        "-an".into(),
        "-c:v".into(),
        encoder.clone(),
    ];
    push_quality_args(&mut args, &encoder, settings);
    args.push("-pix_fmt".into());
    args.push("yuv420p".into());
    args.push(output.display().to_string());

    let mut child = Command::new("ffmpeg")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| RenderError::Ffmpeg(e.to_string()))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| RenderError::Ffmpeg("stdin missing".into()))?;

    let mut n = 0u64;
    for frame in frames {
        let buf = frame?;
        if let Err(e) = stdin.write_all(frame_bytes(&buf)) {
            let _ = e;
            break;
        }
        n += 1;
    }
    drop(stdin);

    let out = child
        .wait_with_output()
        .map_err(|e| RenderError::Ffmpeg(e.to_string()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(RenderError::Ffmpeg(format!(
            "exit {:?} encoder={encoder}: {err}",
            out.status.code()
        )));
    }
    if n == 0 {
        return Err(RenderError::EmptyOutput);
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_nonempty() {
        assert!(!AV1_CANDIDATES.is_empty());
    }
}
