use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AiMode {
    #[default]
    Local,
    Hybrid,
    Online,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModeConfig {
    pub mode: AiMode,
    pub online_opt_in: bool,
    pub confidence_threshold: f32,
}

impl Default for AiModeConfig {
    fn default() -> Self {
        Self { mode: AiMode::Local, online_opt_in: false, confidence_threshold: 0.70 }
    }
}

impl AiModeConfig {
    pub fn new(mode: AiMode, online_opt_in: bool) -> Self {
        Self { mode, online_opt_in, confidence_threshold: 0.70 }
    }
    pub fn with_threshold(mut self, t: f32) -> Self { self.confidence_threshold = t; self }
    pub fn should_try_online(&self, local_confidence: f32, online_available: bool) -> bool {
        if !self.online_opt_in || !online_available { return false; }
        match self.mode {
            AiMode::Local => false,
            AiMode::Online => true,
            AiMode::Hybrid => local_confidence < self.confidence_threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hybrid_threshold() {
        let c = AiModeConfig { mode: AiMode::Hybrid, online_opt_in: true, confidence_threshold: 0.7 };
        assert!(c.should_try_online(0.6, true));
        assert!(!c.should_try_online(0.8, true));
        assert!(!c.should_try_online(0.6, false));
    }
    #[test]
    fn local_never() {
        let c = AiModeConfig { mode: AiMode::Local, online_opt_in: true, confidence_threshold: 0.7 };
        assert!(!c.should_try_online(0.1, true));
    }
    #[test]
    fn online_always_if_available() {
        let c = AiModeConfig { mode: AiMode::Online, online_opt_in: true, confidence_threshold: 0.7 };
        assert!(c.should_try_online(0.99, true));
        assert!(!c.should_try_online(0.99, false));
        let c2 = AiModeConfig { mode: AiMode::Online, online_opt_in: false, confidence_threshold: 0.7 };
        assert!(!c2.should_try_online(0.1, true));
    }
}
