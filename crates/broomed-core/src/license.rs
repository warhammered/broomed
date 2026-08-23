use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroize;

use crate::device::DeviceIdentity;
use crate::error::CoreError;
use crate::secure_store::SecureStore;

// 32 zero bytes base64 placeholder for production; tests use generated key
pub const BROOMED_SERVER_PUBLIC_KEY_B64: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
pub const LICENSE_GRACE_SECS: i64 = 259200; // 72h
pub const REFRESH_WINDOW_SECS: i64 = 86400; // 24h

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LicenseError {
    #[error("invalid activation code")]
    InvalidActivationCode,
    #[error("activation expired")]
    ActivationExpired,
    #[error("activation already used")]
    ActivationAlreadyUsed,
    #[error("subscription inactive")]
    SubscriptionInactive,
    #[error("subscription expired")]
    SubscriptionExpired,
    #[error("device already bound")]
    DeviceAlreadyBound,
    #[error("device deactivated")]
    DeviceDeactivated,
    #[error("license expired")]
    LicenseExpired,
    #[error("license refresh failed")]
    LicenseRefreshFailed,
    #[error("server unavailable")]
    ServerUnavailable,
    #[error("online AI disabled")]
    OnlineAiDisabled,
    #[error("online AI quota exceeded")]
    OnlineAiQuotaExceeded,
    #[error("unsupported AI capability")]
    UnsupportedAiCapability,
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("network error: {0}")]
    Network(String),
}

impl LicenseError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidActivationCode => "INVALID_ACTIVATION_CODE",
            Self::ActivationExpired => "ACTIVATION_EXPIRED",
            Self::ActivationAlreadyUsed => "ACTIVATION_ALREADY_USED",
            Self::SubscriptionInactive => "SUBSCRIPTION_INACTIVE",
            Self::SubscriptionExpired => "SUBSCRIPTION_EXPIRED",
            Self::DeviceAlreadyBound => "DEVICE_ALREADY_BOUND",
            Self::DeviceDeactivated => "DEVICE_DEACTIVATED",
            Self::LicenseExpired => "LICENSE_EXPIRED",
            Self::LicenseRefreshFailed => "LICENSE_REFRESH_FAILED",
            Self::ServerUnavailable => "SERVER_UNAVAILABLE",
            Self::OnlineAiDisabled => "ONLINE_AI_DISABLED",
            Self::OnlineAiQuotaExceeded => "ONLINE_AI_QUOTA_EXCEEDED",
            Self::UnsupportedAiCapability => "UNSUPPORTED_AI_CAPABILITY",
            Self::InvalidResponse(_) => "INVALID_RESPONSE",
            Self::Storage(_) => "STORAGE_ERROR",
            Self::Crypto(_) => "CRYPTO_ERROR",
            Self::Network(_) => "NETWORK_ERROR",
        }
    }
    pub fn from_code(s: &str) -> Option<Self> {
        match s {
            "INVALID_ACTIVATION_CODE" => Some(Self::InvalidActivationCode),
            "ACTIVATION_EXPIRED" => Some(Self::ActivationExpired),
            "ACTIVATION_ALREADY_USED" => Some(Self::ActivationAlreadyUsed),
            "SUBSCRIPTION_INACTIVE" => Some(Self::SubscriptionInactive),
            "SUBSCRIPTION_EXPIRED" => Some(Self::SubscriptionExpired),
            "DEVICE_ALREADY_BOUND" => Some(Self::DeviceAlreadyBound),
            "DEVICE_DEACTIVATED" => Some(Self::DeviceDeactivated),
            "LICENSE_EXPIRED" => Some(Self::LicenseExpired),
            "LICENSE_REFRESH_FAILED" => Some(Self::LicenseRefreshFailed),
            "SERVER_UNAVAILABLE" => Some(Self::ServerUnavailable),
            "ONLINE_AI_DISABLED" => Some(Self::OnlineAiDisabled),
            "ONLINE_AI_QUOTA_EXCEEDED" => Some(Self::OnlineAiQuotaExceeded),
            "UNSUPPORTED_AI_CAPABILITY" => Some(Self::UnsupportedAiCapability),
            _ => None,
        }
    }
}

impl From<LicenseError> for CoreError {
    fn from(e: LicenseError) -> Self {
        CoreError::Internal(format!("{}: {}", e.code(), e))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entitlement {
    pub subscription_status: String,
    pub entitlement: String,
    pub device_id: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub license_id: String,
    pub server_version: String,
    pub period_end: Option<i64>,
    pub signature: String,
}

impl Entitlement {
    pub fn is_valid(&self, now: i64) -> bool {
        now < self.expires_at
            && (self.subscription_status == "active" || self.is_canceled_valid(now))
    }
    fn is_canceled_valid(&self, now: i64) -> bool {
        if self.subscription_status == "canceled" {
            if let Some(pe) = self.period_end {
                return now < pe;
            }
        }
        false
    }
    pub fn needs_refresh(&self, now: i64) -> bool {
        self.expires_at - now < REFRESH_WINDOW_SECS
    }
    pub fn is_within_grace(&self, now: i64, grace_secs: i64) -> bool {
        if self.is_valid(now) {
            return true;
        }
        now < self.expires_at + grace_secs
    }
    pub fn is_online_ai_entitled(&self) -> bool {
        self.entitlement == "online_ai"
    }
}

fn canonical_bytes(ent: &Entitlement) -> Result<Vec<u8>, LicenseError> {
    // canonical JSON without signature, sorted keys
    let mut map = serde_json::Map::new();
    map.insert(
        "device_id".into(),
        serde_json::Value::String(ent.device_id.clone()),
    );
    map.insert(
        "entitlement".into(),
        serde_json::Value::String(ent.entitlement.clone()),
    );
    map.insert(
        "expires_at".into(),
        serde_json::Value::Number(ent.expires_at.into()),
    );
    map.insert(
        "issued_at".into(),
        serde_json::Value::Number(ent.issued_at.into()),
    );
    map.insert(
        "license_id".into(),
        serde_json::Value::String(ent.license_id.clone()),
    );
    if let Some(pe) = ent.period_end {
        map.insert("period_end".into(), serde_json::Value::Number(pe.into()));
    }
    map.insert(
        "server_version".into(),
        serde_json::Value::String(ent.server_version.clone()),
    );
    map.insert(
        "subscription_status".into(),
        serde_json::Value::String(ent.subscription_status.clone()),
    );
    // sorted via BTreeMap iteration already sorted; serde_json Map is BTreeMap internally? Use string
    let v = serde_json::Value::Object(map);
    serde_json::to_vec(&v).map_err(|e| LicenseError::InvalidResponse(e.to_string()))
}

pub fn verify_signature(ent: &Entitlement, server_pubkey_b64: &str) -> Result<(), LicenseError> {
    let pub_bytes = B64
        .decode(server_pubkey_b64)
        .map_err(|e| LicenseError::Crypto(e.to_string()))?;
    if pub_bytes.len() != 32 {
        return Err(LicenseError::Crypto("pubkey len".into()));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&pub_bytes);
    let vk = VerifyingKey::from_bytes(&arr).map_err(|e| LicenseError::Crypto(e.to_string()))?;
    let sig_bytes = B64
        .decode(&ent.signature)
        .map_err(|e| LicenseError::InvalidResponse(e.to_string()))?;
    if sig_bytes.len() != 64 {
        return Err(LicenseError::InvalidResponse("bad sig len".into()));
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let sig = Signature::from_bytes(&sig_arr);
    let msg = canonical_bytes(ent)?;
    vk.verify_strict(&msg, &sig)
        .map_err(|_| LicenseError::InvalidResponse("signature invalid".into()))?;
    Ok(())
}

pub fn sign_entitlement(ent: &mut Entitlement, signing_key: &SigningKey) {
    let msg = canonical_bytes(ent).unwrap();
    let sig = signing_key.sign(&msg);
    ent.signature = B64.encode(sig.to_bytes());
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LicenseState {
    Inactive,
    Active,
    Expired,
    OfflineGrace,
    ActivationRequired,
    DeviceConflict,
}

pub struct LicenseManager {
    pub entitlement: Option<Entitlement>,
    pub device: DeviceIdentity,
    pub api_base: String,
    pub store: SecureStore,
    pub server_pubkey_b64: String,
    #[cfg(feature = "cloud-ai")]
    pub http: reqwest::Client,
}

impl LicenseManager {
    pub fn new(api_base: impl Into<String>, store: SecureStore, device: DeviceIdentity) -> Self {
        let api_base = api_base.into();
        #[cfg(feature = "cloud-ai")]
        let http = reqwest::Client::new();
        let server_pubkey_b64 = std::env::var("BROOMED_SERVER_PUBLIC_KEY_B64")
            .unwrap_or_else(|_| BROOMED_SERVER_PUBLIC_KEY_B64.to_string());
        let mut mgr = Self {
            entitlement: None,
            device,
            api_base,
            store,
            server_pubkey_b64,
            #[cfg(feature = "cloud-ai")]
            http,
        };
        // load cached entitlement
        if let Some(s) = mgr.store.load_token("entitlement") {
            if let Ok(ent) = serde_json::from_str::<Entitlement>(&s) {
                // verify signature if possible
                if verify_signature(&ent, &mgr.server_pubkey_b64).is_ok() {
                    mgr.entitlement = Some(ent);
                }
            }
        }
        mgr
    }

    pub fn check(&self, now: i64) -> LicenseState {
        match &self.entitlement {
            None => LicenseState::ActivationRequired,
            Some(ent) => {
                if ent.is_valid(now) {
                    LicenseState::Active
                } else if ent.is_within_grace(now, LICENSE_GRACE_SECS) {
                    LicenseState::OfflineGrace
                } else {
                    // distinguish device conflict if needed
                    LicenseState::Expired
                }
            }
        }
    }

    pub fn is_online_ai_enabled(&self, now: i64) -> bool {
        let ent = match &self.entitlement {
            Some(e) => e,
            None => return false,
        };
        if !ent.is_online_ai_entitled() {
            return false;
        }
        let valid = ent.is_valid(now) || ent.is_within_grace(now, LICENSE_GRACE_SECS);
        if !valid {
            return false;
        }
        if ent.subscription_status == "active" {
            return true;
        }
        if ent.subscription_status == "canceled" {
            if let Some(pe) = ent.period_end {
                return now < pe;
            }
        }
        false
    }

    pub fn clear(&mut self) {
        self.entitlement = None;
        let _ = self.store.delete_token("entitlement");
        let _ = self.store.delete_token("private_key");
    }

    pub fn cache_entitlement(&mut self, ent: Entitlement) {
        if let Ok(s) = serde_json::to_string(&ent) {
            let _ = self.store.store_token("entitlement", &s);
        }
        self.entitlement = Some(ent);
    }

    #[cfg(feature = "cloud-ai")]
    pub async fn activate(
        &mut self,
        mut activation_code: String,
        app_version: &str,
        platform: &str,
    ) -> Result<Entitlement, LicenseError> {
        let url = format!(
            "{}/api/license/activate",
            self.api_base.trim_end_matches('/')
        );
        let body = serde_json::json!({
            "activation_code": activation_code,
            "device_public_key": self.device.public_key_b64,
            "device_id": self.device.device_id,
            "app_version": app_version,
            "platform": platform
        });
        // never log activation_code; redact
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| LicenseError::Network(e.to_string()))?;
        // zeroize code after use
        activation_code.zeroize();
        drop(activation_code);
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let txt = resp.text().await.unwrap_or_default();
            // try parse error code
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                if let Some(code) = v
                    .get("error")
                    .and_then(|e| e.as_str())
                    .or_else(|| v.get("code").and_then(|c| c.as_str()))
                {
                    if let Some(le) = LicenseError::from_code(code) {
                        return Err(le);
                    }
                    // fallback mapping by status
                }
                if let Some(code) = v.get("error_code").and_then(|e| e.as_str()) {
                    if let Some(le) = LicenseError::from_code(code) {
                        return Err(le);
                    }
                }
            }
            // heuristics: map common strings
            let lower = txt.to_lowercase();
            if lower.contains("invalid") {
                return Err(LicenseError::InvalidActivationCode);
            }
            if lower.contains("expired") {
                return Err(LicenseError::ActivationExpired);
            }
            if lower.contains("already_used") || lower.contains("already used") {
                return Err(LicenseError::ActivationAlreadyUsed);
            }
            if lower.contains("device_already_bound") {
                return Err(LicenseError::DeviceAlreadyBound);
            }
            if lower.contains("device_deactivated") {
                return Err(LicenseError::DeviceDeactivated);
            }
            if status == 409 {
                return Err(LicenseError::DeviceAlreadyBound);
            }
            if status == 401 {
                return Err(LicenseError::InvalidActivationCode);
            }
            if status == 410 {
                return Err(LicenseError::ActivationExpired);
            }
            return Err(LicenseError::InvalidResponse(txt));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| LicenseError::InvalidResponse(e.to_string()))?;
        let ent_val = v.get("entitlement").unwrap_or(&v);
        let ent: Entitlement = serde_json::from_value(ent_val.clone())
            .map_err(|e| LicenseError::InvalidResponse(e.to_string()))?;
        verify_signature(&ent, &self.server_pubkey_b64)?;
        self.cache_entitlement(ent.clone());
        // ensure private key stored already; device key was stored at generation time by caller
        Ok(ent)
    }

    #[cfg(not(feature = "cloud-ai"))]
    pub async fn activate(
        &mut self,
        mut activation_code: String,
        _app_version: &str,
        _platform: &str,
    ) -> Result<Entitlement, LicenseError> {
        activation_code.zeroize();
        Err(LicenseError::ServerUnavailable)
    }

    #[cfg(feature = "cloud-ai")]
    pub async fn refresh(&mut self) -> Result<Entitlement, LicenseError> {
        let ent = match &self.entitlement {
            Some(e) => e.clone(),
            None => return Err(LicenseError::LicenseRefreshFailed),
        };
        let url = format!(
            "{}/api/license/refresh",
            self.api_base.trim_end_matches('/')
        );
        let token = ent.license_id.clone();
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({"device_id": self.device.device_id}))
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                // offline fallback: if within grace keep cached
                let now = chrono::Utc::now().timestamp();
                if ent.is_within_grace(now, LICENSE_GRACE_SECS) {
                    return Err(LicenseError::ServerUnavailable);
                }
                return Err(LicenseError::Network(e.to_string()));
            }
        };
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let txt = resp.text().await.unwrap_or_default();
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                if let Some(code) = v
                    .get("error")
                    .and_then(|e| e.as_str())
                    .or_else(|| v.get("code").and_then(|c| c.as_str()))
                {
                    if let Some(le) = LicenseError::from_code(code) {
                        if le == LicenseError::SubscriptionExpired
                            || le == LicenseError::SubscriptionInactive
                        {
                            return Err(le);
                        }
                    }
                }
            }
            if status >= 500 {
                let now = chrono::Utc::now().timestamp();
                if ent.is_within_grace(now, LICENSE_GRACE_SECS) {
                    return Err(LicenseError::ServerUnavailable);
                }
            }
            if txt.to_lowercase().contains("expired") {
                return Err(LicenseError::SubscriptionExpired);
            }
            return Err(LicenseError::LicenseRefreshFailed);
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| LicenseError::InvalidResponse(e.to_string()))?;
        let ent_val = v.get("entitlement").unwrap_or(&v);
        let new_ent: Entitlement = serde_json::from_value(ent_val.clone())
            .map_err(|e| LicenseError::InvalidResponse(e.to_string()))?;
        verify_signature(&new_ent, &self.server_pubkey_b64)?;
        self.cache_entitlement(new_ent.clone());
        Ok(new_ent)
    }

    #[cfg(not(feature = "cloud-ai"))]
    pub async fn refresh(&mut self) -> Result<Entitlement, LicenseError> {
        Err(LicenseError::ServerUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn test_keypair() -> (SigningKey, String) {
        let mut rng = OsRng;
        let sk = SigningKey::generate(&mut rng);
        let pk_b64 = B64.encode(sk.verifying_key().to_bytes());
        (sk, pk_b64)
    }

    fn make_ent(
        sk: &SigningKey,
        device_id: &str,
        sub: &str,
        ent: &str,
        issued: i64,
        expires: i64,
        period_end: Option<i64>,
    ) -> Entitlement {
        let mut e = Entitlement {
            subscription_status: sub.to_string(),
            entitlement: ent.to_string(),
            device_id: device_id.to_string(),
            issued_at: issued,
            expires_at: expires,
            license_id: "lic123".to_string(),
            server_version: "1.0".to_string(),
            period_end,
            signature: "".to_string(),
        };
        sign_entitlement(&mut e, sk);
        e
    }

    #[test]
    fn verify_roundtrip() {
        let (sk, pk) = test_keypair();
        let e = make_ent(&sk, "dev1", "active", "online_ai", 1000, 2000, None);
        assert!(verify_signature(&e, &pk).is_ok());
    }
    #[test]
    fn invalid_sig() {
        let (sk, pk) = test_keypair();
        let mut e = make_ent(&sk, "dev1", "active", "online_ai", 1000, 2000, None);
        e.signature = B64.encode(vec![0u8; 64]);
        assert!(matches!(
            verify_signature(&e, &pk),
            Err(LicenseError::InvalidResponse(_))
        ));
    }
    #[test]
    fn expiry_and_grace() {
        let (sk, _) = test_keypair();
        let e = make_ent(&sk, "d", "active", "online_ai", 0, 1000, None);
        assert!(e.is_valid(500));
        assert!(!e.is_valid(1001));
        assert!(e.needs_refresh(500)); // 1000-500=500 <86400 true
        assert!(e.needs_refresh(0)); // within 24h window
        let e2 = make_ent(&sk, "d", "active", "online_ai", 0, 200000, None);
        assert!(!e2.needs_refresh(0)); // far future not need refresh
        assert!(e.is_within_grace(1000 + 1000, LICENSE_GRACE_SECS));
        assert!(!e.is_within_grace(1000 + LICENSE_GRACE_SECS + 1, LICENSE_GRACE_SECS));
    }
    #[test]
    fn canceled_period() {
        let (sk, _) = test_keypair();
        let now = 1500;
        let e = make_ent(&sk, "d", "canceled", "online_ai", 0, 3000, Some(2000));
        assert!(e.is_valid(1500));
        assert!(!e.is_valid(2500));
        // manager online enabled only if now < period_end
        let store = SecureStore::memory();
        let (dev, _) = crate::device::DeviceIdentity::generate();
        let mut mgr = LicenseManager::new("http://x", store, dev);
        mgr.entitlement = Some(e.clone());
        assert!(mgr.is_online_ai_enabled(now));
        assert!(!mgr.is_online_ai_enabled(2500));
    }
    #[test]
    fn online_enabled_grace() {
        let (sk, _) = test_keypair();
        let e = make_ent(&sk, "d", "active", "online_ai", 0, 1000, None);
        let store = SecureStore::memory();
        let (dev, _) = crate::device::DeviceIdentity::generate();
        let mut mgr = LicenseManager::new("http://x", store, dev);
        mgr.entitlement = Some(e);
        assert!(!mgr.is_online_ai_enabled(1000 + LICENSE_GRACE_SECS + 10));
        assert!(mgr.is_online_ai_enabled(1000 + 1000));
    }
    #[test]
    fn check_states() {
        let (sk, _) = test_keypair();
        let store = SecureStore::memory();
        let (dev, _) = crate::device::DeviceIdentity::generate();
        let mut mgr = LicenseManager::new("http://x", store, dev);
        assert_eq!(mgr.check(0), LicenseState::ActivationRequired);
        let e = make_ent(
            &sk,
            &mgr.device.device_id,
            "active",
            "online_ai",
            0,
            9999999999,
            None,
        );
        mgr.entitlement = Some(e);
        assert_eq!(mgr.check(0), LicenseState::Active);
    }
    #[test]
    fn code_mapping() {
        assert_eq!(
            LicenseError::InvalidActivationCode.code(),
            "INVALID_ACTIVATION_CODE"
        );
        assert_eq!(
            LicenseError::from_code("DEVICE_ALREADY_BOUND").unwrap(),
            LicenseError::DeviceAlreadyBound
        );
    }
    // mock server tests for activate/refresh error mapping
    #[tokio::test]
    #[cfg(feature = "cloud-ai")]
    async fn activate_invalid_code_mock() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        // simple mock server
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let body = r#"{"error":"INVALID_ACTIVATION_CODE"}"#;
                let resp = format!("HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let store = SecureStore::memory();
        let (dev, sk) = crate::device::DeviceIdentity::generate();
        store.store_private_key(&sk.to_bytes()).unwrap();
        let mut mgr = LicenseManager::new(format!("http://{}", addr), store, dev);
        let res = mgr.activate("BADCODE".to_string(), "0.1.0", "linux").await;
        assert_eq!(res.unwrap_err(), LicenseError::InvalidActivationCode);
        // ensure not persisted
        assert!(mgr.entitlement.is_none());
        assert!(mgr.store.load_token("entitlement").is_none());
    }
    #[tokio::test]
    #[cfg(feature = "cloud-ai")]
    async fn refresh_server_unavailable_grace() {
        let (sk, _pk) = test_keypair();
        let now = chrono::Utc::now().timestamp();
        let e = make_ent(
            &sk,
            "devX",
            "active",
            "online_ai",
            now - 100,
            now + 100,
            None,
        );
        let store = SecureStore::memory();
        let (dev, _) = crate::device::DeviceIdentity::generate();
        let mut mgr = LicenseManager::new("http://127.0.0.1:9", store, dev);
        mgr.entitlement = Some(e.clone());
        // refresh to unreachable -> ServerUnavailable but keep cached
        let err = mgr.refresh().await.unwrap_err();
        assert_eq!(err, LicenseError::ServerUnavailable);
        assert!(mgr.entitlement.is_some());
    }

    #[tokio::test]
    #[cfg(feature = "cloud-ai")]
    async fn activation_code_never_persisted() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let (sk, pk) = test_keypair();
        let now = chrono::Utc::now().timestamp();
        let dev_id = "test-dev-123".to_string();
        let ent = make_ent(&sk, &dev_id, "active", "online_ai", now, now + 3600, None);
        let ent_json = serde_json::to_string(&ent).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body = format!(r#"{{"entitlement":{}}}"#, ent_json);
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let resp=format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}", body.len(), body);
                let _ = s.write_all(resp.as_bytes());
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let store = SecureStore::memory();
        let (mut dev, psk) = crate::device::DeviceIdentity::generate();
        dev.device_id = dev_id.clone();
        store.store_private_key(&psk.to_bytes()).unwrap();
        let mut mgr = LicenseManager::new(format!("http://{}", addr), store, dev);
        mgr.server_pubkey_b64 = pk;
        let code = "SECRET-ACTIVATION-CODE-999".to_string();
        let res = mgr.activate(code, "0.1.0", "linux").await.unwrap();
        assert_eq!(res.device_id, dev_id);
        // store must not contain raw code
        let maybe = mgr.store.load_token("SECRET-ACTIVATION-CODE-999");
        assert!(maybe.is_none());
        let ent_str = mgr.store.load_token("entitlement").unwrap();
        assert!(!ent_str.contains("SECRET-ACTIVATION-CODE-999"));
    }

    #[tokio::test]
    #[cfg(feature = "cloud-ai")]
    async fn activate_expired_and_reused_and_bound() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        for (code, err_str) in [
            ("EXPIRED", "ACTIVATION_EXPIRED"),
            ("REUSED", "ACTIVATION_ALREADY_USED"),
            ("BOUND", "DEVICE_ALREADY_BOUND"),
            ("DEACT", "DEVICE_DEACTIVATED"),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let body = format!(r#"{{"error":"{err_str}"}}"#);
            std::thread::spawn(move || {
                if let Ok((mut s, _)) = listener.accept() {
                    let mut buf = [0u8; 2048];
                    let _ = s.read(&mut buf);
                    let resp = format!(
                        "HTTP/1.1 409 Conflict\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = s.write_all(resp.as_bytes());
                }
            });
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            let store = SecureStore::memory();
            let (dev, sk) = crate::device::DeviceIdentity::generate();
            store.store_private_key(&sk.to_bytes()).unwrap();
            let mut mgr = LicenseManager::new(format!("http://{}", addr), store, dev);
            let err = mgr
                .activate(code.to_string(), "0.1.0", "linux")
                .await
                .unwrap_err();
            assert_eq!(err.code(), err_str);
        }
    }

    #[tokio::test]
    #[cfg(feature = "cloud-ai")]
    async fn refresh_success_and_malformed() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let (sk, pk) = test_keypair();
        let now = chrono::Utc::now().timestamp();
        // success
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let new_ent = make_ent(&sk, "devZ", "active", "online_ai", now, now + 7200, None);
        let ent_json = serde_json::to_string(&new_ent).unwrap();
        let body = format!(r#"{{"entitlement":{}}}"#, ent_json);
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let resp=format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}", body.len(), body);
                let _ = s.write_all(resp.as_bytes());
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        let store = SecureStore::memory();
        let (dev, _) = crate::device::DeviceIdentity::generate();
        let old = make_ent(
            &sk,
            "devZ",
            "active",
            "online_ai",
            now - 100,
            now + 100,
            None,
        );
        let mut mgr = LicenseManager::new(format!("http://{}", addr), store, dev);
        mgr.server_pubkey_b64 = pk.clone();
        mgr.entitlement = Some(old);
        let refreshed = mgr.refresh().await.unwrap();
        assert_eq!(refreshed.expires_at, now + 7200);
        // malformed
        let listener2 = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr2 = listener2.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener2.accept() {
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let body = "not json";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = s.write_all(resp.as_bytes());
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        let store2 = SecureStore::memory();
        let (dev2, _) = crate::device::DeviceIdentity::generate();
        let mut mgr2 = LicenseManager::new(format!("http://{}", addr2), store2, dev2);
        mgr2.server_pubkey_b64 = pk;
        let e2 = make_ent(&sk, "devY", "active", "online_ai", now, now + 3600, None);
        mgr2.entitlement = Some(e2);
        let err = mgr2.refresh().await.unwrap_err();
        assert_eq!(err.code(), "INVALID_RESPONSE");
    }

    #[test]
    fn subscription_expiration_states() {
        let (sk, _) = test_keypair();
        let now = 10000;
        let e_active = make_ent(&sk, "d", "active", "online_ai", 0, 20000, None);
        // expiry far beyond grace: expired at 5000, now 400000 ( > grace)
        let e_expired = make_ent(&sk, "d", "expired", "online_ai", 0, 5000, None);
        let e_inactive = make_ent(&sk, "d", "inactive", "none", 0, 20000, None);
        assert!(e_active.is_valid(now));
        assert!(!e_expired.is_valid(now));
        assert!(!e_inactive.is_valid(now));
        let store = SecureStore::memory();
        let (dev, _) = crate::device::DeviceIdentity::generate();
        let mut mgr = LicenseManager::new("http://x", store, dev);
        mgr.entitlement = Some(e_expired);
        assert!(!mgr.is_online_ai_enabled(now));
        // still within grace at now=10000 ( 5000+259200 >10000 ) => OfflineGrace
        assert_eq!(mgr.check(now), LicenseState::OfflineGrace);
        // after grace
        assert_eq!(
            mgr.check(5000 + LICENSE_GRACE_SECS + 10),
            LicenseState::Expired
        );
    }

    #[test]
    fn malformed_signature_invalid_response() {
        let (sk, pk) = test_keypair();
        let mut e = make_ent(&sk, "d", "active", "online_ai", 0, 999999, None);
        e.signature = "!!!notbase64!!!".to_string();
        assert!(matches!(
            verify_signature(&e, &pk),
            Err(LicenseError::InvalidResponse(_))
        ));
        e.signature = B64.encode(vec![1u8; 64]);
        assert!(matches!(
            verify_signature(&e, &pk),
            Err(LicenseError::InvalidResponse(_))
        ));
    }

    #[test]
    fn online_quota_and_unsupported_codes() {
        assert_eq!(
            LicenseError::OnlineAiQuotaExceeded.code(),
            "ONLINE_AI_QUOTA_EXCEEDED"
        );
        assert_eq!(
            LicenseError::UnsupportedAiCapability.code(),
            "UNSUPPORTED_AI_CAPABILITY"
        );
        assert_eq!(
            LicenseError::from_code("ONLINE_AI_QUOTA_EXCEEDED").unwrap(),
            LicenseError::OnlineAiQuotaExceeded
        );
    }
}
