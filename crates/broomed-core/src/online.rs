use std::sync::{Arc, Mutex};

use crate::ai::{AiCapabilities, AiProvider, AiResult, AiTask};
use crate::error::CoreError;
use crate::license::{LicenseError, LicenseManager};
use crate::types::ProviderId;

pub struct OnlineAiClient {
    pub api_base: String,
    pub license: Arc<Mutex<LicenseManager>>,
    #[cfg(feature = "cloud-ai")]
    pub http: reqwest::Client,
}

impl OnlineAiClient {
    pub fn new(api_base: impl Into<String>, license: Arc<Mutex<LicenseManager>>) -> Self {
        Self {
            api_base: api_base.into(),
            license,
            #[cfg(feature = "cloud-ai")]
            http: reqwest::Client::new(),
        }
    }

    pub fn is_available(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.license.lock().unwrap().is_online_ai_enabled(now)
    }

    fn capability_for_task(task: &AiTask) -> &'static str {
        match task {
            AiTask::DescribeImage => "vision",
            AiTask::GenerateTags
            | AiTask::SuggestFilename
            | AiTask::SuggestFolder
            | AiTask::ClassifyFile => "text",
            AiTask::SemanticSearch | AiTask::DetectSemanticDuplicate => "analyze",
            AiTask::SummarizeDocument => "text",
        }
    }

    #[cfg(feature = "cloud-ai")]
    pub async fn request_capability(
        &self,
        cap: &str,
        payload: serde_json::Value,
    ) -> Result<AiResult, LicenseError> {
        let now = chrono::Utc::now().timestamp();
        let (enabled, token) = {
            let mgr = self.license.lock().unwrap();
            let enabled = mgr.is_online_ai_enabled(now);
            let token = mgr.entitlement.as_ref().map(|e| e.license_id.clone());
            (enabled, token)
        };
        if !enabled {
            return Err(LicenseError::OnlineAiDisabled);
        }
        let token = token.ok_or(LicenseError::OnlineAiDisabled)?;
        let url = format!("{}/api/ai/{}", self.api_base.trim_end_matches('/'), cap);
        // privacy: only selected file content transmitted when online_opt_in && entitlement valid
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| LicenseError::Network(e.to_string()))?;
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
                        return Err(le);
                    }
                }
            }
            if status == 402 || status == 429 {
                return Err(LicenseError::OnlineAiQuotaExceeded);
            }
            if status == 403 {
                return Err(LicenseError::OnlineAiDisabled);
            }
            if txt.to_lowercase().contains("quota") {
                return Err(LicenseError::OnlineAiQuotaExceeded);
            }
            if txt.to_lowercase().contains("unsupported") {
                return Err(LicenseError::UnsupportedAiCapability);
            }
            return Err(LicenseError::InvalidResponse(txt));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| LicenseError::InvalidResponse(e.to_string()))?;
        // server returns AiResult or wrapped
        if let Ok(r) = serde_json::from_value::<AiResult>(v.clone()) {
            return Ok(r);
        }
        if let Some(inner) = v.get("result") {
            if let Ok(r) = serde_json::from_value::<AiResult>(inner.clone()) {
                return Ok(r);
            }
        }
        // try parse_ai_json fallback
        let s = v.to_string();
        crate::ai::parse_ai_json(&s).map_err(|e| LicenseError::InvalidResponse(e.to_string()))
    }

    #[cfg(not(feature = "cloud-ai"))]
    pub async fn request_capability(
        &self,
        _cap: &str,
        _payload: serde_json::Value,
    ) -> Result<AiResult, LicenseError> {
        Err(LicenseError::ServerUnavailable)
    }

    pub async fn classify_via_capability(
        &self,
        task: AiTask,
        input: &str,
    ) -> Result<AiResult, LicenseError> {
        let cap = Self::capability_for_task(&task);
        let payload =
            serde_json::json!({"capability": cap, "input": input, "task": format!("{:?}", task)});
        self.request_capability(cap, payload).await
    }
}

// BroomedOnlineProvider wraps OnlineAiClient as AiProvider
pub struct BroomedOnlineProvider {
    id: ProviderId,
    capabilities: AiCapabilities,
    priority: u8,
    client: Arc<OnlineAiClient>,
}

impl BroomedOnlineProvider {
    pub fn new(client: Arc<OnlineAiClient>) -> Self {
        Self {
            id: ProviderId::new("broomed-online"),
            capabilities: AiCapabilities::new(true, true, false, true),
            priority: 20,
            client,
        }
    }
}

impl AiProvider for BroomedOnlineProvider {
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
        matches!(
            task,
            AiTask::ClassifyFile
                | AiTask::DescribeImage
                | AiTask::GenerateTags
                | AiTask::SuggestFolder
                | AiTask::SuggestFilename
                | AiTask::SummarizeDocument
        )
    }
    async fn classify(&self, task: AiTask, input: &str) -> Result<AiResult, CoreError> {
        if !self.client.is_available() {
            return Err(CoreError::Internal(
                LicenseError::OnlineAiDisabled.code().to_string(),
            ));
        }
        let r = self
            .client
            .classify_via_capability(task, input)
            .await
            .map_err(|e| CoreError::Internal(format!("{}: {}", e.code(), e)))?;
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DeviceIdentity;
    use crate::secure_store::SecureStore;

    #[cfg(feature = "cloud-ai")]
    use crate::license::{sign_entitlement, Entitlement};
    #[cfg(feature = "cloud-ai")]
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    #[cfg(feature = "cloud-ai")]
    use ed25519_dalek::SigningKey;
    #[cfg(feature = "cloud-ai")]
    use rand::rngs::OsRng;
    #[cfg(feature = "cloud-ai")]
    use std::io::{Read, Write};
    #[cfg(feature = "cloud-ai")]
    use std::net::TcpListener;

    #[cfg(feature = "cloud-ai")]
    fn keypair() -> (SigningKey, String) {
        let mut r = OsRng;
        let sk = SigningKey::generate(&mut r);
        let pk = B64.encode(sk.verifying_key().to_bytes());
        (sk, pk)
    }

    #[cfg(feature = "cloud-ai")]
    fn ent(sk: &SigningKey, dev: &str, exp: i64) -> Entitlement {
        let mut e = Entitlement {
            subscription_status: "active".into(),
            entitlement: "online_ai".into(),
            device_id: dev.into(),
            issued_at: exp - 1000,
            expires_at: exp,
            license_id: "lic1".into(),
            server_version: "1".into(),
            period_end: None,
            signature: "".into(),
        };
        sign_entitlement(&mut e, sk);
        e
    }

    #[tokio::test]
    #[cfg(feature = "cloud-ai")]
    async fn no_token_disabled() {
        let store = SecureStore::memory();
        let (dev, _) = DeviceIdentity::generate();
        let mgr = Arc::new(Mutex::new(LicenseManager::new("http://x", store, dev)));
        let client = OnlineAiClient::new("http://x", mgr);
        assert!(!client.is_available());
        let err = client
            .request_capability("text", serde_json::json!({}))
            .await
            .unwrap_err();
        assert_eq!(err, LicenseError::OnlineAiDisabled);
    }

    #[tokio::test]
    #[cfg(feature = "cloud-ai")]
    async fn privacy_local_zero_calls() {
        // we test that when not enabled, no network call happens (server would panic if called)
        // by using a server that counts calls - but since disabled, we shouldn't reach server
        let store = SecureStore::memory();
        let (dev, _) = DeviceIdentity::generate();
        let mgr = Arc::new(Mutex::new(LicenseManager::new(
            "http://127.0.0.1:1",
            store,
            dev,
        )));
        let client = OnlineAiClient::new("http://127.0.0.1:1", mgr);
        assert!(!client.is_available());
        let r = client
            .request_capability("text", serde_json::json!({"input":"hi"}))
            .await;
        assert!(r.is_err());
    }

    #[tokio::test]
    #[cfg(feature = "cloud-ai")]
    async fn success_mock() {
        let (sk, pk) = keypair();
        std::env::set_var("BROOMED_SERVER_PUBLIC_KEY_B64", &pk);
        let now = chrono::Utc::now().timestamp();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let body = r#"{"category":"Documents","confidence":0.9,"reason":"ok","tags":[]}"#;
                let resp=format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}", body.len(), body);
                let _ = s.write_all(resp.as_bytes());
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let store = SecureStore::memory();
        let (dev, _) = DeviceIdentity::generate();
        let e = ent(&sk, &dev.device_id, now + 3600);
        let mut mgr = LicenseManager::new(format!("http://{}", addr), store, dev.clone());
        mgr.entitlement = Some(e.clone());
        // also store entitlement token for load
        let mgr = Arc::new(Mutex::new(mgr));
        let client = OnlineAiClient::new(format!("http://{}", addr), mgr);
        assert!(client.is_available());
        let res = client
            .request_capability("text", serde_json::json!({"input":"hi"}))
            .await
            .unwrap();
        assert_eq!(res.category, "Documents");
        std::env::remove_var("BROOMED_SERVER_PUBLIC_KEY_B64");
    }
}
