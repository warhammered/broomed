use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// --- FileId ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileId(pub Uuid);

impl FileId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for FileId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for FileId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<FileId> for Uuid {
    fn from(id: FileId) -> Self {
        id.0
    }
}

// --- OperationId ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationId(pub Uuid);

impl OperationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for OperationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for OperationId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<OperationId> for Uuid {
    fn from(id: OperationId) -> Self {
        id.0
    }
}

// --- ProviderId ---

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderId(pub String);

impl ProviderId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ProviderId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<ProviderId> for String {
    fn from(id: ProviderId) -> Self {
        id.0
    }
}

impl From<&str> for ProviderId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// --- DirectoryId ---

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DirectoryId(pub PathBuf);

impl DirectoryId {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn as_path(&self) -> &PathBuf {
        &self.0
    }
}

impl fmt::Display for DirectoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl From<PathBuf> for DirectoryId {
    fn from(p: PathBuf) -> Self {
        Self(p)
    }
}

impl From<DirectoryId> for PathBuf {
    fn from(id: DirectoryId) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_roundtrip() {
        // FileId serde roundtrip
        let fid = FileId::new();
        let json = serde_json::to_string(&fid).unwrap();
        let back: FileId = serde_json::from_str(&json).unwrap();
        assert_eq!(fid, back);
        assert_eq!(fid.to_string(), fid.as_uuid().to_string());

        // FileId From/Into
        let uuid = Uuid::new_v4();
        let fid2: FileId = uuid.into();
        let uuid2: Uuid = fid2.into();
        assert_eq!(uuid, uuid2);

        // OperationId serde roundtrip
        let oid = OperationId::new();
        let json = serde_json::to_string(&oid).unwrap();
        let back: OperationId = serde_json::from_str(&json).unwrap();
        assert_eq!(oid, back);

        // ProviderId
        let pid = ProviderId::new("openai");
        assert_eq!(pid.as_str(), "openai");
        assert_eq!(pid.to_string(), "openai");
        let json = serde_json::to_string(&pid).unwrap();
        let back: ProviderId = serde_json::from_str(&json).unwrap();
        assert_eq!(pid, back);
        let s: String = pid.into();
        assert_eq!(s, "openai");

        // DirectoryId
        let did = DirectoryId::new("/tmp/foo");
        assert_eq!(did.as_path(), &PathBuf::from("/tmp/foo"));
        let json = serde_json::to_string(&did).unwrap();
        let back: DirectoryId = serde_json::from_str(&json).unwrap();
        assert_eq!(did, back);
        let pb: PathBuf = did.into();
        assert_eq!(pb, PathBuf::from("/tmp/foo"));
    }
}
