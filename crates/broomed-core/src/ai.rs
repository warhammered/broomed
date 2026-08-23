use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::types::ProviderId;

// ── Task ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiTask {
    ClassifyFile,
    DescribeImage,
    SuggestFilename,
    SuggestFolder,
    DetectSemanticDuplicate,
    GenerateTags,
    SemanticSearch,
    SummarizeDocument,
}

// ── Capabilities ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiCapabilities {
    pub text: bool,
    pub vision: bool,
    pub embeddings: bool,
    pub structured_output: bool,
}

impl AiCapabilities {
    pub fn new(text: bool, vision: bool, embeddings: bool, structured_output: bool) -> Self {
        Self {
            text,
            vision,
            embeddings,
            structured_output,
        }
    }
}

// ── Provider config ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderConfig {
    pub id: ProviderId,
    pub name: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub vision_model: Option<String>,
    #[serde(default)]
    pub embedding_model: Option<String>,
    pub capabilities: AiCapabilities,
    pub priority: u8,
    pub enabled: bool,
}

// api_key deliberately omitted — passed at call site only

// ── Trait ───────────────────────────────────────────────────────────────

pub trait AiProvider {
    fn id(&self) -> &ProviderId;
    fn capabilities(&self) -> &AiCapabilities;
    fn priority(&self) -> u8;
    fn supports(&self, task: &AiTask) -> bool;
}

impl AiProvider for AiProviderConfig {
    fn id(&self) -> &ProviderId {
        &self.id
    }
    fn capabilities(&self) -> &AiCapabilities {
        &self.capabilities
    }
    fn priority(&self) -> u8 {
        self.priority
    }
    fn supports(&self, task: &AiTask) -> bool {
        match task {
            AiTask::DescribeImage => self.capabilities.vision,
            AiTask::DetectSemanticDuplicate | AiTask::SemanticSearch => {
                self.capabilities.embeddings
            }
            _ => self.capabilities.text,
        }
    }
}

// ── Router ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AiRouter {
    pub providers: Vec<AiProviderConfig>,
}

impl AiRouter {
    pub fn new(providers: Vec<AiProviderConfig>) -> Self {
        Self { providers }
    }

    /// Pick highest priority enabled provider whose capabilities fit task.
    /// Returns None if offline / no capable provider.
    pub fn route(&self, task: &AiTask) -> Option<&AiProviderConfig> {
        self.providers
            .iter()
            .filter(|p| p.enabled && p.supports(task))
            .max_by_key(|p| p.priority)
    }
}

// ── Confidence ─────────────────────────────────────────────────────────

pub const DEFAULT_THRESHOLD_HIGH: f32 = 0.90;
pub const DEFAULT_THRESHOLD_MED: f32 = 0.70;
pub const DEFAULT_THRESHOLD_LOW: f32 = 0.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Confidence {
    High(f32),
    Medium(f32),
    Low(f32),
}

pub fn classify_confidence(v: f32, threshold_high: f32, threshold_med: f32) -> Confidence {
    if v >= threshold_high {
        Confidence::High(v)
    } else if v >= threshold_med {
        Confidence::Medium(v)
    } else {
        Confidence::Low(v)
    }
}

// ── Result ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiResult {
    pub category: String,
    #[serde(default)]
    pub subcategory: Option<String>,
    pub confidence: f32,
    #[serde(default)]
    pub suggested_name: Option<String>,
    #[serde(default)]
    pub suggested_folder: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub reason: String,
}

fn extract_json(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.contains("```") {
        if let Some(start) = trimmed.find("```") {
            let after_start = &trimmed[start + 3..];
            if let Some(end) = after_start.find("```") {
                let inner = &after_start[..end];
                let inner = inner.trim();
                let inner = if inner.len() >= 4 && inner[..4].eq_ignore_ascii_case("json") {
                    inner[4..].trim()
                } else {
                    inner
                };
                return inner.to_string();
            }
            // single fence fallback
            let inner = after_start.trim();
            let inner = if inner.len() >= 4 && inner[..4].eq_ignore_ascii_case("json") {
                inner[4..].trim()
            } else {
                inner
            };
            if let Some(s) = inner.find('{') {
                if let Some(e) = inner.rfind('}') {
                    return inner[s..=e].to_string();
                }
            }
            return inner.to_string();
        }
    }
    if let Some(s) = trimmed.find('{') {
        if let Some(e) = trimmed.rfind('}') {
            return trimmed[s..=e].to_string();
        }
    }
    trimmed.to_string()
}

pub fn parse_ai_json(raw: &str) -> Result<AiResult, CoreError> {
    let json_str = extract_json(raw);
    let parsed: AiResult = serde_json::from_str(&json_str)
        .map_err(|e| CoreError::Internal(format!("invalid ai json: {e}")))?;
    if parsed.category.trim().is_empty() {
        return Err(CoreError::Internal("category must be non-empty".into()));
    }
    if !parsed.confidence.is_finite() || parsed.confidence < 0.0 || parsed.confidence > 1.0 {
        return Err(CoreError::Internal(format!(
            "confidence out of range 0.0-1.0: {}",
            parsed.confidence
        )));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_provider(id: &str, priority: u8, enabled: bool) -> AiProviderConfig {
        AiProviderConfig {
            id: ProviderId::new(id),
            name: id.to_string(),
            base_url: "https://api.example.com".into(),
            model: "text-model".into(),
            vision_model: None,
            embedding_model: None,
            capabilities: AiCapabilities {
                text: true,
                vision: false,
                embeddings: false,
                structured_output: true,
            },
            priority,
            enabled,
        }
    }

    fn vision_provider(id: &str, priority: u8, enabled: bool) -> AiProviderConfig {
        AiProviderConfig {
            id: ProviderId::new(id),
            name: id.to_string(),
            base_url: "https://api.example.com".into(),
            model: "vision-model".into(),
            vision_model: Some("vision-model".into()),
            embedding_model: None,
            capabilities: AiCapabilities {
                text: true,
                vision: true,
                embeddings: false,
                structured_output: true,
            },
            priority,
            enabled,
        }
    }

    fn embedding_provider(id: &str, priority: u8) -> AiProviderConfig {
        AiProviderConfig {
            id: ProviderId::new(id),
            name: id.to_string(),
            base_url: "https://api.example.com".into(),
            model: "embed-model".into(),
            vision_model: None,
            embedding_model: Some("embed-model".into()),
            capabilities: AiCapabilities {
                text: false,
                vision: false,
                embeddings: true,
                structured_output: false,
            },
            priority,
            enabled: true,
        }
    }

    #[test]
    fn route_describe_image_picks_vision_high_priority() {
        let router = AiRouter::new(vec![
            text_provider("text-low", 5, true),
            vision_provider("vision-high", 10, true),
        ]);
        let picked = router.route(&AiTask::DescribeImage).unwrap();
        assert_eq!(picked.id.as_str(), "vision-high");
    }

    #[test]
    fn route_classify_file_fallback_text() {
        // vision disabled -> fallback to text provider
        let router = AiRouter::new(vec![
            vision_provider("vision", 10, false),
            text_provider("text", 5, true),
        ]);
        let picked = router.route(&AiTask::ClassifyFile).unwrap();
        assert_eq!(picked.id.as_str(), "text");
    }

    #[test]
    fn route_classify_file_picks_highest_enabled() {
        let router = AiRouter::new(vec![
            text_provider("low", 5, true),
            vision_provider("high", 20, true),
        ]);
        let picked = router.route(&AiTask::ClassifyFile).unwrap();
        assert_eq!(picked.id.as_str(), "high");
    }

    #[test]
    fn route_disabled_provider_ignored() {
        let router = AiRouter::new(vec![vision_provider("v", 10, false)]);
        assert!(router.route(&AiTask::DescribeImage).is_none());
    }

    #[test]
    fn route_no_capable_returns_none() {
        let router = AiRouter::new(vec![text_provider("t", 5, true)]);
        assert!(router.route(&AiTask::DescribeImage).is_none());
        assert!(router.route(&AiTask::SemanticSearch).is_none());
    }

    #[test]
    fn route_embeddings_task() {
        let router = AiRouter::new(vec![
            text_provider("t", 10, true),
            embedding_provider("e", 8),
        ]);
        let picked = router.route(&AiTask::SemanticSearch).unwrap();
        assert_eq!(picked.id.as_str(), "e");
        let picked2 = router.route(&AiTask::DetectSemanticDuplicate).unwrap();
        assert_eq!(picked2.id.as_str(), "e");
    }

    #[test]
    fn route_offline_empty() {
        let router = AiRouter::new(vec![]);
        assert!(router.route(&AiTask::ClassifyFile).is_none());
    }

    #[test]
    fn supports_text_task() {
        let p = text_provider("t", 5, true);
        assert!(p.supports(&AiTask::ClassifyFile));
        assert!(p.supports(&AiTask::GenerateTags));
        assert!(!p.supports(&AiTask::DescribeImage));
    }

    #[test]
    fn supports_vision_false() {
        let p = text_provider("t", 5, true);
        assert!(!p.supports(&AiTask::DescribeImage));
        let v = vision_provider("v", 5, true);
        assert!(v.supports(&AiTask::DescribeImage));
    }

    #[test]
    fn parse_valid_json() {
        let raw = r#"{"category":"docs","confidence":0.95,"tags":["a"],"reason":"ok"}"#;
        let r = parse_ai_json(raw).unwrap();
        assert_eq!(r.category, "docs");
        assert_eq!(r.confidence, 0.95);
    }

    #[test]
    fn parse_fenced_json() {
        let raw = "```json\n{\"category\":\"image\",\"confidence\":0.82,\"reason\":\"looks like cat\"}\n```";
        let r = parse_ai_json(raw).unwrap();
        assert_eq!(r.category, "image");
    }

    #[test]
    fn parse_fenced_json_no_lang() {
        let raw = "```\n{\"category\":\"x\",\"confidence\":0.5,\"reason\":\"y\"}\n```";
        let r = parse_ai_json(raw).unwrap();
        assert_eq!(r.category, "x");
    }

    #[test]
    fn parse_fenced_json_extra_text() {
        let raw = "here is result:\n```json\n{\"category\":\"finance\",\"confidence\":0.91,\"reason\":\"invoice\"}\n```\ndone";
        let r = parse_ai_json(raw).unwrap();
        assert_eq!(r.category, "finance");
    }

    #[test]
    fn parse_all_fields() {
        let raw = r#"{"category":"cat","subcategory":"sub","confidence":0.77,"suggested_name":"foo.txt","suggested_folder":"/docs","tags":["t1","t2"],"reason":"r"}"#;
        let r = parse_ai_json(raw).unwrap();
        assert_eq!(r.subcategory.as_deref(), Some("sub"));
        assert_eq!(r.suggested_name.as_deref(), Some("foo.txt"));
        assert_eq!(r.tags.len(), 2);
    }

    #[test]
    fn parse_rejects_missing_category() {
        let raw = r#"{"confidence":0.9,"reason":"no cat"}"#;
        assert!(parse_ai_json(raw).is_err());
    }

    #[test]
    fn parse_rejects_empty_category() {
        let raw = r#"{"category":"","confidence":0.9,"reason":"empty"}"#;
        assert!(parse_ai_json(raw).is_err());
        let raw2 = r#"{"category":"   ","confidence":0.9,"reason":"blank"}"#;
        assert!(parse_ai_json(raw2).is_err());
    }

    #[test]
    fn parse_rejects_confidence_out_of_range_high() {
        let raw = r#"{"category":"x","confidence":1.5,"reason":"bad"}"#;
        assert!(parse_ai_json(raw).is_err());
    }

    #[test]
    fn parse_rejects_confidence_negative() {
        let raw = r#"{"category":"x","confidence":-0.1,"reason":"bad"}"#;
        assert!(parse_ai_json(raw).is_err());
    }

    #[test]
    fn parse_rejects_confidence_nan() {
        let raw = r#"{"category":"x","confidence":null,"reason":"bad"}"#;
        assert!(parse_ai_json(raw).is_err());
    }

    #[test]
    fn confidence_high_threshold() {
        assert!(matches!(
            classify_confidence(0.95, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::High(_)
        ));
        assert!(matches!(
            classify_confidence(0.90, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::High(_)
        ));
    }

    #[test]
    fn confidence_medium_threshold() {
        assert!(matches!(
            classify_confidence(0.85, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::Medium(_)
        ));
        assert!(matches!(
            classify_confidence(0.70, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::Medium(_)
        ));
        assert!(matches!(
            classify_confidence(0.89, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::Medium(_)
        ));
    }

    #[test]
    fn confidence_low_threshold() {
        assert!(matches!(
            classify_confidence(0.69, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::Low(_)
        ));
        assert!(matches!(
            classify_confidence(0.0, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::Low(_)
        ));
    }

    #[test]
    fn confidence_boundary_90_is_high() {
        let c = classify_confidence(0.90, 0.90, 0.70);
        assert_eq!(c, Confidence::High(0.90));
    }

    #[test]
    fn confidence_boundary_70_is_medium() {
        let c = classify_confidence(0.70, 0.90, 0.70);
        assert_eq!(c, Confidence::Medium(0.70));
    }

    #[test]
    fn confidence_custom_thresholds() {
        assert!(matches!(
            classify_confidence(0.8, 0.85, 0.6),
            Confidence::Medium(0.8)
        ));
        assert!(matches!(
            classify_confidence(0.86, 0.85, 0.6),
            Confidence::High(0.86)
        ));
    }
}
