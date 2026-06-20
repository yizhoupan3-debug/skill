//! Lazy-loaded `SKILL_ROUTING_RUNTIME.json` with `notify` + `tokio::sync::watch` hot-reload skeleton.
//!
//! Full record parsing stays in `router-rs` `route::records`; this module only tracks the raw JSON
//! snapshot so callers can invalidate caches when the file changes on disk.

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;
use tokio::sync::watch;

/// Raw on-disk routing table snapshot (path + JSON text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingTableSnapshot {
    pub path: PathBuf,
    pub raw_json: String,
}

/// Default repo-relative path when no override is supplied.
pub fn default_skill_routing_runtime_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json")
}

fn load_snapshot(path: &Path) -> Result<RoutingTableSnapshot, String> {
    let raw_json = std::fs::read_to_string(path)
        .map_err(|err| format!("failed reading {}: {err}", path.display()))?;
    Ok(RoutingTableSnapshot {
        path: path.to_path_buf(),
        raw_json,
    })
}

fn empty_snapshot(path: PathBuf) -> RoutingTableSnapshot {
    RoutingTableSnapshot {
        path,
        raw_json: String::new(),
    }
}

/// Hot-reload handle: subscribe via [`watch::Receiver`] or read [`current`](Self::current).
pub struct RoutingRuntimeWatch {
    rx: watch::Receiver<RoutingTableSnapshot>,
}

impl RoutingRuntimeWatch {
    /// Bootstrap from `path` (or [`default_skill_routing_runtime_path`] when `None`).
    pub fn bootstrap(path: Option<PathBuf>) -> Result<Self, String> {
        let path = path.unwrap_or_else(default_skill_routing_runtime_path);
        let initial = if path.is_file() {
            load_snapshot(&path)?
        } else {
            empty_snapshot(path.clone())
        };
        let (tx, rx) = watch::channel(initial);

        let watch_path = path.clone();
        thread::spawn(move || {
            let Ok(mut watcher) = RecommendedWatcher::new(
                move |res: Result<Event, notify::Error>| {
                    let Ok(event) = res else {
                        return;
                    };
                    let reload = matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Any
                    );
                    if reload
                        && let Ok(snapshot) = load_snapshot(&watch_path) {
                            let _ = tx.send(snapshot);
                        }
                },
                Config::default(),
            ) else {
                tracing::warn!("[routing-engine] notify watcher init failed");
                return;
            };
            if let Err(err) = watcher.watch(&path, RecursiveMode::NonRecursive) {
                tracing::warn!(
                    "[routing-engine] watch {} failed: {err}",
                    path.display()
                );
                return;
            }
            loop {
                thread::sleep(Duration::from_secs(3600));
            }
        });

        Ok(Self { rx })
    }

    pub fn receiver(&self) -> watch::Receiver<RoutingTableSnapshot> {
        self.rx.clone()
    }

    pub fn current(&self) -> RoutingTableSnapshot {
        self.rx.borrow().clone()
    }
}

static GLOBAL_WATCH: OnceLock<Arc<RoutingRuntimeWatch>> = OnceLock::new();

/// Process-wide lazy singleton (first call loads from disk and starts the watcher thread).
pub fn routing_runtime_watch() -> Arc<RoutingRuntimeWatch> {
    GLOBAL_WATCH
        .get_or_init(|| {
            Arc::new(RoutingRuntimeWatch::bootstrap(None).unwrap_or_else(|err| {
                tracing::warn!("[routing-engine] routing runtime watch bootstrap: {err}");
                let path = default_skill_routing_runtime_path();
                let (tx, rx) = watch::channel(empty_snapshot(path));
                drop(tx);
                RoutingRuntimeWatch { rx }
            }))
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_loads_existing_runtime_json() {
        let path = default_skill_routing_runtime_path();
        if !path.is_file() {
            return;
        }
        let watch = RoutingRuntimeWatch::bootstrap(Some(path.clone())).expect("bootstrap");
        let snap = watch.current();
        assert_eq!(snap.path, path);
        assert!(!snap.raw_json.is_empty());
    }

    #[test]
    fn bootstrap_missing_file_yields_empty_snapshot() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("SKILL_ROUTING_RUNTIME.json");
        let watch = RoutingRuntimeWatch::bootstrap(Some(path.clone())).expect("bootstrap");
        assert_eq!(watch.current().path, path);
        assert!(watch.current().raw_json.is_empty());
    }

    #[test]
    fn routing_runtime_watch_singleton_is_lazy() {
        let a = routing_runtime_watch();
        let b = routing_runtime_watch();
        assert!(Arc::ptr_eq(&a, &b));
    }
}
