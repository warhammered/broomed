use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MascotState {
    Idle,
    Curious,
    Scanning,
    Thinking,
    Found,
    Organizing,
    WaitingConfirmation,
    Success,
    Warning,
    Error,
    Offline,
    Sleeping,
}

impl MascotState {
    /// Priority: error > offline > waiting > organizing > scanning > thinking > found > curious > idle.
    /// Sleeping only via explicit set (never returned here).
    /// Curious / Success / Warning are explicit-only in this mapping (no dedicated bool);
    /// `has_results` maps to Found, otherwise fallback is Idle.
    pub fn from_app_state(
        scan_running: bool,
        ai_thinking: bool,
        has_results: bool,
        organizing: bool,
        waiting: bool,
        error: bool,
        offline: bool,
    ) -> Self {
        if error {
            Self::Error
        } else if offline {
            Self::Offline
        } else if waiting {
            Self::WaitingConfirmation
        } else if organizing {
            Self::Organizing
        } else if scan_running {
            Self::Scanning
        } else if ai_thinking {
            Self::Thinking
        } else if has_results {
            Self::Found
        } else {
            // Curious is explicit-only; Idle is the default quiescent state.
            Self::Idle
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Curious => "Curious",
            Self::Scanning => "Scanning",
            Self::Thinking => "Thinking",
            Self::Found => "Found",
            Self::Organizing => "Organizing",
            Self::WaitingConfirmation => "Waiting for confirmation",
            Self::Success => "Success",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Offline => "Offline",
            Self::Sleeping => "Sleeping",
        }
    }

    pub fn animation_hint(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Curious => "pulse",
            Self::Scanning => "spin",
            Self::Thinking => "pulse",
            Self::Found => "bounce",
            Self::Organizing => "spin",
            Self::WaitingConfirmation => "pulse",
            Self::Success => "bounce",
            Self::Warning => "shake",
            Self::Error => "shake",
            Self::Offline => "pulse",
            Self::Sleeping => "idle",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_state() {
        let s = MascotState::from_app_state(false, false, false, false, false, false, false);
        assert_eq!(s, MascotState::Idle);
        assert_eq!(s.label(), "Idle");
        assert_eq!(s.animation_hint(), "idle");
    }

    #[test]
    fn curious_state() {
        let s = MascotState::Curious;
        assert_eq!(s.label(), "Curious");
        assert_eq!(s.animation_hint(), "pulse");
        // serde roundtrip
        let json = serde_json::to_string(&s).unwrap();
        let back: MascotState = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn scanning_state() {
        let s = MascotState::from_app_state(true, false, false, false, false, false, false);
        assert_eq!(s, MascotState::Scanning);
        assert_eq!(s.label(), "Scanning");
        assert_eq!(s.animation_hint(), "spin");
    }

    #[test]
    fn thinking_state() {
        let s = MascotState::from_app_state(false, true, false, false, false, false, false);
        assert_eq!(s, MascotState::Thinking);
        assert_eq!(s.label(), "Thinking");
        assert_eq!(s.animation_hint(), "pulse");
    }

    #[test]
    fn found_state() {
        let s = MascotState::from_app_state(false, false, true, false, false, false, false);
        assert_eq!(s, MascotState::Found);
        assert_eq!(s.label(), "Found");
        assert_eq!(s.animation_hint(), "bounce");
    }

    #[test]
    fn organizing_state() {
        let s = MascotState::from_app_state(false, false, false, true, false, false, false);
        assert_eq!(s, MascotState::Organizing);
        assert_eq!(s.label(), "Organizing");
        assert_eq!(s.animation_hint(), "spin");
    }

    #[test]
    fn waiting_confirmation_state() {
        let s = MascotState::from_app_state(false, false, false, false, true, false, false);
        assert_eq!(s, MascotState::WaitingConfirmation);
        assert_eq!(s.label(), "Waiting for confirmation");
        assert_eq!(s.animation_hint(), "pulse");
    }

    #[test]
    fn success_state() {
        let s = MascotState::Success;
        assert_eq!(s.label(), "Success");
        assert_eq!(s.animation_hint(), "bounce");
    }

    #[test]
    fn warning_state() {
        let s = MascotState::Warning;
        assert_eq!(s.label(), "Warning");
        assert_eq!(s.animation_hint(), "shake");
    }

    #[test]
    fn error_state_priority() {
        // error overrides all
        let s = MascotState::from_app_state(true, true, true, true, true, true, true);
        assert_eq!(s, MascotState::Error);
        assert_eq!(s.label(), "Error");
        assert_eq!(s.animation_hint(), "shake");
    }

    #[test]
    fn offline_state_priority() {
        // offline beats everything except error
        let s = MascotState::from_app_state(true, true, true, true, true, false, true);
        assert_eq!(s, MascotState::Offline);
        assert_eq!(s.label(), "Offline");
        assert_eq!(s.animation_hint(), "pulse");
    }

    #[test]
    fn sleeping_state() {
        let s = MascotState::Sleeping;
        assert_eq!(s.label(), "Sleeping");
        assert_eq!(s.animation_hint(), "idle");
        // sleeping never returned by from_app_state
        let auto = MascotState::from_app_state(false, false, false, false, false, false, false);
        assert_ne!(auto, MascotState::Sleeping);
        let auto2 = MascotState::from_app_state(true, true, true, true, true, true, true);
        assert_ne!(auto2, MascotState::Sleeping);
    }

    #[test]
    fn priority_chain() {
        // waiting > organizing > scanning > thinking > found
        assert_eq!(
            MascotState::from_app_state(false, false, false, true, true, false, false),
            MascotState::WaitingConfirmation
        );
        assert_eq!(
            MascotState::from_app_state(true, false, false, true, false, false, false),
            MascotState::Organizing
        );
        assert_eq!(
            MascotState::from_app_state(true, true, false, false, false, false, false),
            MascotState::Scanning
        );
        assert_eq!(
            MascotState::from_app_state(false, true, true, false, false, false, false),
            MascotState::Thinking
        );
    }
}
