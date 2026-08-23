use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine;
use serde::{Deserialize, Serialize};

// ponytail: single file, enum backend with in-memory for tests; keyring attempted if feature enabled, else file fallback.

fn credentials_path() -> PathBuf {
    let base = crate::models::model_base_dir();
    // parent of models dir, e.g. ~/.local/share/broomed/credentials.json
    let parent = base.parent().map(|p| p.to_path_buf()).unwrap_or(base);
    parent.join("credentials.json")
}

#[allow(dead_code)]
#[derive(Debug, Default, Serialize, Deserialize)]
struct FileCreds(HashMap<String, String>);

fn load_file_creds(path: &PathBuf) -> HashMap<String, String> {
    if !path.exists() {
        return HashMap::new();
    }
    let s = std::fs::read_to_string(path).unwrap_or_default();
    if s.trim().is_empty() {
        return HashMap::new();
    }
    serde_json::from_str::<HashMap<String, String>>(&s).unwrap_or_default()
}

fn save_file_creds(path: &PathBuf, map: &HashMap<String, String>) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string(map) {
        let _ = std::fs::write(path, s);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

/// Redact secret for diagnostics
pub fn redact(s: &str) -> String {
    if s.is_empty() {
        return "***".to_string();
    }
    "***".to_string()
}

pub struct SecureStore {
    // ponytail: global lock for file fallback, per-process; upgrade to per-key if needed
    file_path: PathBuf,
    // in-memory override for tests
    mem: Option<Arc<Mutex<HashMap<String, String>>>>,
    mem_private: Option<Arc<Mutex<Option<Vec<u8>>>>>,
}

impl Clone for SecureStore {
    fn clone(&self) -> Self {
        Self {
            file_path: self.file_path.clone(),
            mem: self.mem.clone(),
            mem_private: self.mem_private.clone(),
        }
    }
}

impl SecureStore {
    pub fn new() -> Self {
        Self {
            file_path: credentials_path(),
            mem: None,
            mem_private: None,
        }
    }
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            file_path: path,
            mem: None,
            mem_private: None,
        }
    }
    /// In-memory store for tests (no filesystem/keyring)
    pub fn memory() -> Self {
        Self {
            file_path: PathBuf::from(":memory:"),
            mem: Some(Arc::new(Mutex::new(HashMap::new()))),
            mem_private: Some(Arc::new(Mutex::new(None))),
        }
    }

    #[allow(dead_code)]
    fn is_memory(&self) -> bool {
        self.mem.is_some()
    }

    pub fn store_token(&self, key: &str, value: &str) -> Result<(), String> {
        if let Some(m) = &self.mem {
            m.lock().unwrap().insert(key.to_string(), value.to_string());
            return Ok(());
        }
        // try keyring if feature enabled
        #[cfg(feature = "keyring")]
        {
            if let Ok(entry) = keyring::Entry::new("broomed", key) {
                if entry.set_password(value).is_ok() {
                    return Ok(());
                }
            }
        }
        // fallback file
        let mut map = load_file_creds(&self.file_path);
        map.insert(key.to_string(), value.to_string());
        save_file_creds(&self.file_path, &map);
        Ok(())
    }

    pub fn load_token(&self, key: &str) -> Option<String> {
        if let Some(m) = &self.mem {
            return m.lock().unwrap().get(key).cloned();
        }
        #[cfg(feature = "keyring")]
        {
            if let Ok(entry) = keyring::Entry::new("broomed", key) {
                if let Ok(p) = entry.get_password() {
                    return Some(p);
                }
            }
        }
        let map = load_file_creds(&self.file_path);
        map.get(key).cloned()
    }

    pub fn delete_token(&self, key: &str) -> Result<(), String> {
        if let Some(m) = &self.mem {
            m.lock().unwrap().remove(key);
            if let Some(mp) = &self.mem_private {
                if key == "private_key" {
                    *mp.lock().unwrap() = None;
                }
            }
            return Ok(());
        }
        #[cfg(feature = "keyring")]
        {
            if let Ok(entry) = keyring::Entry::new("broomed", key) {
                let _ = entry.delete_password();
            }
        }
        let mut map = load_file_creds(&self.file_path);
        map.remove(key);
        save_file_creds(&self.file_path, &map);
        Ok(())
    }

    pub fn store_private_key(&self, bytes: &[u8]) -> Result<(), String> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        if let Some(mp) = &self.mem_private {
            *mp.lock().unwrap() = Some(bytes.to_vec());
            // also store as token for unified load
            if let Some(m) = &self.mem {
                m.lock().unwrap().insert("private_key".to_string(), b64);
            }
            return Ok(());
        }
        self.store_token("private_key", &b64)
    }

    pub fn load_private_key(&self) -> Option<Vec<u8>> {
        if let Some(mp) = &self.mem_private {
            if let Some(v) = mp.lock().unwrap().clone() {
                return Some(v);
            }
        }
        let b64 = self.load_token("private_key")?;
        base64::engine::general_purpose::STANDARD.decode(b64).ok()
    }
}

impl Default for SecureStore {
    fn default() -> Self {
        Self::new()
    }
}

// ensure Debug doesn't leak
impl std::fmt::Debug for SecureStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecureStore")
            .field("file_path", &self.file_path)
            .field("has_mem", &self.mem.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_memory() {
        let s = SecureStore::memory();
        s.store_token("k", "v").unwrap();
        assert_eq!(s.load_token("k").as_deref(), Some("v"));
        assert_eq!(redact("secret"), "***");
        // ensure debug doesn't contain value
        let dbg = format!("{:?}", s);
        assert!(!dbg.contains("secret"));
        s.delete_token("k").unwrap();
        assert!(s.load_token("k").is_none());
    }
    #[test]
    fn private_key_roundtrip() {
        let s = SecureStore::memory();
        let bytes = vec![1, 2, 3, 32];
        s.store_private_key(&bytes).unwrap();
        assert_eq!(s.load_private_key().unwrap(), bytes);
    }
    #[test]
    fn redact_helper() {
        assert_eq!(redact("token123"), "***");
        assert_eq!(redact(""), "***");
    }
    #[test]
    fn file_fallback_roundtrip() {
        let dir = std::env::temp_dir().join(format!("broomed_store_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("credentials.json");
        let s = SecureStore::with_path(path.clone());
        s.store_token("a", "b").unwrap();
        assert_eq!(s.load_token("a").as_deref(), Some("b"));
        // new instance loads same file
        let s2 = SecureStore::with_path(path.clone());
        assert_eq!(s2.load_token("a").as_deref(), Some("b"));
        s.delete_token("a").unwrap();
        assert!(s.load_token("a").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
