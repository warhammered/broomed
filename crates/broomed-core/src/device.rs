use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub public_key_b64: String,
    pub platform: String,
    pub app_version: String,
    pub created_at: u64,
}

impl DeviceIdentity {
    pub fn generate() -> (Self, SigningKey) {
        let mut csprng = OsRng;
        let signing = SigningKey::generate(&mut csprng);
        let verifying = signing.verifying_key();
        let pk_b64 = B64.encode(verifying.to_bytes());
        let id = Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let dev = Self {
            device_id: id,
            public_key_b64: pk_b64,
            platform: std::env::consts::OS.to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: now,
        };
        (dev, signing)
    }

    /// Generate with explicit app_version/platform for tests
    pub fn generate_with(app_version: &str, platform: &str) -> (Self, SigningKey) {
        let (mut d, k) = Self::generate();
        d.app_version = app_version.to_string();
        d.platform = platform.to_string();
        (d, k)
    }

    pub fn public_key_bytes(&self) -> Result<[u8; 32], String> {
        let b = B64.decode(&self.public_key_b64).map_err(|e| e.to_string())?;
        if b.len() != 32 {
            return Err(format!("pubkey len {}", b.len()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&b);
        Ok(arr)
    }

    pub fn verifying_key(&self) -> Result<VerifyingKey, String> {
        let b = self.public_key_bytes()?;
        VerifyingKey::from_bytes(&b).map_err(|e| e.to_string())
    }
}

pub fn device_fingerprint(pubkey_b64: &str) -> String {
    let bytes = B64.decode(pubkey_b64).unwrap_or_default();
    let h = blake3::hash(&bytes);
    h.to_hex().to_string()[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generate_roundtrip() {
        let (d, sk) = DeviceIdentity::generate();
        assert!(!d.device_id.is_empty());
        assert_eq!(d.public_key_b64.len(), 44); // 32 bytes b64
        let vk = d.verifying_key().unwrap();
        assert_eq!(vk.to_bytes(), sk.verifying_key().to_bytes());
    }
    #[test]
    fn fingerprint_len() {
        let (d, _) = DeviceIdentity::generate();
        assert_eq!(device_fingerprint(&d.public_key_b64).len(), 16);
    }
}
