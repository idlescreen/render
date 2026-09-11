//! Encoder-probe helpers extracted from `encode_select.rs`.
//!
//! These functions used to live alongside the main encode-backend dispatch
//! (AV1 vs H.264 vs RawDump selection) in `encode_select.rs`, but that file
//! hit the project's 256-line cap. They were extracted here to give both
//! modules meaningful headroom while preserving every public path: callers
//! still reach the probe functions via `crate::encode_select::*` thanks to
//! the `pub use crate::encoder_probe::*;` re-export in `encode_select.rs`.
//!
//! This module owns:
//! * `probe_encoder` — tiny lavfi encode that filters out listed-but-broken
//!   drivers (e.g. NVENC with no CUDA available).
//! * `probe_quality_args` — quality flags used only by the probe pass.
//! * `detect_av1_encoder` / `detect_h264_encoder` — encoder discovery.
//! * `push_quality_args` / `push_h264_quality_args` — production quality-args
//!   pushers used by the actual encode pipeline.
//! * Probe helpers: `ffmpeg_encoders_text`, `listed_in_ffmpeg`, `first_working`.

use crate::encode_select::{
    EncodeSettings, HW_AV1_CANDIDATES, HW_H264_CANDIDATES, SW_AV1_CANDIDATES, SW_H264_CANDIDATES,
};
use crate::error::RenderError;
use std::process::Command;

fn ffmpeg_encoders_text() -> Result<String, RenderError> {
    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .map_err(|_| RenderError::FfmpegMissing)?;
    if !out.status.success() {
        return Err(RenderError::FfmpegMissing);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn listed_in_ffmpeg(text: &str, name: &str) -> bool {
    text.lines().any(|line| {
        line.split_whitespace()
            .nth(1)
            .is_some_and(|tok| tok == name)
    })
}

/// Tiny lavfi encode to prove the encoder can open (filters out "listed but no CUDA").
pub fn probe_encoder(name: &str) -> bool {
    use std::process::Stdio;
    // 1 black frame → null mux. Must succeed for the encoder to be selectable.
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=64x64:d=0.04",
            "-frames:v",
            "1",
            "-an",
            "-c:v",
            name,
        ])
        .args(probe_quality_args(name))
        .args(["-f", "null", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(status, Ok(s) if s.success())
}

pub fn probe_quality_args(name: &str) -> Vec<&'static str> {
    if name.contains("nvenc") {
        vec!["-rc", "vbr", "-cq", "40", "-preset", "p1"]
    } else if name.contains("qsv") {
        vec!["-global_quality", "40"]
    } else if name.contains("amf") {
        vec!["-rc", "cqp", "-qp_i", "40", "-qp_p", "40"]
    } else {
        vec!["-crf", "40", "-preset", "12"]
    }
}

fn first_working(text: &str, names: &[&str]) -> Option<String> {
    for name in names {
        if listed_in_ffmpeg(text, name) && probe_encoder(name) {
            return Some((*name).to_string());
        }
    }
    None
}

/// Pick an available AV1 encoder (hardware first when `prefer_hw`).
/// Hardware candidates are **probed** so listed-but-broken drivers (no CUDA) are skipped.
pub fn detect_av1_encoder(prefer_hw: bool) -> Result<String, RenderError> {
    let text = ffmpeg_encoders_text()?;
    if prefer_hw {
        if let Some(name) = first_working(&text, HW_AV1_CANDIDATES) {
            return Ok(name);
        }
    }
    if let Some(name) = first_working(&text, SW_AV1_CANDIDATES) {
        return Ok(name);
    }
    if !prefer_hw {
        if let Some(name) = first_working(&text, HW_AV1_CANDIDATES) {
            return Ok(name);
        }
    }
    Err(RenderError::Ffmpeg(
        "no working AV1 encoder (probed nvenc/qsv/amf + libsvtav1/aom/rav1e)".into(),
    ))
}

/// Pick an available H.264 encoder (hardware first when `prefer_hw`).
/// Mirrors [`detect_av1_encoder`] for the H.264 family.
pub fn detect_h264_encoder(prefer_hw: bool) -> Result<String, RenderError> {
    let text = ffmpeg_encoders_text()?;
    if prefer_hw {
        if let Some(name) = first_working(&text, HW_H264_CANDIDATES) {
            return Ok(name);
        }
    }
    if let Some(name) = first_working(&text, SW_H264_CANDIDATES) {
        return Ok(name);
    }
    if !prefer_hw {
        if let Some(name) = first_working(&text, HW_H264_CANDIDATES) {
            return Ok(name);
        }
    }
    Err(RenderError::Ffmpeg("no working H.264 encoder".into()))
}

/// Append encoder-specific quality flags after `-c:v <name>`.
pub fn push_quality_args(args: &mut Vec<String>, encoder: &str, settings: &EncodeSettings) {
    let q = settings.crf.to_string();
    if encoder.contains("nvenc") {
        args.push("-rc".into());
        args.push("vbr".into());
        args.push("-cq".into());
        args.push(q);
        args.push("-preset".into());
        args.push(settings.preset.clone().unwrap_or_else(|| "p4".into()));
    } else if encoder.contains("qsv") {
        args.push("-global_quality".into());
        args.push(q);
        if let Some(preset) = &settings.preset {
            args.push("-preset".into());
            args.push(preset.clone());
        }
    } else if encoder.contains("amf") {
        args.push("-rc".into());
        args.push("cqp".into());
        args.push("-qp_i".into());
        args.push(q.clone());
        args.push("-qp_p".into());
        args.push(q);
        if let Some(preset) = &settings.preset {
            args.push("-quality".into());
            args.push(preset.clone());
        }
    } else {
        args.push("-crf".into());
        args.push(q);
        if let Some(preset) = &settings.preset {
            args.push("-preset".into());
            args.push(preset.clone());
        }
    }
}

/// Append H.264-specific quality flags after `-c:v <name>`. NVENC uses `-rc vbr -cq`;
/// libx264/QSV/AMF use `-crf` (the latter two honor it in current builds).
pub fn push_h264_quality_args(args: &mut Vec<String>, encoder: &str, settings: &EncodeSettings) {
    let q = settings.crf.to_string();
    if encoder.contains("nvenc") {
        args.push("-rc".into());
        args.push("vbr".into());
        args.push("-cq".into());
        args.push(q);
        args.push("-preset".into());
        args.push(settings.preset.clone().unwrap_or_else(|| "p4".into()));
    } else {
        args.push("-crf".into());
        args.push(q);
        if let Some(preset) = &settings.preset {
            args.push("-preset".into());
            args.push(preset.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvenc_quality_uses_cq() {
        let s = EncodeSettings {
            crf: 30,
            preset: Some("p5".into()),
            encoder: None,
            prefer_hw: true,
        };
        let mut args = vec!["-c:v".into(), "av1_nvenc".into()];
        push_quality_args(&mut args, "av1_nvenc", &s);
        assert!(args.iter().any(|a| a == "-cq"));
        assert!(args.iter().any(|a| a == "30"));
        assert!(args.iter().any(|a| a == "p5"));
    }

    #[test]
    fn software_quality_uses_crf() {
        let s = EncodeSettings::default();
        let mut args = Vec::new();
        push_quality_args(&mut args, "libsvtav1", &s);
        assert!(args.iter().any(|a| a == "-crf"));
    }
}
