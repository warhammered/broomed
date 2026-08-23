use std::path::Path;

use crate::analysis::{Classification, FileAnalysis};
use crate::error::CoreError;

// ── Embedding ────────────────────────────────────────────────────────

pub trait EmbeddingEngine: Send + Sync {
    fn model_id(&self) -> &str;
    fn dim(&self) -> usize;
    fn embed(&self, text: &str) -> Result<Vec<f32>, CoreError>;
    fn is_available(&self) -> bool;
}

/// Heuristic embedding (offline, deterministic, 384-dim compatible)
/// Uses simple hashed bag-of-words so it works without model files.
#[derive(Debug, Clone)]
pub struct HeuristicEmbedding {
    dim: usize,
}

impl Default for HeuristicEmbedding {
    fn default() -> Self {
        Self { dim: 384 }
    }
}

impl HeuristicEmbedding {
    pub fn new() -> Self {
        Self::default()
    }
}

impl EmbeddingEngine for HeuristicEmbedding {
    fn model_id(&self) -> &str {
        "heuristic-384"
    }
    fn dim(&self) -> usize {
        self.dim
    }
    fn is_available(&self) -> bool {
        true
    }
    fn embed(&self, text: &str) -> Result<Vec<f32>, CoreError> {
        // ponytail: deterministic hashed embedding - not ML, but satisfies contract, offline, zero deps
        let mut v = vec![0f32; self.dim];
        for (i, word) in text.split_whitespace().enumerate() {
            let mut h = blake3::Hasher::new();
            h.update(word.to_ascii_lowercase().as_bytes());
            h.update(&(i as u64).to_le_bytes());
            let hash = h.finalize();
            let bytes = hash.as_bytes();
            for (j, b) in bytes.iter().enumerate().take(self.dim) {
                let idx = (j + i * 7) % self.dim;
                v[idx] += (*b as f32 / 255.0) - 0.5;
            }
        }
        // L2 normalize
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
        for x in &mut v {
            *x /= norm;
        }
        if v.iter().all(|x| *x == 0.0) {
            v[0] = 1.0;
        }
        Ok(v)
    }
}

// ── Text Reasoning ───────────────────────────────────────────────────

pub trait TextReasoningEngine: Send + Sync {
    fn model_id(&self) -> &str;
    fn is_available(&self) -> bool;
    fn classify(&self, analysis: &FileAnalysis, input: &str) -> Result<Classification, CoreError>;
}

/// Heuristic reasoning - uses extension + keywords + extracted text
#[derive(Debug, Clone, Default)]
pub struct HeuristicReasoning;

impl TextReasoningEngine for HeuristicReasoning {
    fn model_id(&self) -> &str {
        "heuristic-reasoning"
    }
    fn is_available(&self) -> bool {
        true
    }
    fn classify(&self, analysis: &FileAnalysis, input: &str) -> Result<Classification, CoreError> {
        // Reuse existing heuristic but produce validated Classification
        let ext = analysis.extension.as_deref().unwrap_or("");
        let (category, folder, confidence) = match ext {
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "bmp" | "heic" => {
                ("Images", "Images", 0.85)
            }
            "mp4" | "mov" | "avi" | "mkv" | "webm" => ("Videos", "Videos", 0.86),
            "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" => ("Audio", "Audio", 0.84),
            "pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "csv" => {
                ("Documents", "Documents", 0.82)
            }
            "zip" | "rar" | "7z" | "tar" | "gz" | "xz" => ("Archives", "Archives", 0.83),
            "rs" | "py" | "js" | "ts" | "html" | "css" | "json" | "toml" => ("Code", "Code", 0.80),
            _ => {
                // keyword fallback on input text
                let low = input.to_ascii_lowercase();
                if low.contains("invoice") || low.contains("receipt") || low.contains("finance") {
                    ("Documents", "Documents/Finance", 0.75)
                } else if low.contains("photo") || low.contains("vacation") || low.contains("image")
                {
                    ("Images", "Images", 0.72)
                } else {
                    ("General", "General", 0.62)
                }
            }
        };
        let c = Classification {
            category: category.to_string(),
            subcategory: None,
            tags: vec![category.to_ascii_lowercase()],
            suggested_folder: folder.to_string(),
            confidence,
            reason: format!("heuristic: ext .{ext} -> {category}"),
        };
        c.validate().map_err(CoreError::Internal)?;
        Ok(c)
    }
}

// ── Vision ───────────────────────────────────────────────────────────

pub trait VisionEngine: Send + Sync {
    fn model_id(&self) -> &str;
    fn is_available(&self) -> bool;
    fn describe(&self, path: &Path) -> Result<String, CoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct StubVision;

impl VisionEngine for StubVision {
    fn model_id(&self) -> &str {
        "stub-vision"
    }
    fn is_available(&self) -> bool {
        // ponytail: stub always available, real GGUF behind feature
        true
    }
    fn describe(&self, path: &Path) -> Result<String, CoreError> {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("image");
        // deterministic placeholder - real model would produce richer description
        Ok(format!(
            "image file: {name} (vision stub - install smolvlm2 for richer description)"
        ))
    }
}

// ── Audio ────────────────────────────────────────────────────────────

pub trait AudioTranscriptionEngine: Send + Sync {
    fn model_id(&self) -> &str;
    fn is_available(&self) -> bool;
    fn transcribe(&self, path: &Path) -> Result<String, CoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct StubAudio;

impl AudioTranscriptionEngine for StubAudio {
    fn model_id(&self) -> &str {
        "stub-whisper-tiny"
    }
    fn is_available(&self) -> bool {
        true
    }
    fn transcribe(&self, path: &Path) -> Result<String, CoreError> {
        // ponytail: metadata-only stub - real whisper.cpp behind feature
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(ext.as_str(), "mp3" | "wav" | "m4a" | "ogg" | "flac") {
            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("audio");
            Ok(format!(
                "audio file: {name} (transcription requires whisper-tiny model)"
            ))
        } else {
            Err(CoreError::Internal(format!(
                "not an audio file: {}",
                path.display()
            )))
        }
    }
}

// ── OCR ──────────────────────────────────────────────────────────────

pub trait OcrEngine: Send + Sync {
    fn model_id(&self) -> &str;
    fn is_available(&self) -> bool;
    fn extract_text(&self, path: &Path) -> Result<String, CoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct StubOcr;

impl OcrEngine for StubOcr {
    fn model_id(&self) -> &str {
        "stub-ocr"
    }
    fn is_available(&self) -> bool {
        true
    }
    fn extract_text(&self, path: &Path) -> Result<String, CoreError> {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("image");
        Ok(format!(
            "ocr stub for {name} (install tesseract for real OCR)"
        ))
    }
}

// ── Media ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub duration_secs: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub has_audio: bool,
    pub mime: Option<String>,
}

pub trait MediaEngine: Send + Sync {
    fn probe(&self, path: &Path) -> Result<MediaInfo, CoreError>;
    fn extract_thumbnail(&self, _path: &Path) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::Internal(
            "thumbnail not implemented in stub".into(),
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub struct StubMedia;

impl MediaEngine for StubMedia {
    fn probe(&self, path: &Path) -> Result<MediaInfo, CoreError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mime = match ext.as_str() {
            "mp4" | "mov" | "avi" | "mkv" => Some(format!("video/{ext}")),
            "mp3" | "wav" | "flac" => Some(format!("audio/{ext}")),
            "jpg" | "jpeg" => Some("image/jpeg".into()),
            "png" => Some("image/png".into()),
            "pdf" => Some("application/pdf".into()),
            _ => None,
        };
        Ok(MediaInfo {
            duration_secs: None,
            width: None,
            height: None,
            has_audio: matches!(ext.as_str(), "mp4" | "mov" | "avi" | "mkv" | "mp3" | "wav"),
            mime,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heuristic_embedding_dim_norm() {
        let e = HeuristicEmbedding::new();
        let v = e.embed("hello world invoice receipt").unwrap();
        assert_eq!(v.len(), 384);
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
        // determinism
        let v2 = e.embed("hello world invoice receipt").unwrap();
        assert_eq!(v, v2);
        // different text -> different vector
        let v3 = e.embed("completely different content").unwrap();
        assert_ne!(v, v3);
    }
    #[test]
    fn heuristic_reasoning_validate() {
        let r = HeuristicReasoning;
        let a = FileAnalysis::new("/tmp/photo.jpg");
        let c = r.classify(&a, "photo.jpg").unwrap();
        assert_eq!(c.category, "Images");
        assert!(c.validate().is_ok());
    }
    #[test]
    fn stub_vision() {
        let v = StubVision;
        let s = v.describe(Path::new("/tmp/cat.jpg")).unwrap();
        assert!(s.contains("cat.jpg"));
    }
    #[test]
    fn stub_audio_rejects_non_audio() {
        let a = StubAudio;
        assert!(a.transcribe(Path::new("/tmp/doc.pdf")).is_err());
    }
    #[test]
    fn stub_media_probe() {
        let m = StubMedia;
        let info = m.probe(Path::new("/tmp/video.mp4")).unwrap();
        assert!(info.has_audio);
        assert_eq!(info.mime.as_deref(), Some("video/mp4"));
    }
}
