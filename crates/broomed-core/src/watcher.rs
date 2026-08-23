use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub debounce_ms: u64,
    pub watched: Vec<PathBuf>,
    pub excluded: Vec<PathBuf>,
}

impl WatchConfig {
    pub fn should_ignore(&self, path: &Path) -> bool {
        // excluded wins: any prefix match -> ignore
        for excl in &self.excluded {
            if path == excl.as_path() || path.starts_with(excl) {
                return true;
            }
        }
        // if watched is non-empty, only allow paths under watched
        if !self.watched.is_empty() {
            for w in &self.watched {
                if path == w.as_path() || path.starts_with(w) {
                    return false;
                }
            }
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
}

impl FsEvent {
    pub fn path(&self) -> &Path {
        match self {
            FsEvent::Created(p) | FsEvent::Modified(p) | FsEvent::Deleted(p) => p,
        }
    }
}

/// A filesystem event tagged with a millisecond timestamp for time-window debouncing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedFsEvent {
    pub event: FsEvent,
    pub timestamp_ms: u64,
}

impl TimedFsEvent {
    pub fn new(event: FsEvent, timestamp_ms: u64) -> Self {
        Self {
            event,
            timestamp_ms,
        }
    }

    pub fn path(&self) -> &Path {
        self.event.path()
    }
}

/// Debounces filesystem events by collapsing multiple operations on the same path, keeping the latest state.
pub fn debounce_events(events: Vec<FsEvent>, _window_ms: u64) -> Vec<FsEvent> {
    let mut last: HashMap<PathBuf, (usize, FsEvent)> = HashMap::new();
    for (idx, ev) in events.into_iter().enumerate() {
        let key = ev.path().to_path_buf();
        last.insert(key, (idx, ev));
    }
    let mut ordered: Vec<(usize, FsEvent)> = last.into_values().collect();
    ordered.sort_by_key(|(idx, _)| *idx);
    ordered.into_iter().map(|(_, ev)| ev).collect()
}

/// Debounces timed filesystem events using a sliding time window (window_ms).
/// Events for the same path occurring within `window_ms` of each other are coalesced into the latest event.
pub fn debounce_timed_events(events: Vec<TimedFsEvent>, window_ms: u64) -> Vec<TimedFsEvent> {
    if events.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<TimedFsEvent> = Vec::new();
    for ev in events {
        let mut coalesced = false;
        // Check if there is an existing event for the same path within the time window
        for existing in result.iter_mut().rev() {
            if existing.path() == ev.path()
                && ev.timestamp_ms >= existing.timestamp_ms
                && (ev.timestamp_ms - existing.timestamp_ms) <= window_ms
            {
                existing.event = ev.event.clone();
                existing.timestamp_ms = ev.timestamp_ms;
                coalesced = true;
                break;
            }
        }
        if !coalesced {
            result.push(ev);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_ignore_excluded_prefix() {
        let cfg = WatchConfig {
            debounce_ms: 100,
            watched: vec![PathBuf::from("/tmp/watched")],
            excluded: vec![
                PathBuf::from("/tmp/watched/.git"),
                PathBuf::from("/tmp/watched/node_modules"),
            ],
        };
        assert!(cfg.should_ignore(Path::new("/tmp/watched/.git/config")));
        assert!(cfg.should_ignore(Path::new("/tmp/watched/node_modules/pkg/file.js")));
        assert!(cfg.should_ignore(Path::new("/tmp/watched/.git")));
        assert!(!cfg.should_ignore(Path::new("/tmp/watched/src/main.rs")));
    }

    #[test]
    fn should_ignore_watched_filter() {
        let cfg = WatchConfig {
            debounce_ms: 100,
            watched: vec![PathBuf::from("/a"), PathBuf::from("/b")],
            excluded: vec![],
        };
        assert!(!cfg.should_ignore(Path::new("/a/file.txt")));
        assert!(!cfg.should_ignore(Path::new("/b/sub/file.txt")));
        assert!(cfg.should_ignore(Path::new("/c/file.txt")));
    }

    #[test]
    fn should_ignore_empty_watched_allows_non_excluded() {
        let cfg = WatchConfig {
            debounce_ms: 50,
            watched: vec![],
            excluded: vec![PathBuf::from("/tmp/skip")],
        };
        assert!(!cfg.should_ignore(Path::new("/tmp/keep/file.txt")));
        assert!(cfg.should_ignore(Path::new("/tmp/skip/file.txt")));
    }

    #[test]
    fn should_ignore_exact_excluded() {
        let cfg = WatchConfig {
            debounce_ms: 50,
            watched: vec![],
            excluded: vec![PathBuf::from("/tmp/exact")],
        };
        assert!(cfg.should_ignore(Path::new("/tmp/exact")));
    }

    #[test]
    fn debounce_keeps_last_per_path() {
        let events = vec![
            FsEvent::Created(PathBuf::from("/a/file.txt")),
            FsEvent::Modified(PathBuf::from("/a/file.txt")),
            FsEvent::Created(PathBuf::from("/b/file.txt")),
            FsEvent::Deleted(PathBuf::from("/a/file.txt")),
        ];
        let out = debounce_events(events, 100);
        assert_eq!(out.len(), 2);
        assert!(out.contains(&FsEvent::Deleted(PathBuf::from("/a/file.txt"))));
        assert!(out.contains(&FsEvent::Created(PathBuf::from("/b/file.txt"))));
        assert!(!out.contains(&FsEvent::Created(PathBuf::from("/a/file.txt"))));
    }

    #[test]
    fn debounce_single_path_keeps_last_variant() {
        let events = vec![
            FsEvent::Created(PathBuf::from("/x")),
            FsEvent::Modified(PathBuf::from("/x")),
            FsEvent::Modified(PathBuf::from("/x")),
        ];
        let out = debounce_events(events, 50);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], FsEvent::Modified(PathBuf::from("/x")));
    }

    #[test]
    fn debounce_empty() {
        let out = debounce_events(vec![], 100);
        assert!(out.is_empty());
    }

    #[test]
    fn debounce_order_by_last_occurrence() {
        let events = vec![
            FsEvent::Created(PathBuf::from("/a")),
            FsEvent::Created(PathBuf::from("/b")),
            FsEvent::Modified(PathBuf::from("/a")),
        ];
        let out = debounce_events(events, 100);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], FsEvent::Created(PathBuf::from("/b")));
        assert_eq!(out[1], FsEvent::Modified(PathBuf::from("/a")));
    }

    #[test]
    fn debounce_timed_events_sliding_window() {
        let timed = vec![
            TimedFsEvent::new(FsEvent::Created(PathBuf::from("/doc.txt")), 1000),
            TimedFsEvent::new(FsEvent::Modified(PathBuf::from("/doc.txt")), 1050), // within 100ms -> coalesced
            TimedFsEvent::new(FsEvent::Modified(PathBuf::from("/doc.txt")), 1500), // >100ms later -> new event
        ];
        let out = debounce_timed_events(timed, 100);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].event, FsEvent::Modified(PathBuf::from("/doc.txt")));
        assert_eq!(out[0].timestamp_ms, 1050);
        assert_eq!(out[1].event, FsEvent::Modified(PathBuf::from("/doc.txt")));
        assert_eq!(out[1].timestamp_ms, 1500);
    }
}
