// ponytail: pyo3 binding when Python needs native speed — this module is pure Rust callable for now
use std::path::Path;

use crate::error::CoreError;
use crate::fs::{SafeWalk, TraversalBudget};
use crate::hash::hash_file;
use crate::intent::parse_intent;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn scan_directory_py(base: &str, max_files: usize) -> Result<Vec<String>, CoreError> {
    let walker = SafeWalk::new(base).with_budget(TraversalBudget {
        max_files,
        ..Default::default()
    });
    let paths = walker.walk()?;
    Ok(paths.into_iter().map(|p| p.display().to_string()).collect())
}

pub fn hash_file_py(path: &str) -> Result<String, CoreError> {
    hash_file(Path::new(path))
}

pub fn parse_intent_py(text: &str) -> String {
    format!("{:?}", parse_intent(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_non_empty() {
        assert!(!version().is_empty());
    }

    #[test]
    fn scan_directory_py_ok() {
        let base = env!("CARGO_MANIFEST_DIR").to_string();
        let files = scan_directory_py(&base, 10_000).unwrap();
        assert!(!files.is_empty());
    }

    #[test]
    fn scan_directory_py_invalid_base() {
        let err = scan_directory_py("/no/such/dir/__broomed_test__", 10).unwrap_err();
        assert!(matches!(err, CoreError::InvalidPath(_)));
    }

    #[test]
    fn hash_file_py_roundtrip() {
        let dir = std::env::temp_dir().join(format!("broomed_bridge_hash_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("hello.txt");
        std::fs::write(&path, b"bridge test").unwrap();
        let h = hash_file_py(path.to_str().unwrap()).unwrap();
        assert_eq!(h.len(), 64);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_intent_py_debug_string() {
        let s = parse_intent_py("organize my downloads");
        assert!(s.contains("Organize"));
        let s2 = parse_intent_py("find duplicates");
        assert!(s2.contains("FindDuplicates"));
    }
}
