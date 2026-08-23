use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::ai::{AiProvider, AiTask};
use crate::error::CoreError;
use crate::types::OperationId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ── Pipeline (Phase 2) ────────────────────────────────────────────

/// Preview of a planned move paired with AI result — serializable for IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPreview {
    pub operation: Operation,
    pub ai_result: crate::ai::AiResult,
}

/// Per-file loop, no batch wrapper — classify each file then batch plan_move
/// with confidence threshold. Keeps logic in core, IPC thin.
pub async fn plan_organize_with_provider<P: AiProvider>(
    files: Vec<String>,
    base: &Path,
    provider: &P,
    task: AiTask,
    threshold: f32,
) -> Result<Vec<PlanPreview>, CoreError> {
    let mut out = Vec::new();
    for file in files {
        let ai_result = provider.classify(task, &file).await?;
        if !ai_result.confidence.is_finite() || ai_result.confidence < threshold {
            continue;
        }
        let src = Path::new(&file);
        let filename = match src.file_name() {
            Some(n) => n,
            None => continue,
        };
        let folder = ai_result
            .suggested_folder
            .as_deref()
            .unwrap_or(ai_result.category.as_str());
        if folder.trim().is_empty() {
            continue;
        }
        let dst = base.join(folder).join(filename);
        match plan_move(src, &dst, base, ai_result.confidence) {
            Ok(mut op) => {
                op.reason = ai_result.reason.clone();
                op.confidence = ai_result.confidence;
                out.push(PlanPreview {
                    operation: op,
                    ai_result,
                });
            }
            Err(e) => {
                tracing::warn!("plan_organize skip {}: {}", file, e);
                continue;
            }
        }
    }
    Ok(out)
}

/// Convenience wrapper using bundled → heuristic provider selection.
pub async fn plan_organize(
    files: Vec<String>,
    base: &Path,
    task: AiTask,
    threshold: f32,
) -> Result<Vec<PlanPreview>, CoreError> {
    let bundled = crate::ai::BundledLocalProvider::new();
    if bundled.supports(&task) {
        return plan_organize_with_provider(files, base, &bundled, task, threshold).await;
    }
    let fallback = crate::ai::HeuristicFallback::new();
    plan_organize_with_provider(files, base, &fallback, task, threshold).await
}

/// Execute a batch of planned operations: record to journal then move.
/// Validates via journal (record before execute so undo can recover).
pub fn execute_plan(ops: &[Operation], journal: &Journal) -> Result<Vec<OperationId>, CoreError> {
    let mut ids = Vec::new();
    for op in ops {
        journal.record(op)?;
        execute(op)?;
        // mark executed
        journal.conn.execute(
            "UPDATE operations SET status = 'executed' WHERE id = ?1",
            rusqlite::params![op.id.to_string()],
        )?;
        ids.push(op.id);
    }
    Ok(ids)
}

/// Execute PlanPreviews variant (records + executes).
pub fn execute_previews(
    previews: &[PlanPreview],
    journal: &Journal,
) -> Result<Vec<OperationId>, CoreError> {
    let ops: Vec<Operation> = previews.iter().map(|p| p.operation.clone()).collect();
    execute_plan(&ops, journal)
}

/// Default journal path for Tauri when no explicit db_path supplied.
pub fn default_journal_path() -> PathBuf {
    std::env::temp_dir().join("broomed_journal.db")
}

/// Open (or create) a journal at the given path, ensuring schema.
pub fn open_journal(path: &Path) -> Result<Journal, CoreError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::Io(e.to_string()))?;
        }
    }
    let conn = rusqlite::Connection::open(path)?;
    crate::db::create_schema(&conn)?;
    Ok(Journal::new(conn))
}

/// Open default temp journal.
pub fn open_default_journal() -> Result<Journal, CoreError> {
    open_journal(&default_journal_path())
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

    /// Return ids of latest N non-undone operations (rowid desc).
    pub fn latest_ids(&self, n: usize) -> Result<Vec<OperationId>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM operations WHERE status != 'undone' ORDER BY rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![n as i64], |r| {
            let s: String = r.get(0)?;
            Ok(s)
        })?;
        let mut out = Vec::new();
        for r in rows {
            let s = r.map_err(|e| CoreError::Internal(e.to_string()))?;
            let uuid = s
                .parse::<uuid::Uuid>()
                .map_err(|e| CoreError::Internal(e.to_string()))?;
            out.push(OperationId::from(uuid));
        }
        Ok(out)
    }

    /// Undo last N operations (LIFO). Returns undone ids.
    pub fn undo_last(&self, n: usize) -> Result<Vec<OperationId>, CoreError> {
        let ids = self.latest_ids(n)?;
        let mut undone = Vec::new();
        for id in ids {
            self.undo(&id)?;
            undone.push(id);
        }
        Ok(undone)
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

    #[tokio::test]
    async fn plan_organize_threshold_filters() {
        let dir = temp_base("op_plan_thresh");
        let f1 = dir.join("photo.jpg");
        let f2 = dir.join("doc.pdf");
        fs::write(&f1, b"a").unwrap();
        fs::write(&f2, b"b").unwrap();
        let files = vec![
            f1.to_string_lossy().to_string(),
            f2.to_string_lossy().to_string(),
        ];
        // low threshold includes both (heuristic confidences ~0.82-0.85)
        let previews = plan_organize(files.clone(), &dir, crate::ai::AiTask::ClassifyFile, 0.5)
            .await
            .unwrap();
        assert_eq!(previews.len(), 2);
        // high threshold 0.90 excludes both (max is 0.86 + 0.05 bundled bump)
        let previews2 = plan_organize(files, &dir, crate::ai::AiTask::ClassifyFile, 0.90)
            .await
            .unwrap();
        assert_eq!(previews2.len(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn pipeline_execute_and_undo_last() {
        let dir = temp_base("op_pipeline");
        let f = dir.join("song.mp3");
        fs::write(&f, b"audio").unwrap();
        let files = vec![f.to_string_lossy().to_string()];
        let previews = plan_organize(files, &dir, crate::ai::AiTask::ClassifyFile, 0.5)
            .await
            .unwrap();
        assert_eq!(previews.len(), 1);
        assert!(previews[0].ai_result.confidence > 0.3);
        assert_eq!(previews[0].ai_result.category, "Audio");
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        let journal = Journal::new(conn);
        let ids = execute_previews(&previews, &journal).unwrap();
        assert_eq!(ids.len(), 1);
        let dst = &previews[0].operation.destination;
        assert!(dst.exists(), "dst should exist after execute");
        assert!(!f.exists(), "src should be gone");
        // undo via journal
        let undone = journal.undo_last(1).unwrap();
        assert_eq!(undone.len(), 1);
        assert!(f.exists(), "src restored after undo");
        assert!(!dst.exists(), "dst gone after undo");
        let _ = fs::remove_dir_all(&dir);
    }
}
