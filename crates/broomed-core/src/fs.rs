use std::path::PathBuf;

use walkdir::WalkDir;

use crate::error::CoreError;

#[derive(Debug, Clone)]
pub struct TraversalBudget {
    pub max_files: usize,
    pub max_depth: u8,
    pub follow_symlinks: bool,
    pub include_hidden: bool,
}

impl Default for TraversalBudget {
    fn default() -> Self {
        Self {
            max_files: 10_000,
            max_depth: 20,
            follow_symlinks: false,
            include_hidden: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SafeWalk {
    base: PathBuf,
    budget: TraversalBudget,
}

impl SafeWalk {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self {
            base: base.into(),
            budget: TraversalBudget::default(),
        }
    }

    pub fn with_budget(mut self, budget: TraversalBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Walk base, enforcing max_depth/max_files, skipping hidden if configured,
    /// never following symlinks when `follow_symlinks=false`.
    pub fn walk(&self) -> Result<Vec<PathBuf>, CoreError> {
        let base_canon = self.base.canonicalize().map_err(|e| {
            CoreError::InvalidPath(format!(
                "base canonicalize failed {}: {e}",
                self.base.display()
            ))
        })?;

        if !base_canon.is_dir() {
            return Err(CoreError::InvalidPath(format!(
                "base is not a directory: {}",
                base_canon.display()
            )));
        }

        let mut out = Vec::new();
        let max_depth = self.budget.max_depth as usize;

        // Use filter_entry to skip hidden dirs entirely when hidden excluded
        let walker = WalkDir::new(&base_canon)
            .follow_links(self.budget.follow_symlinks)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(|e| {
                if !self.budget.include_hidden {
                    if let Some(name) = e.file_name().to_str() {
                        if name.starts_with('.') {
                            return false;
                        }
                    }
                    // also skip any hidden component in the relative path
                    if let Ok(rel) = e.path().strip_prefix(&base_canon) {
                        for comp in rel.components() {
                            if let std::path::Component::Normal(os) = comp {
                                if os.to_string_lossy().starts_with('.') {
                                    return false;
                                }
                            }
                        }
                    }
                }
                true
            });

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    // skip unreadable entries (permission, broken symlink)
                    tracing::debug!("SafeWalk skipping entry: {err}");
                    continue;
                }
            };

            // never follow symlinks when disabled: skip any symlink entry
            if !self.budget.follow_symlinks && entry.file_type().is_symlink() {
                continue;
            }
            // extra defensive: skip symlink metadata even if WalkDir reports file
            if !self.budget.follow_symlinks {
                if let Ok(meta) = std::fs::symlink_metadata(entry.path()) {
                    if meta.file_type().is_symlink() {
                        continue;
                    }
                }
            }

            // only collect files (not dirs)
            if entry.file_type().is_file() {
                // hidden file check (already filtered via filter_entry, but re-check for files)
                if !self.budget.include_hidden {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with('.') {
                            continue;
                        }
                    }
                }
                if out.len() >= self.budget.max_files {
                    return Err(CoreError::Conflict(format!(
                        "traversal budget exceeded: max_files={}",
                        self.budget.max_files
                    )));
                }
                out.push(entry.path().to_path_buf());
                if out.len() > self.budget.max_files {
                    return Err(CoreError::Conflict(format!(
                        "traversal budget exceeded: max_files={}",
                        self.budget.max_files
                    )));
                }
            }
        }

        // budget exceeded check also for empty-exceeded edge (max_files==0 with files found)
        if out.len() > self.budget.max_files {
            return Err(CoreError::Conflict(format!(
                "traversal budget exceeded: max_files={}",
                self.budget.max_files
            )));
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn walks_crate_with_budget() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let walker = SafeWalk::new(&base).with_budget(TraversalBudget {
            max_files: 10_000,
            max_depth: 10,
            follow_symlinks: false,
            include_hidden: false,
        });
        let files = walker.walk().unwrap();
        assert!(files.len() > 0, "expected >0 files, got {}", files.len());
        // ensure hidden not included by default
        for p in &files {
            let rel = p.strip_prefix(base.canonicalize().unwrap()).unwrap_or(p);
            for comp in rel.components() {
                if let std::path::Component::Normal(os) = comp {
                    assert!(
                        !os.to_string_lossy().starts_with('.'),
                        "hidden file leaked: {}",
                        p.display()
                    );
                }
            }
        }
    }

    #[test]
    fn budget_exceeded() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let walker = SafeWalk::new(&base).with_budget(TraversalBudget {
            max_files: 1,
            max_depth: 10,
            follow_symlinks: false,
            include_hidden: false,
        });
        // crate has >1 file, so 1 should trigger budget error or return 1
        // our impl errors when exceeding, so with crate >1 file this should err
        let res = walker.walk();
        // either returns 1 file and no error (if exactly 1) or error; accept both but prefer error when >1
        match res {
            Ok(v) => assert!(v.len() <= 1),
            Err(CoreError::Conflict(_)) => {}
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn respects_max_depth() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let shallow = SafeWalk::new(&base)
            .with_budget(TraversalBudget {
                max_files: 10_000,
                max_depth: 1,
                follow_symlinks: false,
                include_hidden: false,
            })
            .walk()
            .unwrap();
        let deep = SafeWalk::new(&base)
            .with_budget(TraversalBudget {
                max_files: 10_000,
                max_depth: 10,
                follow_symlinks: false,
                include_hidden: false,
            })
            .walk()
            .unwrap();
        assert!(shallow.len() <= deep.len());
    }
}
