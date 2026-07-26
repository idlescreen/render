use crate::error::RenderError;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Preferred ffmpeg AV1 encoder names, in order.
pub const AV1_CANDIDATES: &[&str] = &["libsvtav1", "libaom-av1", "librav1e"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeBackend {
    /// Pipe BGRA frames to ffmpeg AV1.
    FfmpegAv1,
    /// Write a single raw BGRA dump (tests / no ffmpeg).
    RawDump,
}

/// Quality / speed knobs passed to ffmpeg.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeSettings {
    /// Constant rate factor (lower = larger/better). Default 35.
    pub crf: u8,
    /// Encoder preset (e.g. SVT-AV1 `6`–`12`, higher is faster).
    pub preset: Option<String>,
    /// Force a specific ffmpeg encoder name (must be installed).
    pub encoder: Option<String>,
}

impl Default for EncodeSettings {
    fn default() -> Self {
        Self {
            crf: 35,
            preset: None,
            encoder: None,
        }
    }
}

/// Pick first available AV1 encoder from `ffmpeg -encoders`.
pub fn detect_av1_encoder() -> Result<String, RenderError> {
    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .map_err(|_| RenderError::FfmpegMissing)?;
    if !out.status.success() {
        return Err(RenderError::FfmpegMissing);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for name in AV1_CANDIDATES {
        if text.contains(name) {
            return Ok((*name).to_string());
        }
    }
    Err(RenderError::Ffmpeg(
        "no AV1 encoder (libsvtav1/libaom-av1/librav1e) found".into(),
    ))
}

fn frame_bytes<B>(buf: &B) -> &[u8]
where
    B: std::ops::Deref,
    B::Target: AsRef<[u8]>,
{
    (*buf).as_ref()
}

/// Encode a sequence of BGRA frames by spawning ffmpeg (or raw dump).
///
/// Accepts `Vec<u8>` or `Arc<Vec<u8>>` (and similar) without an extra full-frame copy.
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
        EncodeBackend::FfmpegAv1 => {
            write_ffmpeg_av1(settings, width, height, fps, output, frames)
        }
    }
}

fn write_raw_dump<I, B>(
    output: &Path,
    frames: I,
) -> Result<u64, RenderError>
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
        None => detect_av1_encoder()?,
    };
    let size = format!("{width}x{height}");
    let crf = settings.crf.to_string();
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
        encoder,
        "-crf".into(),
        crf,
        "-pix_fmt".into(),
        "yuv420p".into(),
    ];
    if let Some(preset) = &settings.preset {
        args.push("-preset".into());
        args.push(preset.clone());
    }
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
            // Broken pipe if ffmpeg exited early — collect status below.
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
            "exit {:?}: {err}",
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

    #[test]
    fn default_crf() {
        assert_eq!(EncodeSettings::default().crf, 35);
    }
}
