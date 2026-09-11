// SPDX-License-Identifier: Apache-2.0

//! Encoded output sinks: PNG sequence writer and ffmpeg pipe.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::encode_select::{
    is_hardware_encoder, push_h264_quality_args, push_quality_args, EncodeSettings,
};
use crate::error::RenderError;
use crate::png_writer::encode_bgra_frame_to_png;

use super::frame_bytes;

pub(super) fn write_png_sequence<I, B>(
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
pub(super) fn write_ffmpeg_pipe<I, B>(
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
    tracing::info!(
        encoder,
        hw = is_hardware_encoder(encoder),
        codec = if av1 { "av1" } else { "h264" },
        "encode backend"
    );
    let mut args: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "bgra",
        "-s",
        &format!("{width}x{height}"),
        "-r",
        &fps.to_string(),
        "-i",
        "-",
        "-an",
        "-c:v",
        encoder,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if av1 {
        push_quality_args(&mut args, encoder, settings);
    } else {
        push_h264_quality_args(&mut args, encoder, settings);
    }
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
        if stdin.write_all(frame_bytes(&frame?)).is_err() {
            break;
        }
        n += 1;
    }
    drop(stdin);
    let out = child
        .wait_with_output()
        .map_err(|e| RenderError::Ffmpeg(e.to_string()))?;
    if !out.status.success() {
        return Err(RenderError::Ffmpeg(format!(
            "exit {:?} encoder={encoder}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    if n == 0 {
        return Err(RenderError::EmptyOutput);
    }
    Ok(n)
}
