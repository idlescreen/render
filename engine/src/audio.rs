//! Mux optional audio bed under a finished video master.

use crate::error::RenderError;
use std::path::Path;
use std::process::Command;

/// Mux `audio` under `video`, writing `output` (may equal `video` via temp swap).
/// Audio is looped or cut with `-shortest` so it matches video length.
pub fn mux_audio_bed(video: &Path, audio: &Path, output: &Path) -> Result<(), RenderError> {
    if !audio.is_file() {
        return Err(RenderError::Job(format!(
            "audio file not found: {}",
            audio.display()
        )));
    }
    let tmp = output.with_extension("mux.tmp.mkv");
    let out = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-stream_loop",
            "-1",
            "-i",
        ])
        .arg(audio)
        .arg("-i")
        .arg(video)
        .args([
            "-map",
            "1:v:0",
            "-map",
            "0:a:0",
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-shortest",
        ])
        .arg(&tmp)
        .output()
        .map_err(|e| RenderError::Ffmpeg(e.to_string()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let _ = std::fs::remove_file(&tmp);
        return Err(RenderError::Ffmpeg(format!("audio mux failed: {err}")));
    }
    std::fs::rename(&tmp, output).map_err(|source| RenderError::Io {
        path: output.to_path_buf(),
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn missing_audio_errors() {
        let r = mux_audio_bed(
            Path::new("/tmp/no-video.mkv"),
            Path::new("/tmp/definitely-missing-audio-idle-xyz.wav"),
            Path::new("/tmp/out.mkv"),
        );
        assert!(r.is_err());
        let _ = PathBuf::from(".");
    }
}
