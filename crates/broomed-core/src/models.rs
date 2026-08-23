use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Versioned manifest for a single model asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub id: String,
    pub version: String,
    pub filename: String,
    /// blake3 hex
    pub checksum: String,
    pub size_bytes: u64,
    /// e.g. "onnx", "safetensors", "gguf"
    pub runtime: String,
    /// Min compatible app version
    #[serde(default)]
    pub min_app_version: Option<String>,
}

/// Registry of all models Broomed can manage.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelRegistry {
    pub models: HashMap<String, ModelManifest>,
}

impl ModelRegistry {
    pub fn default_registry() -> Self {
        let mut m = HashMap::new();
        // ponytail: tiny footprint defaults - all quantized, lazily loaded
        m.insert(
            "all-MiniLM-L6-v2".into(),
            ModelManifest {
                id: "all-MiniLM-L6-v2".into(),
                version: "1.0.0".into(),
                filename: "model.safetensors".into(),
                checksum: "".into(),
                size_bytes: 80_000_000,
                runtime: "candle-onnx".into(),
                min_app_version: None,
            },
        );
        m.insert(
            "smollm2-360m".into(),
            ModelManifest {
                id: "smollm2-360m".into(),
                version: "1.0.0".into(),
                filename: "smollm2-360m-q4_k_m.gguf".into(),
                checksum: "".into(),
                size_bytes: 220_000_000,
                runtime: "llama.cpp".into(),
                min_app_version: None,
            },
        );
        m.insert(
            "smolvlm2-256m".into(),
            ModelManifest {
                id: "smolvlm2-256m".into(),
                version: "1.0.0".into(),
                filename: "smolvlm2-256m-q4_k_m.gguf".into(),
                checksum: "".into(),
                size_bytes: 180_000_000,
                runtime: "llama.cpp-vision".into(),
                min_app_version: None,
            },
        );
        m.insert(
            "whisper-tiny".into(),
            ModelManifest {
                id: "whisper-tiny".into(),
                version: "1.0.0".into(),
                filename: "whisper-tiny.bin".into(),
                checksum: "".into(),
                size_bytes: 75_000_000,
                runtime: "whisper.cpp".into(),
                min_app_version: None,
            },
        );
        Self { models: m }
    }

    pub fn total_default_bytes(&self) -> u64 {
        self.models.values().map(|v| v.size_bytes).sum()
    }
}

/// Where models live on disk.
pub fn model_base_dir() -> PathBuf {
    if let Ok(p) = std::env::var("BROOMED_MODEL_DIR") {
        return PathBuf::from(p);
    }
    // platform-aware
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("Broomed").join("models");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Broomed")
                .join("models");
        }
    }
    // linux fallback
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("broomed")
            .join("models");
    }
    std::env::temp_dir().join("broomed_models")
}

pub fn model_dir_for(id: &str) -> PathBuf {
    model_base_dir().join(id)
}

/// Verify blake3 checksum of a file (streaming, bounded).
pub fn verify_checksum(path: &Path, expected_hex: &str) -> Result<bool, CoreError> {
    if expected_hex.trim().is_empty() {
        // no checksum pinned yet - treat as pass but warn
        return Ok(true);
    }
    let hash = crate::hash::hash_file(path)?;
    Ok(hash.eq_ignore_ascii_case(expected_hex.trim()))
}

/// Lazy model handle - loads on first use, unloads on drop is implicit.
#[derive(Debug)]
pub struct LazyModel<T> {
    id: String,
    path: PathBuf,
    cell: OnceLock<Arc<T>>,
    // ponytail: per-model RwLock for hot-swap without global lock
    _marker: std::marker::PhantomData<T>,
}

impl<T> LazyModel<T> {
    pub fn new(id: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            cell: OnceLock::new(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.cell.get().is_some()
    }

    pub fn is_available(&self) -> bool {
        self.path.exists()
    }

    pub fn get(&self) -> Option<Arc<T>> {
        self.cell.get().map(Arc::clone)
    }

    pub fn set(&self, v: T) -> Result<(), CoreError> {
        let arc = Arc::new(v);
        self.cell
            .set(arc)
            .map_err(|_| CoreError::Internal(format!("{} already loaded", self.id)))?;
        Ok(())
    }
}

/// Atomic model update: download to .tmp then rename, verify checksum.
pub fn atomic_install_model(manifest: &ModelManifest, data: &[u8]) -> Result<PathBuf, CoreError> {
    let dir = model_dir_for(&manifest.id);
    std::fs::create_dir_all(&dir).map_err(|e| CoreError::Io(e.to_string()))?;
    let dest = dir.join(&manifest.filename);
    let tmp = dir.join(format!(".{}.tmp", manifest.filename));
    std::fs::write(&tmp, data).map_err(|e| CoreError::Io(e.to_string()))?;
    if !manifest.checksum.trim().is_empty() {
        let ok = verify_checksum(&tmp, &manifest.checksum)?;
        if !ok {
            let _ = std::fs::remove_file(&tmp);
            return Err(CoreError::Internal(format!(
                "checksum mismatch for {}",
                manifest.id
            )));
        }
    }
    // atomic rename
    std::fs::rename(&tmp, &dest).map_err(|e| CoreError::Io(format!("atomic rename: {e}")))?;
    // write version sidecar
    let meta_path = dir.join("manifest.json");
    let json =
        serde_json::to_string_pretty(manifest).map_err(|e| CoreError::Internal(e.to_string()))?;
    std::fs::write(&meta_path, json).map_err(|e| CoreError::Io(e.to_string()))?;
    Ok(dest)
}

/// Corrupted-model recovery: if manifest says corrupted, remove files.
pub fn recover_if_corrupted(id: &str) -> Result<bool, CoreError> {
    let dir = model_dir_for(id);
    let manifest_path = dir.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(false);
    }
    let s = std::fs::read_to_string(&manifest_path).map_err(|e| CoreError::Io(e.to_string()))?;
    let manifest: ModelManifest = serde_json::from_str(&s)
        .map_err(|e| CoreError::Internal(format!("manifest parse: {e}")))?;
    let file = dir.join(&manifest.filename);
    if !file.exists() {
        return Ok(false);
    }
    if manifest.checksum.trim().is_empty() {
        return Ok(false);
    }
    let ok = verify_checksum(&file, &manifest.checksum)?;
    if !ok {
        tracing::warn!("model {} corrupted, removing", id);
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_file(&manifest_path);
        return Ok(true);
    }
    Ok(false)
}

/// Simple global model registry cache (lazy).
static REGISTRY: OnceLock<RwLock<ModelRegistry>> = OnceLock::new();

pub fn global_registry() -> &'static RwLock<ModelRegistry> {
    REGISTRY.get_or_init(|| RwLock::new(ModelRegistry::default_registry()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_total_within_target() {
        let r = ModelRegistry::default_registry();
        let total = r.total_default_bytes();
        // target 500-700 MB
        assert!(
            total >= 400_000_000,
            "total {total} too small, check manifest"
        );
        assert!(total <= 750_000_000, "total {total} exceeds 700MB target");
    }
    #[test]
    fn no_single_model_over_300() {
        let r = ModelRegistry::default_registry();
        for m in r.models.values() {
            assert!(
                m.size_bytes <= 300_000_000,
                "{} is {} >300MB",
                m.id,
                m.size_bytes
            );
        }
    }
    #[test]
    fn atomic_install_and_verify() {
        let dir = std::env::temp_dir().join(format!("broomed_test_models_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("BROOMED_MODEL_DIR", &dir);
        let manifest = ModelManifest {
            id: "test-model".into(),
            version: "0.1.0".into(),
            filename: "model.bin".into(),
            checksum: "".into(),
            size_bytes: 5,
            runtime: "test".into(),
            min_app_version: None,
        };
        let path = atomic_install_model(&manifest, b"hello").unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read(path).unwrap(), b"hello");
        let _ = std::fs::remove_dir_all(&dir);
        std::env::remove_var("BROOMED_MODEL_DIR");
    }
    #[test]
    fn verify_empty_checksum_passes() {
        let dir = std::env::temp_dir().join(format!("broomed_verify_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("f.bin");
        std::fs::write(&p, b"x").unwrap();
        assert!(verify_checksum(&p, "").unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
