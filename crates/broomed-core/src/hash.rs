use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::CoreError;

/// Streaming blake3 hash, 64 KB chunks, hex string.
pub fn hash_file(path: &Path) -> Result<String, CoreError> {
    let mut file = File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CoreError::NotFound(format!("{}: {e}", path.display()))
        } else {
            CoreError::Io(e.to_string())
        }
    })?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| CoreError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn hash_deterministic() {
        let dir = std::env::temp_dir().join(format!("broomed_hash_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test.txt");
        fs::write(&path, b"hello world").unwrap();
        let h1 = hash_file(&path).unwrap();
        let h2 = hash_file(&path).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
        // different content -> different hash
        fs::write(&path, b"hello world!").unwrap();
        let h3 = hash_file(&path).unwrap();
        assert_ne!(h1, h3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hash_empty_file() {
        let dir = std::env::temp_dir().join(format!("broomed_hash_empty_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("empty.bin");
        fs::write(&path, b"").unwrap();
        let h = hash_file(&path).unwrap();
        // blake3 of empty
        assert_eq!(h, blake3::hash(b"").to_hex().to_string());
        let _ = fs::remove_dir_all(&dir);
    }
}
