//! Pure Rust high-level bridge wrappers for directory scanning, hashing, and intent parsing.

use std::path::Path;

use crate::error::CoreError;
use crate::fs::{SafeWalk, TraversalBudget};
use crate::hash::hash_file;
use crate::intent::parse_intent;

/// Returns current version of the broomed-core library.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Recursively scans a directory with a safety budget and returns paths as strings.
pub fn scan_directory(base: &str, max_files: usize) -> Result<Vec<String>, CoreError> {
    let walker = SafeWalk::new(base).with_budget(TraversalBudget {
        max_files,
        ..Default::default()
    });
    let paths = walker.walk()?;
    Ok(paths.into_iter().map(|p| p.display().to_string()).collect())
}

/// Hashes a file at the given path string using BLAKE3.
pub fn hash_file_str(path: &str) -> Result<String, CoreError> {
    hash_file(Path::new(path))
}

/// Parses a natural language intent string into its debug string representation.
pub fn parse_intent_str(text: &str) -> String {
    format!("{:?}", parse_intent(text))
}

// ── Backwards-compatible aliases ──────────────────────────────────────────
pub fn scan_directory_py(base: &str, max_files: usize) -> Result<Vec<String>, CoreError> {
    scan_directory(base, max_files)
}

pub fn hash_file_py(path: &str) -> Result<String, CoreError> {
    hash_file_str(path)
}

pub fn parse_intent_py(text: &str) -> String {
    parse_intent_str(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_non_empty() {
        assert!(!version().is_empty());
    }

    #[test]
    fn scan_directory_ok() {
        let base = env!("CARGO_MANIFEST_DIR").to_string();
        let files = scan_directory(&base, 50_000).unwrap();
        assert!(!files.is_empty());
    }

    #[test]
    fn scan_directory_py_ok() {
        let base = env!("CARGO_MANIFEST_DIR").to_string();
        let files = scan_directory_py(&base, 50_000).unwrap();
        assert!(!files.is_empty());
    }

    #[test]
    fn scan_directory_invalid_base() {
        let err = scan_directory("/no/such/dir/__broomed_test__", 10).unwrap_err();
        assert!(matches!(err, CoreError::InvalidPath(_)));
    }

    #[test]
    fn hash_file_str_roundtrip() {
        let dir = std::env::temp_dir().join(format!("broomed_bridge_hash_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("hello.txt");
        std::fs::write(&path, b"bridge test").unwrap();
        let h = hash_file_str(path.to_str().unwrap()).unwrap();
        assert_eq!(h.len(), 64);
        let h2 = hash_file_py(path.to_str().unwrap()).unwrap();
        assert_eq!(h, h2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_intent_str_debug() {
        let s = parse_intent_str("organize my downloads");
        assert!(s.contains("Organize"));
        let s2 = parse_intent_py("find duplicates");
        assert!(s2.contains("FindDuplicates"));
    }
}
