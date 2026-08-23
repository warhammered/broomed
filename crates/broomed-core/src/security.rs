use std::path::{Component, Path, PathBuf};

use crate::error::CoreError;

/// Validate `path` is contained within `base`.
///
/// Rejects `..` traversal, absolute paths escaping base, and symlink escape.
pub fn validate_path(path: &Path, base: &Path) -> Result<PathBuf, CoreError> {
    if path.as_os_str().is_empty() {
        return Err(CoreError::InvalidPath("empty path".into()));
    }
    // reject any `..` component (lexical traversal)
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(CoreError::InvalidPath(format!(
            "path contains '..': {}",
            path.display()
        )));
    }
    // NUL check (defensive)
    if path.as_os_str().to_string_lossy().contains('\0') {
        return Err(CoreError::InvalidPath("path contains NUL".into()));
    }

    let base_canon = base.canonicalize().map_err(|e| {
        CoreError::InvalidPath(format!(
            "base not found/canonicalize failed {}: {e}",
            base.display()
        ))
    })?;

    // lexically join
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_canon.join(path)
    };

    // lexical normalization: strip `.` components
    let normalized = normalize_lexically(&candidate);

    if !normalized.starts_with(&base_canon) {
        return Err(CoreError::InvalidPath(format!(
            "path escapes base: {} not within {}",
            path.display(),
            base.display()
        )));
    }

    // symlink check: walk each prefix under base and reject symlinks
    // also detect symlink escape via canonicalization
    let rel = normalized
        .strip_prefix(&base_canon)
        .unwrap_or(Path::new(""));
    let mut accum = base_canon.clone();
    for comp in rel.components() {
        if let Component::Normal(os) = comp {
            accum = accum.join(os);
            if let Ok(meta) = std::fs::symlink_metadata(&accum) {
                if meta.file_type().is_symlink() {
                    return Err(CoreError::Conflict(format!(
                        "symlink rejected: {}",
                        accum.display()
                    )));
                }
            }
        }
    }

    // if candidate (or its parent) exists, canonicalize and verify still inside base
    if normalized.exists() {
        if let Ok(canon) = normalized.canonicalize() {
            if !canon.starts_with(&base_canon) {
                return Err(CoreError::Conflict(format!(
                    "symlink escape: {} resolves outside base",
                    path.display()
                )));
            }
            return Ok(canon);
        }
    } else if let Some(parent) = normalized.parent() {
        if parent.exists() {
            if let Ok(parent_canon) = parent.canonicalize() {
                if !parent_canon.starts_with(&base_canon) {
                    return Err(CoreError::Conflict(format!(
                        "symlink escape via parent: {}",
                        path.display()
                    )));
                }
            }
        }
    }

    Ok(normalized)
}

fn normalize_lexically(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                // already rejected earlier, but handle defensively
                out.pop();
            }
            Component::Normal(s) => out.push(s),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_parent_traversal() {
        let base = std::env::temp_dir();
        let err = validate_path(Path::new("../etc"), &base).unwrap_err();
        assert!(matches!(err, CoreError::InvalidPath(_)));
    }

    #[test]
    fn rejects_absolute_escape() {
        let base = std::env::temp_dir().canonicalize().unwrap();
        // absolute path definitely outside base (root on unix, use /etc)
        let outside = if cfg!(windows) {
            PathBuf::from("C:\\Windows\\System32")
        } else {
            PathBuf::from("/etc/passwd")
        };
        // if temp_dir is /tmp, /etc is outside; otherwise pick a guaranteed outside
        if !outside.starts_with(&base) {
            let err = validate_path(&outside, &base).unwrap_err();
            assert!(matches!(
                err,
                CoreError::InvalidPath(_) | CoreError::Conflict(_)
            ));
        }
    }

    #[test]
    fn allows_subdir_file() {
        let base = std::env::temp_dir().canonicalize().unwrap();
        // create a temp subdir under base to ensure canonicalization works
        let sub = base.join(format!("broomed_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&sub);
        let ok = validate_path(Path::new("subdir/file.txt"), &sub).unwrap();
        assert!(ok.starts_with(&sub));
        let _ = fs::remove_dir_all(&sub);
    }

    #[test]
    fn allows_absolute_inside_base() {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let sub = base.join(format!("broomed_abs_{}", std::process::id()));
        let _ = fs::create_dir_all(&sub);
        let inside = sub.join("inner.txt");
        // file need not exist; lexical check should pass
        let ok = validate_path(&inside, &sub).unwrap();
        assert!(ok.starts_with(&sub));
        let _ = fs::remove_dir_all(&sub);
    }
}
