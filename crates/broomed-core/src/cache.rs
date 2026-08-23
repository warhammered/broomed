use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use rusqlite::OptionalExtension;

use crate::analysis::FileAnalysis;
use crate::error::CoreError;

/// In-memory bounded cache + SQLite persistence helper.
static MEM_CACHE: OnceLock<Mutex<CacheStore>> = OnceLock::new();

fn mem_cache() -> &'static Mutex<CacheStore> {
    MEM_CACHE.get_or_init(|| Mutex::new(CacheStore::default()))
}

#[derive(Default)]
struct CacheStore {
    map: HashMap<String, FileAnalysis>,
    order: Vec<String>, // simple LRU order (oldest front)
    cap: usize,
}

impl CacheStore {
    #[allow(dead_code)]
    fn with_cap(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: Vec::new(),
            cap,
        }
    }
    fn insert(&mut self, key: String, val: FileAnalysis) {
        if self.map.contains_key(&key) {
            self.map.insert(key.clone(), val);
            self.order.retain(|k| k != &key);
            self.order.push(key);
            return;
        }
        if self.map.len() >= self.cap.max(1) {
            if let Some(old) = self.order.first().cloned() {
                self.order.remove(0);
                self.map.remove(&old);
            }
        }
        self.order.push(key.clone());
        self.map.insert(key, val);
    }
    fn get(&self, key: &str) -> Option<FileAnalysis> {
        self.map.get(key).cloned()
    }
}

const DEFAULT_CAP: usize = 2048;

/// Put analysis into memory cache.
pub fn cache_put(content_hash: &str, analysis: &FileAnalysis) {
    let key = analysis.cache_key(content_hash);
    let mut guard = mem_cache().lock().unwrap();
    if guard.cap == 0 {
        guard.cap = DEFAULT_CAP;
    }
    guard.insert(key, analysis.clone());
}

/// Get from memory cache if present and still valid (pipeline version matches).
pub fn cache_get(content_hash: &str, probe: &FileAnalysis) -> Option<FileAnalysis> {
    let key = probe.cache_key(content_hash);
    mem_cache().lock().unwrap().get(&key)
}

/// Persist analysis to SQLite (file_metadata table as JSON + file_embeddings)
pub fn persist_analysis(
    conn: &rusqlite::Connection,
    file_id: &str,
    content_hash: &str,
    analysis: &FileAnalysis,
) -> Result<(), CoreError> {
    let key = analysis.cache_key(content_hash);
    let json = serde_json::to_string(analysis).map_err(|e| CoreError::Internal(e.to_string()))?;
    conn.execute(
        "INSERT OR REPLACE INTO file_metadata (file_id, data) VALUES (?1, ?2)",
        rusqlite::params![file_id, json],
    )?;
    // store cache key in settings for quick invalidation check (optional)
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![format!("cache:{file_id}"), key],
    )?;
    if let Some(emb) = &analysis.embedding {
        // store as blob of f32 LE
        let mut bytes = Vec::with_capacity(emb.len() * 4);
        for v in emb {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let model = analysis
            .model_versions
            .embedding
            .clone()
            .unwrap_or_else(|| "all-MiniLM-L6-v2".to_string());
        conn.execute(
            "INSERT OR REPLACE INTO file_embeddings (file_id, model, vec, version) VALUES (?1, ?2, ?3, 1)",
            rusqlite::params![file_id, model, bytes],
        )?;
    }
    Ok(())
}

pub fn load_analysis(
    conn: &rusqlite::Connection,
    file_id: &str,
) -> Result<Option<FileAnalysis>, CoreError> {
    let mut stmt = conn.prepare("SELECT data FROM file_metadata WHERE file_id = ?1")?;
    let res: Option<String> = stmt
        .query_row(rusqlite::params![file_id], |r| r.get(0))
        .optional()
        .map_err(|e| CoreError::Internal(e.to_string()))?;
    if let Some(json) = res {
        let a: FileAnalysis =
            serde_json::from_str(&json).map_err(|e| CoreError::Internal(e.to_string()))?;
        Ok(Some(a))
    } else {
        Ok(None)
    }
}

pub fn invalidate_if_model_changed(
    conn: &rusqlite::Connection,
    file_id: &str,
    current_key: &str,
) -> Result<bool, CoreError> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let stored: Option<String> = stmt
        .query_row(rusqlite::params![format!("cache:{file_id}")], |r| r.get(0))
        .optional()
        .map_err(|e| CoreError::Internal(e.to_string()))?;
    if let Some(s) = stored {
        if s != current_key {
            conn.execute(
                "DELETE FROM file_metadata WHERE file_id = ?1",
                rusqlite::params![file_id],
            )?;
            conn.execute(
                "DELETE FROM file_embeddings WHERE file_id = ?1",
                rusqlite::params![file_id],
            )?;
            return Ok(true);
        }
    }
    Ok(false)
}

/// Check if file is unchanged via hash + size.
pub fn is_unchanged(path: &Path, stored_hash: &str, stored_size: u64) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() != stored_size {
        return false;
    }
    let Ok(hash) = crate::hash::hash_file(path) else {
        return false;
    };
    hash == stored_hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::FileAnalysis;
    use crate::db::create_schema;
    use rusqlite::Connection;

    #[test]
    fn cache_roundtrip() {
        let mut a = FileAnalysis::new("/tmp/a.txt");
        a.size = Some(10);
        let hash = "abc";
        cache_put(hash, &a);
        let got = cache_get(hash, &a).unwrap();
        assert_eq!(got.path, a.path);
    }

    #[test]
    fn persist_and_load() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO files (id, path, filename, parent_directory) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["fid1", "/tmp/a.txt", "a.txt", "/tmp"],
        )
        .unwrap();
        let mut a = FileAnalysis::new("/tmp/a.txt");
        a.size = Some(5);
        a.embedding = Some(vec![0.1, 0.2, 0.3]);
        a.model_versions.embedding = Some("1.0.0".into());
        persist_analysis(&conn, "fid1", "hash123", &a).unwrap();
        let loaded = load_analysis(&conn, "fid1").unwrap().unwrap();
        assert_eq!(loaded.size, Some(5));
        assert_eq!(loaded.embedding.unwrap().len(), 3);
    }

    #[test]
    fn invalidate_on_version_change() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO files (id, path, filename, parent_directory) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["fid2", "/tmp/b.txt", "b.txt", "/tmp"],
        )
        .unwrap();
        let mut a = FileAnalysis::new("/tmp/b.txt");
        a.model_versions.embedding = Some("1.0.0".into());
        a.size = Some(1);
        persist_analysis(&conn, "fid2", "h1", &a).unwrap();
        let key_old = a.cache_key("h1");
        // same key -> no invalidation
        assert!(!invalidate_if_model_changed(&conn, "fid2", &key_old).unwrap());
        // new version -> invalidation
        a.model_versions.embedding = Some("2.0.0".into());
        let key_new = a.cache_key("h1");
        assert!(invalidate_if_model_changed(&conn, "fid2", &key_new).unwrap());
        assert!(load_analysis(&conn, "fid2").unwrap().is_none());
    }
}
