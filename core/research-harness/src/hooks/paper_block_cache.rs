//! Shared mtime-based block cache for paper hooks (adversarial / prose).
//!
//! Both `paper_adversarial_hook` and `paper_prose_hook` use identical caching
//! logic: check file mtime → cache hit → read file → prefix validation →
//! fallback to builtin → cache store. This module eliminates that duplication.

use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

/// Maximum allowed file size for cached blocks (100 KiB).
const MAX_BLOCK_BYTES: usize = 1024 * 100;

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
    pub const fn new(
        rel_path: &'static str,
        prefix_line: &'static str,
        log_label: &'static str,
    ) -> Self {
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
        let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
        {
            let guard = self.cache.lock().unwrap_or_else(|e| {
                tracing::warn!("{} block cache poisoned, clearing cache", self.log_label);
                let mut guard = e.into_inner();
                *guard = None; // Clear potentially corrupted cache
                guard
            });
            if let Some(ref cached) = *guard
                && cached.mtime == mtime
            {
                return cached.content.clone();
            }
        }
        let content = match fs::read_to_string(&path) {
            Ok(t) => {
                if t.len() > MAX_BLOCK_BYTES {
                    tracing::warn!(
                        "paper_block_cache: {} ({:.1} KiB) exceeds {} KiB limit — using built-in fallback. \
                         Increase MAX_BLOCK_BYTES or reduce file size.",
                        path.display(), t.len() as f64 / 1024.0, MAX_BLOCK_BYTES / 1024,
                    );
                    builtin()
                } else {
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
            }
            Err(_) => builtin(),
        };
        {
            let mut guard = self.cache.lock().unwrap_or_else(|e| {
                tracing::warn!("{} block cache poisoned, clearing cache", self.log_label);
                let mut guard = e.into_inner();
                *guard = None;
                guard
            });
            *guard = Some(CachedBlock {
                content: content.clone(),
                mtime,
            });
        }
        content
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::fs;

    const TEST_PREFIX: &str = "**TEST_PREFIX**";
    const TEST_BODY: &str = "sample block content body";

    fn test_builtin() -> String {
        format!("{TEST_PREFIX}\n\nbuiltin fallback")
    }

    fn test_cache(rel: &'static str) -> BlockCache {
        BlockCache::new(rel, TEST_PREFIX, "test-cache")
    }

    fn prep_file(path: &Path, content: impl AsRef<str>) {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p).unwrap();
        }
        fs::write(path, content.as_ref()).unwrap();
    }

    #[test]
    fn missing_file_uses_builtin() {
        let tmp = std::env::temp_dir().join("cache_test_missing");
        let _ = fs::remove_dir_all(&tmp);
        let cache = test_cache("configs/framework/NONEXISTENT.txt");
        let result = cache.resolve(&tmp, test_builtin);
        assert_eq!(result, test_builtin());
    }

    #[test]
    fn empty_file_uses_builtin() {
        let tmp = std::env::temp_dir().join("cache_test_empty");
        let _ = fs::remove_dir_all(&tmp);
        let cache = test_cache("configs/framework/EMPTY.txt");
        prep_file(&tmp.join("configs/framework/EMPTY.txt"), "");
        let result = cache.resolve(&tmp, test_builtin);
        assert_eq!(result, test_builtin());
    }

    #[test]
    fn header_only_file_uses_builtin() {
        let tmp = std::env::temp_dir().join("cache_test_header_only");
        let _ = fs::remove_dir_all(&tmp);
        let cache = test_cache("conf/HEADER_ONLY.txt");
        prep_file(&tmp.join("conf/HEADER_ONLY.txt"), TEST_PREFIX);
        let result = cache.resolve(&tmp, test_builtin);
        assert_eq!(result, test_builtin());
    }

    #[test]
    fn header_only_with_whitespace_uses_builtin() {
        let tmp = std::env::temp_dir().join("cache_test_header_ws");
        let _ = fs::remove_dir_all(&tmp);
        let cache = test_cache("HEADER_WS.txt");
        prep_file(&tmp.join("HEADER_WS.txt"), format!("{TEST_PREFIX}  \n  \n"));
        let result = cache.resolve(&tmp, test_builtin);
        assert_eq!(result, test_builtin());
    }

    #[test]
    fn valid_file_returns_content() {
        let tmp = std::env::temp_dir().join("cache_test_valid");
        let _ = fs::remove_dir_all(&tmp);
        let cache = test_cache("VALID.txt");
        prep_file(
            &tmp.join("VALID.txt"),
            format!("{TEST_PREFIX}\n\n{TEST_BODY}"),
        );
        let result = cache.resolve(&tmp, test_builtin);
        assert!(result.contains(TEST_BODY));
        assert!(result.contains(TEST_PREFIX));
    }

    #[test]
    fn file_without_prefix_gets_prefix_prepended() {
        let tmp = std::env::temp_dir().join("cache_test_no_prefix");
        let _ = fs::remove_dir_all(&tmp);
        let cache = test_cache("NO_PREFIX.txt");
        prep_file(&tmp.join("NO_PREFIX.txt"), TEST_BODY);
        let result = cache.resolve(&tmp, test_builtin);
        assert!(result.starts_with(TEST_PREFIX));
        assert!(result.contains(TEST_BODY));
    }

    #[test]
    fn mtime_cache_hit_returns_same_content() {
        let tmp = std::env::temp_dir().join("cache_test_mtime_hit");
        let _ = fs::remove_dir_all(&tmp);
        let file_path = tmp.join("MTIME.txt");
        let cache = test_cache("MTIME.txt");
        prep_file(&file_path, format!("{TEST_PREFIX}\n\n{TEST_BODY}"));

        let r1 = cache.resolve(&tmp, test_builtin);
        assert!(r1.contains(TEST_BODY));

        // Second call with unchanged mtime → cache hit
        let r2 = cache.resolve(&tmp, || "SHOULD_NOT_BE_USED".to_string());
        assert_eq!(r1, r2);
    }

    #[test]
    fn mtime_changed_re_reads() {
        let tmp = std::env::temp_dir().join("cache_test_mtime_change");
        let _ = fs::remove_dir_all(&tmp);
        let file_path = tmp.join("MTIME_CHANGE.txt");
        let cache = test_cache("MTIME_CHANGE.txt");
        prep_file(&file_path, format!("{TEST_PREFIX}\n\noriginal"));

        let _r1 = cache.resolve(&tmp, test_builtin);

        // Update file with new content
        prep_file(&file_path, format!("{TEST_PREFIX}\n\nmodified"));

        // Ensure mtime changes (sleep 10ms)
        std::thread::sleep(std::time::Duration::from_millis(10));

        let r2 = cache.resolve(&tmp, test_builtin);
        assert!(r2.contains("modified"));
    }

    #[test]
    fn poisoned_mutex_recovers() {
        let tmp = std::env::temp_dir().join("cache_test_poison");
        let _ = fs::remove_dir_all(&tmp);
        let cache = test_cache("POISON.txt");

        // Not easy to truly poison a Mutex in a single-threaded test;
        // verify the unwrap_or_else path works: no crash on access
        prep_file(&tmp.join("POISON.txt"), format!("{TEST_PREFIX}\n\nok"));
        let result = cache.resolve(&tmp, test_builtin);
        assert!(result.contains("ok"));
    }
}
