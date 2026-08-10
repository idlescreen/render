use crate::encode_select::{
    detect_av1_encoder, detect_h264_encoder, is_hardware_encoder, push_h264_quality_args,
    push_quality_args,
};
use crate::error::RenderError;
use crate::png_writer::encode_bgra_frame_to_png;
use std::io::Write;
use std::path::{Path, PathBuf};
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
    /// Pipe BGRA frames to ffmpeg H.264.
    FfmpegH264,
    /// Write a single raw BGRA dump (tests / no ffmpeg).
    RawDump,
    /// Stream raw BGRA + 16-byte header to stdout.
    StdoutRaw,
    /// Write one PNG per frame into the output directory.
    PngSequence,
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
        EncodeBackend::StdoutRaw => write_stdout_raw(width, height, fps, frames),
        EncodeBackend::PngSequence => write_png_sequence(output, width, height, frames),
        EncodeBackend::FfmpegAv1 => {
            let encoder = match &settings.encoder {
                Some(name) => name.clone(),
                None => detect_av1_encoder(settings.prefer_hw)?,
            };
            write_ffmpeg_pipe(settings, &encoder, width, height, fps, output, frames, true)
        }
        EncodeBackend::FfmpegH264 => {
            let encoder = match &settings.encoder {
                Some(name) => name.clone(),
                None => detect_h264_encoder(settings.prefer_hw)?,
            };
            write_ffmpeg_pipe(settings, &encoder, width, height, fps, output, frames, false)
        }
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

/// Stream raw BGRA + 16-byte header to stdout. Header: width|height|fps|magic LE u32s;
/// magic `0x4942_5247` == "GBRI".
fn write_stdout_raw<I, B>(
    width: u32,
    height: u32,
    fps: u32,
    frames: I,
) -> Result<u64, RenderError>
where
    I: Iterator<Item = Result<B, RenderError>>,
    B: std::ops::Deref,
    B::Target: AsRef<[u8]>,
{
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut header = [0u8; 16];
    header[0..4].copy_from_slice(&width.to_le_bytes());
    header[4..8].copy_from_slice(&height.to_le_bytes());
    header[8..12].copy_from_slice(&fps.to_le_bytes());
    header[12..16].copy_from_slice(&0x4952_4247u32.to_le_bytes());
    let p = PathBuf::from("<stdout>");
    out.write_all(&header).map_err(|source| RenderError::Io {
        path: p.clone(),
        source,
    })?;
    let mut n = 0u64;
    for frame in frames {
        let buf = frame?;
        out.write_all(frame_bytes(&buf)).map_err(|source| RenderError::Io {
            path: p.clone(),
            source,
        })?;
        n += 1;
    }
    if n == 0 {
        return Err(RenderError::EmptyOutput);
    }
    Ok(n)
}

/// Write one PNG per frame into `output_dir`, naming `frame0001.png` etc.
fn write_png_sequence<I, B>(
    output_dir: &Path,
    width: u32,
    height: u32,
    frames: I,
) -> Result<u64, RenderError>
where
    I: Iterator<Item = Result<B, RenderError>>,
    B: std::ops::Deref,
    B::Target: AsRef<[u8]>,
{
    std::fs::create_dir_all(output_dir).map_err(|source| RenderError::Io {
        path: output_dir.to_path_buf(),
        source,
    })?;
    let mut n = 0u64;
    for frame in frames {
        let buf = frame?;
        let png_bytes = encode_bgra_frame_to_png(width, height, frame_bytes(&buf));
        let path = output_dir.join(format!("frame{:06}.png", n + 1));
        std::fs::write(&path, &png_bytes).map_err(|source| RenderError::Io {
            path: path.clone(),
            source,
        })?;
        n += 1;
    }
    if n == 0 {
        return Err(RenderError::EmptyOutput);
    }
    Ok(n)
}

#[allow(clippy::too_many_arguments)]
fn write_ffmpeg_pipe<I, B>(
    settings: &EncodeSettings,
    encoder: &str,
    width: u32,
    height: u32,
    fps: u32,
    output: &Path,
    frames: I,
    av1: bool,
) -> Result<u64, RenderError>
where
    I: Iterator<Item = Result<B, RenderError>>,
    B: std::ops::Deref,
    B::Target: AsRef<[u8]>,
{
    tracing::info!(encoder, hw = is_hardware_encoder(encoder), codec = if av1 {"av1"}else{"h264"}, "encode backend");
    let mut args: Vec<String> = [
        "-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo",
        "-pix_fmt", "bgra", "-s", &format!("{width}x{height}"),
        "-r", &fps.to_string(), "-i", "-", "-an", "-c:v", encoder,
    ].iter().map(|s| s.to_string()).collect();
    if av1 { push_quality_args(&mut args, encoder, settings); }
    else { push_h264_quality_args(&mut args, encoder, settings); }
    args.push("-pix_fmt".into()); args.push("yuv420p".into());
    args.push(output.display().to_string());

    let mut child = Command::new("ffmpeg")
        .args(&args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped())
        .spawn().map_err(|e| RenderError::Ffmpeg(e.to_string()))?;
    let mut stdin = child.stdin.take().ok_or_else(|| RenderError::Ffmpeg("stdin missing".into()))?;
    let mut n = 0u64;
    for frame in frames {
        if stdin.write_all(frame_bytes(&frame?)).is_err() { break; }
        n += 1;
    }
    drop(stdin);
    let out = child.wait_with_output().map_err(|e| RenderError::Ffmpeg(e.to_string()))?;
    if !out.status.success() {
        return Err(RenderError::Ffmpeg(format!("exit {:?} encoder={encoder}: {}",
            out.status.code(), String::from_utf8_lossy(&out.stderr))));
    }
    if n == 0 { return Err(RenderError::EmptyOutput); }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_nonempty() {
        assert!(!AV1_CANDIDATES.is_empty());
    }

    #[test]
    fn stdout_raw_header_magic_is_gbri() {
        let bytes = 0x4952_4247u32.to_le_bytes();
        assert_eq!(&bytes, b"GBRI");
    }
}

