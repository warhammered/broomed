use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;

use crate::error::CoreError;
use crate::types::OperationId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpKind {
    Move,
    Copy,
    Rename,
    Trash,
}

impl OpKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Copy => "copy",
            Self::Rename => "rename",
            Self::Trash => "trash",
        }
    }

    // ponytail: keep from_str name (callers use OpKind::from_str) — allow clippy lint to avoid trait churn
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "move" => Some(Self::Move),
            "copy" => Some(Self::Copy),
            "rename" => Some(Self::Rename),
            "trash" => Some(Self::Trash),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub id: OperationId,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub kind: OpKind,
    pub reason: String,
    pub confidence: f32,
    pub reversible: bool,
    pub status: String,
}

/// Validate src/dst under base, reject if dst exists, never overwrites.
pub fn plan_move(
    src: &Path,
    dst: &Path,
    base: &Path,
    confidence: f32,
) -> Result<Operation, CoreError> {
    let src_v = crate::security::validate_path(src, base)?;
    let dst_v = crate::security::validate_path(dst, base)?;

    if !src_v.exists() {
        return Err(CoreError::NotFound(format!(
            "source not found: {}",
            src_v.display()
        )));
    }
    if dst_v.exists() {
        return Err(CoreError::Conflict(format!(
            "destination exists: {}",
            dst_v.display()
        )));
    }

    Ok(Operation {
        id: OperationId::new(),
        source: src_v,
        destination: dst_v,
        kind: OpKind::Move,
        reason: String::new(),
        confidence,
        reversible: true,
        status: "planned".to_string(),
    })
}

/// Create parent dirs, fs::rename with cross-device fallback to copy+verify+remove,
/// verify dst exists + hash matches if file.
pub fn execute(op: &Operation) -> Result<(), CoreError> {
    if op.source == op.destination {
        return Err(CoreError::Conflict("source == destination".into()));
    }
    if op.destination.exists() {
        return Err(CoreError::Conflict(format!(
            "destination exists: {}",
            op.destination.display()
        )));
    }
    if !op.source.exists() {
        return Err(CoreError::NotFound(format!(
            "source not found: {}",
            op.source.display()
        )));
    }

    if let Some(parent) = op.destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::Io(e.to_string()))?;
    }

    let is_file = op.source.is_file();
    let pre_hash = if is_file {
        Some(crate::hash::hash_file(&op.source)?)
    } else {
        None
    };

    match std::fs::rename(&op.source, &op.destination) {
        Ok(()) => {
            verify_destination(&op.destination, pre_hash)?;
            Ok(())
        }
        Err(_) => {
            // cross-device fallback: copy + verify + remove
            copy_recursively(&op.source, &op.destination)?;
            if !op.destination.exists() {
                return Err(CoreError::Io("copy failed: destination missing".into()));
            }
            if let Some(expected) = pre_hash {
                let dst_hash = crate::hash::hash_file(&op.destination)?;
                if dst_hash != expected {
                    return Err(CoreError::Io("hash mismatch after copy".into()));
                }
            } else if op.source.is_dir() {
                // for dirs, ensure dst is dir
                if !op.destination.is_dir() {
                    return Err(CoreError::Io("copy failed: destination not dir".into()));
                }
            }
            // remove source after verified
            if op.source.is_dir() {
                std::fs::remove_dir_all(&op.source).map_err(|e| CoreError::Io(e.to_string()))?;
            } else {
                std::fs::remove_file(&op.source).map_err(|e| CoreError::Io(e.to_string()))?;
            }
            Ok(())
        }
    }
}

fn verify_destination(dst: &Path, pre_hash: Option<String>) -> Result<(), CoreError> {
    if !dst.exists() {
        return Err(CoreError::Io("rename failed: destination missing".into()));
    }
    if let Some(expected) = pre_hash {
        // only verify if file
        if dst.is_file() {
            let dst_hash = crate::hash::hash_file(dst)?;
            if dst_hash != expected {
                return Err(CoreError::Io("hash mismatch after rename".into()));
            }
        }
    }
    Ok(())
}

fn copy_recursively(src: &Path, dst: &Path) -> Result<(), CoreError> {
    if src.is_file() {
        std::fs::copy(src, dst).map_err(|e| CoreError::Io(e.to_string()))?;
    } else if src.is_dir() {
        std::fs::create_dir_all(dst).map_err(|e| CoreError::Io(e.to_string()))?;
        for entry in walkdir::WalkDir::new(src) {
            let entry = entry.map_err(|e| CoreError::Io(e.to_string()))?;
            let rel = entry.path().strip_prefix(src).unwrap();
            if rel.as_os_str().is_empty() {
                continue;
            }
            let target = dst.join(rel);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&target).map_err(|e| CoreError::Io(e.to_string()))?;
            } else if entry.file_type().is_file() {
                if let Some(p) = target.parent() {
                    std::fs::create_dir_all(p).map_err(|e| CoreError::Io(e.to_string()))?;
                }
                std::fs::copy(entry.path(), &target).map_err(|e| CoreError::Io(e.to_string()))?;
            }
        }
    } else {
        return Err(CoreError::NotFound(format!(
            "source not found: {}",
            src.display()
        )));
    }
    Ok(())
}

/// Journal wrapping rusqlite connection.
pub struct Journal {
    conn: rusqlite::Connection,
}

impl Journal {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }

    /// Insert operation into `operations` table.
    pub fn record(&self, op: &Operation) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO operations (id, source, destination, operation_type, reason, confidence, reversible, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                op.id.to_string(),
                op.source.to_string_lossy().to_string(),
                op.destination.to_string_lossy().to_string(),
                op.kind.as_str(),
                op.reason,
                op.confidence as f64,
                if op.reversible { 1 } else { 0 },
                op.status,
            ],
        )?;
        Ok(())
    }

    /// Undo by swapping src/dst: rename destination back to source.
    pub fn undo(&self, id: &OperationId) -> Result<(), CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT source, destination, reversible FROM operations WHERE id = ?1")?;
        let row: Option<(String, String, i32)> = stmt
            .query_row(rusqlite::params![id.to_string()], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?;

        let (src_s, dst_s, reversible) = match row {
            Some(v) => v,
            None => return Err(CoreError::NotFound(format!("operation not found: {id}"))),
        };

        if reversible == 0 {
            return Err(CoreError::Conflict("operation not reversible".into()));
        }

        let src = PathBuf::from(src_s);
        let dst = PathBuf::from(dst_s);

        // undo: move dst -> src
        if !dst.exists() {
            return Err(CoreError::NotFound(format!(
                "undo source not found (operation destination): {}",
                dst.display()
            )));
        }
        if src.exists() {
            return Err(CoreError::Conflict(format!(
                "undo destination exists: {}",
                src.display()
            )));
        }
        if let Some(parent) = src.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::Io(e.to_string()))?;
        }

        let is_file = dst.is_file();
        let pre_hash = if is_file {
            Some(crate::hash::hash_file(&dst)?)
        } else {
            None
        };

        match std::fs::rename(&dst, &src) {
            Ok(()) => {
                verify_destination(&src, pre_hash)?;
            }
            Err(_) => {
                copy_recursively(&dst, &src)?;
                if !src.exists() {
                    return Err(CoreError::Io(
                        "undo copy failed: destination missing".into(),
                    ));
                }
                if let Some(expected) = pre_hash {
                    let got = crate::hash::hash_file(&src)?;
                    if got != expected {
                        return Err(CoreError::Io("hash mismatch after undo copy".into()));
                    }
                }
                if dst.is_dir() {
                    std::fs::remove_dir_all(&dst).map_err(|e| CoreError::Io(e.to_string()))?;
                } else {
                    std::fs::remove_file(&dst).map_err(|e| CoreError::Io(e.to_string()))?;
                }
            }
        }

        // mark undone
        self.conn.execute(
            "UPDATE operations SET status = 'undone' WHERE id = ?1",
            rusqlite::params![id.to_string()],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_schema;
    use rusqlite::Connection;
    use std::fs;

    fn temp_base(name: &str) -> PathBuf {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let dir = base.join(format!("broomed_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    #[test]
    fn plan_move_rejects_existing_dst() {
        let dir = temp_base("op_reject");
        let src = dir.join("src.txt");
        let dst = dir.join("dst.txt");
        fs::write(&src, b"src").unwrap();
        fs::write(&dst, b"dst").unwrap();
        let err = plan_move(&src, &dst, &dir, 0.9).unwrap_err();
        assert!(matches!(err, CoreError::Conflict(_)), "got {err:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn execute_creates_file() {
        let dir = temp_base("op_exec");
        let src = dir.join("a.txt");
        let dst = dir.join("sub/b.txt");
        fs::write(&src, b"content").unwrap();
        let op = plan_move(&src, &dst, &dir, 0.9).unwrap();
        execute(&op).unwrap();
        assert!(dst.exists(), "dst missing");
        assert!(!src.exists(), "src should be gone");
        assert_eq!(fs::read_to_string(&dst).unwrap(), "content");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn journal_record_and_undo() {
        let dir = temp_base("op_journal");
        let src = dir.join("orig.txt");
        let dst = dir.join("moved.txt");
        fs::write(&src, b"hello").unwrap();
        let op = plan_move(&src, &dst, &dir, 1.0).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        let journal = Journal::new(conn);
        journal.record(&op).unwrap();
        execute(&op).unwrap();
        assert!(dst.exists());
        assert!(!src.exists());
        journal.undo(&op.id).unwrap();
        assert!(src.exists(), "src should be restored");
        assert!(!dst.exists(), "dst should be gone after undo");
        assert_eq!(fs::read_to_string(&src).unwrap(), "hello");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_move_validates_base_escape() {
        let dir = temp_base("op_escape");
        let src = dir.join("ok.txt");
        fs::write(&src, b"x").unwrap();
        // absolute path outside base
        let outside = if cfg!(windows) {
            PathBuf::from("C:\\Windows\\System32\\evil.txt")
        } else {
            PathBuf::from("/etc/passwd_evil")
        };
        // if outside is indeed outside, should error
        if !outside.starts_with(&dir) {
            let err = plan_move(&src, &outside, &dir, 0.5).unwrap_err();
            assert!(matches!(
                err,
                CoreError::InvalidPath(_) | CoreError::Conflict(_)
            ));
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
