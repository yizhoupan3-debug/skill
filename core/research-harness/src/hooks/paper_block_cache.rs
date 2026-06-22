//! Shared mtime-based block cache for paper hooks (adversarial / prose).
//!
//! Both `paper_adversarial_hook` and `paper_prose_hook` use identical caching
//! logic: check file mtime → cache hit → read file → prefix validation →
//! fallback to builtin → cache store. This module eliminates that duplication.

use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

struct CachedBlock {
    content: String,
    mtime: Option<SystemTime>,
}

/// Per-hook cache instance. Each call site creates its own `BlockCache` with
/// its specific `rel_path`, `prefix_line`, and `builtin` function.
pub struct BlockCache {
    cache: Mutex<Option<CachedBlock>>,
    rel_path: &'static str,
    prefix_line: &'static str,
    log_label: &'static str,
}

impl BlockCache {
    pub const fn new(rel_path: &'static str, prefix_line: &'static str, log_label: &'static str) -> Self {
        Self {
            cache: Mutex::new(None),
            rel_path,
            prefix_line,
            log_label,
        }
    }

    /// Resolve the block content: disk file with prefix validation, falling back to builtin.
    ///
    /// Caching strategy: mtime-based — if the file's mtime hasn't changed since
    /// the last read, returns the cached content without re-reading.
    pub fn resolve(&self, repo_root: &Path, builtin: impl FnOnce() -> String) -> String {
        let path = repo_root.join(self.rel_path);
        let mtime = fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
        {
            let guard = self.cache.lock().unwrap_or_else(|e| {
                tracing::warn!("{} block cache poisoned, recovering", self.log_label);
                e.into_inner()
            });
            if let Some(ref cached) = *guard
                && cached.mtime == mtime {
                    return cached.content.clone();
                }
        }
        let content = match fs::read_to_string(&path) {
            Ok(t) => {
                let trimmed = t.trim();
                if trimmed.is_empty() {
                    builtin()
                } else if let Some(after) = trimmed.strip_prefix(self.prefix_line) {
                    let after = after.trim();
                    if after.is_empty() {
                        builtin()
                    } else {
                        trimmed.to_string()
                    }
                } else {
                    format!("{}\n\n{}", self.prefix_line, trimmed)
                }
            }
            Err(_) => builtin(),
        };
        {
            let mut guard = self.cache.lock().unwrap_or_else(|e| {
                tracing::warn!("{} block cache poisoned, recovering", self.log_label);
                e.into_inner()
            });
            *guard = Some(CachedBlock { content: content.clone(), mtime });
        }
        content
    }
}
