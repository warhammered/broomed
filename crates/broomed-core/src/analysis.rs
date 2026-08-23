use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Normalized internal representation for analyzed files.
/// All AI fields coexist without coupling to a specific model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysis {
    pub file_id: String,
    pub path: PathBuf,
    pub mime_type: Option<String>,
    pub extension: Option<String>,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
    pub metadata: FileMetadata,
    pub extracted_text: Option<String>,
    pub ocr_text: Option<String>,
    pub transcript: Option<String>,
    pub visual_summary: Option<String>,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub confidence: Option<f32>,
    pub reasoning: Option<String>,
    /// Which model versions produced this analysis (for cache invalidation)
    pub model_versions: ModelVersions,
    pub pipeline_version: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileMetadata {
    pub filename: String,
    pub exif: Option<String>,
    pub id3: Option<Id3Metadata>,
    pub duration_secs: Option<f64>,
    pub bitrate: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Id3Metadata {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelVersions {
    pub embedding: Option<String>,
    pub reasoning: Option<String>,
    pub vision: Option<String>,
    pub audio: Option<String>,
}

impl FileAnalysis {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let p = path.into();
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase());
        let filename = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        Self {
            file_id: uuid::Uuid::new_v4().to_string(),
            path: p,
            mime_type: None,
            extension: ext,
            size: None,
            modified_at: None,
            metadata: FileMetadata {
                filename,
                ..Default::default()
            },
            extracted_text: None,
            ocr_text: None,
            transcript: None,
            visual_summary: None,
            tags: Vec::new(),
            category: None,
            subcategory: None,
            embedding: None,
            confidence: None,
            reasoning: None,
            model_versions: ModelVersions::default(),
            pipeline_version: 1,
        }
    }

    /// Cache key incorporates content hash + size + metadata + model versions + pipeline version.
    pub fn cache_key(&self, content_hash: &str) -> String {
        let mut s = String::new();
        s.push_str(content_hash);
        s.push('|');
        s.push_str(&self.size.unwrap_or(0).to_string());
        s.push('|');
        s.push_str(self.mime_type.as_deref().unwrap_or(""));
        s.push('|');
        s.push_str(self.extension.as_deref().unwrap_or(""));
        s.push('|');
        s.push_str(self.model_versions.embedding.as_deref().unwrap_or("none"));
        s.push('|');
        s.push_str(self.model_versions.reasoning.as_deref().unwrap_or("none"));
        s.push('|');
        s.push_str(self.model_versions.vision.as_deref().unwrap_or("none"));
        s.push('|');
        s.push_str(&self.pipeline_version.to_string());
        // no file path in key - hash covers content identity
        // stable hash of key for storage
        let mut hasher = blake3::Hasher::new();
        hasher.update(s.as_bytes());
        hasher.finalize().to_hex().to_string()
    }
}

/// Structured output preferred for classification (validated, deterministic).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Classification {
    pub category: String,
    pub subcategory: Option<String>,
    pub tags: Vec<String>,
    pub suggested_folder: String,
    pub confidence: f32,
    pub reason: String,
}

impl Classification {
    pub fn validate(&self) -> Result<(), String> {
        if self.category.trim().is_empty() {
            return Err("category must be non-empty".into());
        }
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err(format!("confidence out of range: {}", self.confidence));
        }
        if self.suggested_folder.contains("..") || self.suggested_folder.contains('\0') {
            return Err("suggested_folder contains unsafe components".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_key_stable() {
        let mut a = FileAnalysis::new("/tmp/foo.pdf");
        a.size = Some(123);
        a.mime_type = Some("application/pdf".into());
        a.model_versions.embedding = Some("1.0.0".into());
        let k1 = a.cache_key("abc123");
        let k2 = a.cache_key("abc123");
        assert_eq!(k1, k2);
        // different hash -> different key
        let k3 = a.cache_key("different");
        assert_ne!(k1, k3);
        // model version change -> different key
        a.model_versions.embedding = Some("2.0.0".into());
        let k4 = a.cache_key("abc123");
        assert_ne!(k1, k4);
    }
    #[test]
    fn classification_validation() {
        let c = Classification {
            category: "Documents".into(),
            subcategory: None,
            tags: vec!["docs".into()],
            suggested_folder: "Documents".into(),
            confidence: 0.9,
            reason: "test".into(),
        };
        assert!(c.validate().is_ok());
        let bad = Classification {
            category: "".into(),
            ..c.clone()
        };
        assert!(bad.validate().is_err());
        let bad2 = Classification {
            confidence: 2.0,
            ..c.clone()
        };
        assert!(bad2.validate().is_err());
        let bad3 = Classification {
            suggested_folder: "../evil".into(),
            ..c
        };
        assert!(bad3.validate().is_err());
    }
}
