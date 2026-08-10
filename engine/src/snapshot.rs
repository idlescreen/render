//! Snapshot baseline compare for `--snapshot-last-only` mode.
//!
//! Two-arg signature: `<current>` bytes vs `<baseline>` bytes, byte-equal.
//! For PNG inputs the comparison decodes both into RGBA8 buffers (since two
//! encoders can produce different IDAT zlib output for identical pixels, but
//! always decode to the same RGBA8). For raw BGRA inputs we memcmp the bytes
//! directly — that path catches renderer-determinism regressions.

use std::io::Read;
use std::path::Path;

/// Snapshot mismatch reason (carried back through [`compare`] for diagnostics).
#[derive(Debug, thiserror::Error)]
pub enum SnapshotMismatch {
    #[error("baseline missing at {0} (run with --update-baselines to seed)")]
    MissingBaseline(String),
    #[error("baseline length {baseline} != current length {current}")]
    LengthMismatch { baseline: usize, current: usize },
    #[error("pixel mismatch at byte {byte_offset}: baseline {baseline:#04x} vs current {current:#04x}")]
    PixelMismatch {
        byte_offset: usize,
        baseline: u8,
        current: u8,
    },
    #[error("snapshot io error on {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

/// Compare two files. PNG inputs are decoded into RGBA8 (deterministic across
/// zlib settings); raw `.bgra` inputs are memcmp'd verbatim.
pub fn compare(current_path: &Path, baseline_path: &Path) -> Result<(), SnapshotMismatch> {
    if !baseline_path.is_file() {
        return Err(SnapshotMismatch::MissingBaseline(
            baseline_path.display().to_string(),
        ));
    }
    let ext = baseline_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let current = read_bytes(current_path).map_err(|source| SnapshotMismatch::Io {
        path: current_path.display().to_string(),
        source,
    })?;
    let baseline = read_bytes(baseline_path).map_err(|source| SnapshotMismatch::Io {
        path: baseline_path.display().to_string(),
        source,
    })?;
    if ext == "png" {
        let cur_decoded = decode_png_rgba(&current, current_path)?;
        let base_decoded = decode_png_rgba(&baseline, baseline_path)?;
        if cur_decoded.len() != base_decoded.len() {
            return Err(SnapshotMismatch::LengthMismatch {
                baseline: base_decoded.len(),
                current: cur_decoded.len(),
            });
        }
        for (i, (c, b)) in cur_decoded.iter().zip(base_decoded.iter()).enumerate() {
            if c != b {
                return Err(SnapshotMismatch::PixelMismatch {
                    byte_offset: i,
                    baseline: *b,
                    current: *c,
                });
            }
        }
        return Ok(());
    }
    // Raw: memcmp.
    if current.len() != baseline.len() {
        return Err(SnapshotMismatch::LengthMismatch {
            baseline: baseline.len(),
            current: current.len(),
        });
    }
    for (i, (c, b)) in current.iter().zip(baseline.iter()).enumerate() {
        if c != b {
            return Err(SnapshotMismatch::PixelMismatch {
                byte_offset: i,
                baseline: *b,
                current: *c,
            });
        }
    }
    Ok(())
}

/// Write `bytes` to `path`, creating parents as needed.
pub fn write_baseline(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, bytes)
}

/// Read all bytes from `path`.
pub fn read_baseline(path: &Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let mut f = std::fs::File::open(path)?;
    let mut v = Vec::new();
    f.read_to_end(&mut v)?;
    Ok(v)
}

fn decode_png_rgba(bytes: &[u8], path: &Path) -> Result<Vec<u8>, SnapshotMismatch> {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info().map_err(|e| SnapshotMismatch::Io {
        path: path.display().to_string(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
    })?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| SnapshotMismatch::Io {
        path: path.display().to_string(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
    })?;
    buf.truncate(info.buffer_size());
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_equal_passes() {
        let tmp = tempfile::tempdir().expect("tmp");
        let a = tmp.path().join("a.bgra");
        let b = tmp.path().join("b.bgra");
        std::fs::write(&a, &[1u8, 2, 3, 4]).expect("write");
        std::fs::write(&b, &[1u8, 2, 3, 4]).expect("write");
        compare(&a, &b).expect("eq");
    }

    #[test]
    fn byte_unequal_fails() {
        let tmp = tempfile::tempdir().expect("tmp");
        let a = tmp.path().join("a.bgra");
        let b = tmp.path().join("b.bgra");
        std::fs::write(&a, &[1u8, 2, 3, 4]).expect("write");
        std::fs::write(&b, &[1u8, 2, 3, 5]).expect("write");
        let e = compare(&a, &b).expect_err("diff");
        assert!(matches!(e, SnapshotMismatch::PixelMismatch { .. }));
    }

    #[test]
    fn missing_baseline_surfaces() {
        let tmp = tempfile::tempdir().expect("tmp");
        let a = tmp.path().join("a.bgra");
        let b = tmp.path().join("absent.bgra");
        std::fs::write(&a, &[1u8]).expect("write");
        let e = compare(&a, &b).expect_err("missing");
        assert!(matches!(e, SnapshotMismatch::MissingBaseline(_)));
    }

    #[test]
    fn png_compare_decodes_to_rgba() {
        let tmp = tempfile::tempdir().expect("tmp");
        let a = tmp.path().join("a.png");
        let b = tmp.path().join("b.png");
        let bgra = vec![10u8, 20, 30, 255];
        let png_a = crate::png_writer::encode_bgra_frame_to_png(1, 1, &bgra);
        std::fs::write(&a, &png_a).expect("write");
        std::fs::write(&b, &png_a).expect("write");
        compare(&a, &b).expect("png eq");
    }
}
