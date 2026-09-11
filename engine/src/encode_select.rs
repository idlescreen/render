//! AV1 + H.264 encoder discovery and quality-flag mapping.
//!
//! Encoder-probe helpers (`probe_encoder`, `detect_av1_encoder`,
//! `detect_h264_encoder`, `push_quality_args`, `push_h264_quality_args`, …)
//! live in [`crate::encoder_probe`]. They are re-exported here so external
//! callers that still reference `crate::encode_select::*` keep working —
//! this file used to hit the project's 256-line cap, so the probe helpers
//! were extracted for headroom.

pub use crate::encoder_probe::{
    detect_av1_encoder, detect_h264_encoder, probe_encoder, probe_quality_args,
    push_h264_quality_args, push_quality_args,
};

/// Software AV1 encoders (CPU), preferred order.
pub const SW_AV1_CANDIDATES: &[&str] = &["libsvtav1", "libaom-av1", "librav1e"];
/// Hardware AV1 encoders that accept CPU-fed frames (skip VAAPI — needs hwupload).
pub const HW_AV1_CANDIDATES: &[&str] = &["av1_nvenc", "av1_qsv", "av1_amf"];

/// H.264 software encoder (always prefer libx264 — universally available).
pub const SW_H264_CANDIDATES: &[&str] = &["libx264"];
/// H.264 hardware encoders (NVENC/QSV/AMF), in preference order.
pub const HW_H264_CANDIDATES: &[&str] = &["h264_nvenc", "h264_qsv", "h264_amf"];

/// Quality / speed knobs passed to ffmpeg.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeSettings {
    /// Quality scale 0–63 (lower = better/larger). Soft CRF / hard CQ / QP.
    pub crf: u8,
    /// Encoder preset string (SVT numeric, NVENC `p1`–`p7`, etc.).
    pub preset: Option<String>,
    /// Force a specific ffmpeg encoder name (must be installed).
    pub encoder: Option<String>,
    /// When auto-detecting, try hardware AV1 before software.
    pub prefer_hw: bool,
}

impl Default for EncodeSettings {
    fn default() -> Self {
        Self {
            crf: 35,
            preset: None,
            encoder: None,
            prefer_hw: true,
        }
    }
}

/// True for NVENC / QSV / AMF style hardware encoders (AV1 or H.264).
pub fn is_hardware_encoder(name: &str) -> bool {
    name.ends_with("_nvenc") || name.ends_with("_qsv") || name.ends_with("_amf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_nonempty() {
        assert!(!SW_AV1_CANDIDATES.is_empty());
        assert!(!HW_AV1_CANDIDATES.is_empty());
    }

    #[test]
    fn default_prefers_hw() {
        assert!(EncodeSettings::default().prefer_hw);
        assert_eq!(EncodeSettings::default().crf, 35);
    }

    #[test]
    fn hardware_classifier() {
        assert!(is_hardware_encoder("av1_nvenc"));
        assert!(is_hardware_encoder("h264_nvenc"));
        assert!(!is_hardware_encoder("libsvtav1"));
        assert!(!is_hardware_encoder("libx264"));
    }
}
