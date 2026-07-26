//! Path safety for plugin/audio/output arguments.

use crate::error::RenderError;
use std::path::{Component, Path};

/// Reject paths that contain `..` components (path traversal).
pub fn deny_parent_dirs(path: &Path, label: &str) -> Result<(), RenderError> {
    for c in path.components() {
        if matches!(c, Component::ParentDir) {
            return Err(RenderError::Job(format!(
                "{label} must not contain '..' path components"
            )));
        }
    }
    Ok(())
}

/// Create parent directories for `path` when present (no-op for bare filenames).
pub fn ensure_parent_dir(path: &Path) -> Result<(), RenderError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() || parent.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(parent).map_err(|source| RenderError::Io {
        path: parent.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn accepts_plain() {
        assert!(deny_parent_dirs(Path::new("/tmp/out.mkv"), "output").is_ok());
    }

    #[test]
    fn rejects_dotdot() {
        assert!(deny_parent_dirs(Path::new("../evil.so"), "plugin").is_err());
        assert!(deny_parent_dirs(Path::new("/tmp/../etc/passwd"), "audio").is_err());
    }

    #[test]
    fn ensure_parent_bare_ok() {
        assert!(ensure_parent_dir(Path::new("out.mkv")).is_ok());
    }
}
