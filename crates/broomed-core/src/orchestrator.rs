use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::analysis::{Classification, FileAnalysis};
use crate::engines::{
    AudioTranscriptionEngine, EmbeddingEngine, HeuristicEmbedding, HeuristicReasoning, MediaEngine,
    OcrEngine, StubAudio, StubMedia, StubOcr, StubVision, TextReasoningEngine, VisionEngine,
};
use crate::error::CoreError;
use crate::hardware::HardwareInfo;

/// Decision about which AI stages are necessary for a file.
#[derive(Debug, Clone)]
pub struct OrchestratorDecision {
    pub needs_text: bool,
    pub needs_ocr: bool,
    pub needs_vision: bool,
    pub needs_audio: bool,
    pub needs_embedding: bool,
    pub reason: String,
}

impl OrchestratorDecision {
    pub fn all_off(reason: impl Into<String>) -> Self {
        Self {
            needs_text: false,
            needs_ocr: false,
            needs_vision: false,
            needs_audio: false,
            needs_embedding: false,
            reason: reason.into(),
        }
    }
}

/// Central orchestrator - decides minimal required specialists.
pub struct Orchestrator {
    pub hardware: HardwareInfo,
    pub embedding: Arc<dyn EmbeddingEngine>,
    pub reasoning: Arc<dyn TextReasoningEngine>,
    pub vision: Arc<dyn VisionEngine>,
    pub audio: Arc<dyn AudioTranscriptionEngine>,
    pub ocr: Arc<dyn OcrEngine>,
    pub media: Arc<dyn MediaEngine>,
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            hardware: HardwareInfo::detect(),
            embedding: Arc::new(HeuristicEmbedding::new()),
            reasoning: Arc::new(HeuristicReasoning),
            vision: Arc::new(StubVision),
            audio: Arc::new(StubAudio),
            ocr: Arc::new(StubOcr),
            media: Arc::new(StubMedia),
        }
    }

    pub fn with_hardware(mut self, hw: HardwareInfo) -> Self {
        self.hardware = hw;
        self
    }

    /// Decide which stages are needed based on deterministic metadata.
    pub fn decide(
        &self,
        path: &Path,
        mime_hint: Option<&str>,
        has_text: bool,
        has_id3: bool,
    ) -> OrchestratorDecision {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        // MP3 with complete ID3 -> skip audio transcription
        if matches!(ext.as_str(), "mp3" | "m4a" | "wav" | "flac" | "ogg") {
            if has_id3 {
                return OrchestratorDecision {
                    needs_text: true,
                    needs_ocr: false,
                    needs_vision: false,
                    needs_audio: false,
                    needs_embedding: true,
                    reason: "audio with ID3 - skip transcription".into(),
                };
            }
            return OrchestratorDecision {
                needs_text: true,
                needs_ocr: false,
                needs_vision: false,
                needs_audio: true,
                needs_embedding: true,
                reason: "audio without metadata - transcribe".into(),
            };
        }

        // TXT/MD/code -> text + embedding only
        if matches!(
            ext.as_str(),
            "txt" | "md" | "rs" | "py" | "js" | "ts" | "json" | "toml" | "csv" | "html" | "css"
        ) {
            return OrchestratorDecision {
                needs_text: true,
                needs_ocr: false,
                needs_vision: false,
                needs_audio: false,
                needs_embedding: true,
                reason: "text/code file".into(),
            };
        }

        // PDF with extractable text -> text only, skip OCR
        if ext == "pdf" && has_text {
            return OrchestratorDecision {
                needs_text: true,
                needs_ocr: false,
                needs_vision: false,
                needs_audio: false,
                needs_embedding: true,
                reason: "pdf with extractable text - skip OCR".into(),
            };
        }
        if ext == "pdf" {
            return OrchestratorDecision {
                needs_text: true,
                needs_ocr: true,
                needs_vision: false,
                needs_audio: false,
                needs_embedding: true,
                reason: "scanned pdf - needs OCR".into(),
            };
        }

        // JPEG with EXIF only -> maybe skip vision if metadata strong? but vision helps
        if matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "bmp"
        ) {
            // ponytail: vision only when visual understanding would materially improve
            // for now use filename heuristic - if filename is descriptive, skip vision
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let has_descriptive = stem.contains("vacation")
                || stem.contains("receipt")
                || stem.contains("screenshot");
            if has_descriptive {
                return OrchestratorDecision {
                    needs_text: true,
                    needs_ocr: false,
                    needs_vision: false,
                    needs_audio: false,
                    needs_embedding: true,
                    reason: "image with descriptive filename - skip vision".into(),
                };
            }
            return OrchestratorDecision {
                needs_text: true,
                needs_ocr: false,
                needs_vision: true,
                needs_audio: false,
                needs_embedding: true,
                reason: "image needs visual summary".into(),
            };
        }

        // Video -> metadata + keyframes vision only when useful, audio transcription optionally
        if matches!(ext.as_str(), "mp4" | "mov" | "avi" | "mkv" | "webm") {
            return OrchestratorDecision {
                needs_text: true,
                needs_ocr: false,
                needs_vision: true,
                needs_audio: true,
                needs_embedding: true,
                reason: "video - sampled vision + optional audio".into(),
            };
        }

        // fallback
        let mime_needs_vision = mime_hint.is_some_and(|m| m.starts_with("image/"));
        OrchestratorDecision {
            needs_text: true,
            needs_ocr: false,
            needs_vision: mime_needs_vision,
            needs_audio: false,
            needs_embedding: true,
            reason: "fallback - text+embedding".into(),
        }
    }

    /// Analyze a file into normalized FileAnalysis (queue-friendly, bounded).
    pub fn analyze(&self, path: &Path) -> Result<FileAnalysis, CoreError> {
        let mut analysis = FileAnalysis::new(path);
        // size + mime probing (deterministic, no AI)
        if let Ok(meta) = std::fs::metadata(path) {
            analysis.size = Some(meta.len());
        }
        let media_info = self.media.probe(path).unwrap_or(crate::engines::MediaInfo {
            duration_secs: None,
            width: None,
            height: None,
            has_audio: false,
            mime: None,
        });
        analysis.mime_type = media_info.mime.clone();
        analysis.metadata.width = media_info.width;
        analysis.metadata.height = media_info.height;
        analysis.metadata.duration_secs = media_info.duration_secs;

        // Try to read extracted_text for text files (streaming, bounded 1MB)
        let has_text = try_extract_text(path, &mut analysis).is_ok();
        let has_id3 = analysis.metadata.id3.is_some();

        let decision = self.decide(path, analysis.mime_type.as_deref(), has_text, has_id3);

        // invoke only required specialists
        if decision.needs_ocr {
            if let Ok(t) = self.ocr.extract_text(path) {
                analysis.ocr_text = Some(t);
            }
        }
        if decision.needs_vision {
            if let Ok(desc) = self.vision.describe(path) {
                analysis.visual_summary = Some(desc);
            }
        }
        if decision.needs_audio {
            if let Ok(tr) = self.audio.transcribe(path) {
                analysis.transcript = Some(tr);
            }
        }

        // classification + tags
        let input_text = analysis
            .extracted_text
            .as_deref()
            .or(analysis.ocr_text.as_deref())
            .or(analysis.transcript.as_deref())
            .or(analysis.visual_summary.as_deref())
            .unwrap_or(path.to_string_lossy().as_ref())
            .to_string();
        // include filename for better context
        let classify_input = format!("{} {}", analysis.metadata.filename, input_text);
        if decision.needs_text {
            let cls = self.reasoning.classify(&analysis, &classify_input)?;
            analysis.category = Some(cls.category.clone());
            analysis.subcategory = cls.subcategory.clone();
            analysis.tags = cls.tags.clone();
            analysis.confidence = Some(cls.confidence);
            analysis.reasoning = Some(cls.reason.clone());
            analysis.model_versions.reasoning = Some(self.reasoning.model_id().to_string());
        }

        // embeddings (always for search if needs_embedding)
        if decision.needs_embedding {
            let emb_input = format!(
                "{} {} {}",
                analysis.category.as_deref().unwrap_or(""),
                classify_input,
                analysis.tags.join(" ")
            );
            if let Ok(vec) = self.embedding.embed(&emb_input) {
                analysis.embedding = Some(vec);
                analysis.model_versions.embedding = Some(self.embedding.model_id().to_string());
            }
        }
        if decision.needs_vision {
            analysis.model_versions.vision = Some(self.vision.model_id().to_string());
        }
        if decision.needs_audio {
            analysis.model_versions.audio = Some(self.audio.model_id().to_string());
        }

        Ok(analysis)
    }

    /// Classify via reasoning engine directly (for plan_organize compat)
    pub fn classify_simple(&self, input: &str) -> Result<Classification, CoreError> {
        let analysis = FileAnalysis::new(PathBuf::from(input));
        self.reasoning.classify(&analysis, input)
    }
}

fn try_extract_text(path: &Path, analysis: &mut FileAnalysis) -> Result<(), CoreError> {
    let ext = analysis
        .extension
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        ext.as_str(),
        "txt" | "md" | "rs" | "py" | "js" | "ts" | "json" | "toml" | "csv" | "html" | "css" | "pdf"
    ) {
        return Err(CoreError::Internal("not a text file".into()));
    }
    // ponytail: streaming, bounded 1MB, never hold entire file in memory unnecessarily
    let mut s = String::new();
    let mut f = std::fs::File::open(path).map_err(|e| CoreError::Io(e.to_string()))?;
    use std::io::Read;
    let mut buf = [0u8; 8192];
    let mut total = 0usize;
    const LIMIT: usize = 1024 * 1024;
    loop {
        let n = f.read(&mut buf).map_err(|e| CoreError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        if total + n > LIMIT {
            s.push_str(&String::from_utf8_lossy(&buf[..LIMIT - total]));
            break;
        }
        s.push_str(&String::from_utf8_lossy(&buf[..n]));
        total += n;
    }
    if s.trim().is_empty() {
        return Err(CoreError::Internal("empty text".into()));
    }
    analysis.extracted_text = Some(s);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mp3_with_id3_skips_audio() {
        let orch = Orchestrator::new();
        let d = orch.decide(Path::new("/tmp/song.mp3"), None, false, true);
        assert!(!d.needs_audio, "should skip whisper when ID3 present");
        assert!(d.needs_embedding);
    }
    #[test]
    fn mp3_without_id3_needs_audio() {
        let orch = Orchestrator::new();
        let d = orch.decide(Path::new("/tmp/voice.m4a"), None, false, false);
        assert!(d.needs_audio);
    }
    #[test]
    fn txt_needs_text_not_vision() {
        let orch = Orchestrator::new();
        let d = orch.decide(Path::new("/tmp/readme.md"), None, true, false);
        assert!(d.needs_text);
        assert!(!d.needs_vision);
        assert!(!d.needs_audio);
    }
    #[test]
    fn pdf_with_text_skips_ocr() {
        let orch = Orchestrator::new();
        let d = orch.decide(
            Path::new("/tmp/doc.pdf"),
            Some("application/pdf"),
            true,
            false,
        );
        assert!(!d.needs_ocr);
    }
    #[test]
    fn pdf_scanned_needs_ocr() {
        let orch = Orchestrator::new();
        let d = orch.decide(Path::new("/tmp/scan.pdf"), None, false, false);
        assert!(d.needs_ocr);
    }
    #[test]
    fn video_needs_vision_and_audio() {
        let orch = Orchestrator::new();
        let d = orch.decide(Path::new("/tmp/clip.mp4"), None, false, false);
        assert!(d.needs_vision);
        assert!(d.needs_audio);
    }
    #[test]
    fn analyze_text_file() {
        let dir = std::env::temp_dir().join(format!("broomed_orch_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("hello.txt");
        std::fs::write(&p, b"hello world invoice receipt").unwrap();
        let orch = Orchestrator::new();
        let a = orch.analyze(&p).unwrap();
        assert!(a.category.is_some());
        assert!(a.embedding.is_some());
        assert_eq!(a.embedding.as_ref().unwrap().len(), 384);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
