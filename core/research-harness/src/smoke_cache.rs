//! LRU + TTL cache for experiment results with disk persistence.
//!
//! # Cache key
//! SHA-256 of `file_content_hash || sorted(params_json)`.
//! Uses content hash (not mtime) to avoid filesystem precision issues on FAT32/overlay.
//!
//! # Eviction
//! LRU eviction at capacity limit. TTL check on read (stale entries skipped).
//!
//! # Persistence
//! **In-memory writes are decoupled from disk persistence** — `set()` updates only the
//! in-memory cache. Explicit `flush()` writes to disk. This avoids O(N) disk writes in
//! parallel experiment runs where N experiments each call `set()`.
//!
//! Atomic write (temp + fsync + rename) via core-state-utils pattern.
//! Cross-process access protected by `flock` on a sentinel lock file.

use serde_json::{Value, json};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};

// ── Constants ──

const DEFAULT_CAPACITY: usize = 256;
const DEFAULT_TTL_SECS: u64 = 3600; // 1 hour
const CACHE_DISK_FILENAME: &str = "cache.json";
const CACHE_LOCK_FILENAME: &str = ".cache.lock";
const LOCK_ACQUIRE_TIMEOUT_MS: u64 = 500;

// ── Cache entry ──

struct CacheEntry {
    result: Value,
    cached_at: Instant,
}

// ── Cache struct ──

pub(crate) struct ExperimentCache {
    inner: OnceLock<RwLock<HashMap<String, CacheEntry>>>,
    artifacts_dir: PathBuf,
    capacity: usize,
    ttl: Duration,
    pub no_cache: bool,
    dirty: AtomicBool,  // true when in-memory cache has changes not yet on disk
}

impl ExperimentCache {
    pub fn new(artifacts_dir: &Path, no_cache: bool) -> Self {
        Self {
            inner: OnceLock::new(),
            artifacts_dir: artifacts_dir.to_path_buf(),
            capacity: DEFAULT_CAPACITY,
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
            no_cache,
            dirty: AtomicBool::new(false),
        }
    }

    fn cache(&self) -> &RwLock<HashMap<String, CacheEntry>> {
        self.inner.get_or_init(|| {
            let map = self.load_from_disk();
            RwLock::new(map)
        })
    }

    /// Compute a stable cache key for a template + content hash + params combination.
    ///
    /// This version takes a pre-computed `content_hash` to avoid re-hashing the
    /// template file for every experiment in a batch. Use `template_hash(path)`
    /// to pre-compute the hash, then call this function for each params variation.
    ///
    /// See also [`cache_key()`](Self::cache_key) for the legacy one-shot variant.
    pub fn cache_key_with_hash(
        template_name: &str,
        content_hash: &str,
        params: &HashMap<String, String>,
    ) -> String {
        // Use BTreeMap for deterministic key->value iteration order
        let sorted: std::collections::BTreeMap<&String, &String> = params.iter().collect();
        let params_json = serde_json::to_string(&sorted).unwrap_or_default();

        let mut hasher = Sha256::new();
        hasher.update(template_name.as_bytes());
        hasher.update(b"|");
        hasher.update(content_hash.as_bytes());
        hasher.update(b"|");
        hasher.update(params_json.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Compute a stable cache key for a template + params combination (one-shot).
    ///
    /// Convenience wrapper that computes the content hash internally.
    /// For batch operations on the same template, prefer:
    /// `cache_key_with_hash(name, &template_hash(path), params)`.
    pub fn cache_key(template_path: &Path, template_name: &str, params: &HashMap<String, String>) -> String {
        let content_hash = get_template_content_hash(template_path);
        Self::cache_key_with_hash(template_name, &content_hash, params)
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        if self.no_cache {
            return None;
        }
        let map = self.cache().read().ok()?;
        let entry = map.get(key)?;
        if entry.cached_at.elapsed() > self.ttl {
            return None; // stale — caller will recompute
        }
        Some(entry.result.clone())
    }

    /// Insert into in-memory cache only. Does NOT persist to disk.
    /// Call [`flush()`](Self::flush) to write pending changes to disk.
    pub fn set(&self, key: String, result: Value) {
        if self.no_cache {
            return;
        }
        let mut map = match self.cache().write() {
            Ok(m) => m,
            Err(_) => return,
        };

        // Evict oldest entries when at capacity
        while map.len() >= self.capacity {
            match map.iter().min_by_key(|(_, e)| e.cached_at) {
                Some((oldest_key, _)) => {
                    let key = oldest_key.clone();
                    map.remove(&key);
                }
                None => break,
            }
        }

        map.insert(key, CacheEntry {
            result: result.clone(),
            cached_at: Instant::now(),
        });

        self.dirty.store(true, Ordering::Release);
    }

    /// Explicitly persist in-memory cache to disk.
    ///
    /// This is a no-op if no changes have been made since the last flush.
    /// Thread-safe: only one writer will actually write while other concurrent
    /// callers skip.
    pub fn flush(&self) {
        if self.no_cache {
            return;
        }
        if !self.dirty.swap(false, Ordering::AcqRel) {
            return; // nothing to write
        }
        let map = match self.cache().read() {
            Ok(m) => m,
            Err(_) => return,
        };
        self.persist_to_disk(&map);
    }

    // ── Disk persistence (flock-guarded) ──

    fn cache_disk_path(&self) -> PathBuf {
        self.artifacts_dir.join(CACHE_DISK_FILENAME)
    }

    fn lock_path(&self) -> PathBuf {
        self.artifacts_dir.join(CACHE_LOCK_FILENAME)
    }

    /// Load cache from disk under a shared flock.
    fn load_from_disk(&self) -> HashMap<String, CacheEntry> {
        let path = self.cache_disk_path();
        if !path.exists() {
            return HashMap::new();
        }

        // Best-effort shared lock — not a hard error if we can't lock (stale reads acceptable)
        let _lock_guard = acquire_cache_lock(&self.lock_path(), /* exclusive */ false);

        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => return HashMap::new(),
        };
        let data: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => return HashMap::new(),
        };
        let Some(entries) = data.as_object() else {
            return HashMap::new();
        };

        let now = Instant::now();
        let mut map = HashMap::new();
        for (key, entry_val) in entries {
            let result = entry_val.get("result").cloned().unwrap_or(Value::Null);
            let age_secs = entry_val
                .get("age_secs")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let cached_at = now - Duration::from_secs_f64(age_secs);
            map.insert(key.clone(), CacheEntry { result, cached_at });
        }
        map
    }

    /// Persist cache to disk under an exclusive flock.
    fn persist_to_disk(&self, map: &HashMap<String, CacheEntry>) {
        let _lock_guard = match acquire_cache_lock(&self.lock_path(), /* exclusive */ true) {
            Some(g) => g,
            None => return, // best-effort
        };

        let now = Instant::now();
        let mut serializable = serde_json::Map::new();
        for (key, entry) in map {
            let age_secs = now.duration_since(entry.cached_at).as_secs_f64();
            serializable.insert(
                key.clone(),
                json!({
                    "age_secs": age_secs,
                    "result": entry.result,
                }),
            );
        }
        let content = serde_json::to_string(&Value::Object(serializable)).unwrap_or_default();

        // Atomic write: temp file + fsync + rename
        let final_path = self.cache_disk_path();
        let pid = std::process::id();
        let tmp_path = final_path.with_extension(format!("json.tmp-{pid}"));

        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
        {
            let ok = file.write_all(content.as_bytes()).is_ok()
                && file.sync_all().is_ok();
            drop(file);
            if ok {
                if let Err(e) = fs::rename(&tmp_path, &final_path) {
                    tracing::warn!(
                        error = %e,
                        tmp_path = %tmp_path.display(),
                        final_path = %final_path.display(),
                        "persist_to_disk: rename failed — cache data may be stale",
                    );
                }
            }
        }
        let _ = fs::remove_file(&tmp_path);
    }
}

// ── Template content hash (no caching — SHA-256 is fast and avoids stale/mtime races) ──

fn get_template_content_hash(path: &Path) -> String {
    hash_file_content(path)
}

fn hash_file_content(path: &Path) -> String {
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(_) => break,
        }
    }
    hex::encode(hasher.finalize())
}

// ── File lock ──

/// Acquire a flock on `.cache.lock`. Returns a guard that releases the lock on drop.
fn acquire_cache_lock(lock_path: &Path, exclusive: bool) -> Option<CacheLockGuard> {
    let file = match fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
    {
        Ok(f) => f,
        Err(_) => return None,
    };

    let deadline = Instant::now() + Duration::from_millis(LOCK_ACQUIRE_TIMEOUT_MS);
    let mut delay_ms = 10u64;
    loop {
        let locked = if exclusive {
            fs2::FileExt::try_lock_exclusive(&file)
        } else {
            fs2::FileExt::try_lock_shared(&file)
        };
        match locked {
            Ok(()) => return Some(CacheLockGuard(file)),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return None; // timeout
                }
                std::thread::sleep(Duration::from_millis(delay_ms));
                delay_ms = (delay_ms * 2).min(100);
            }
            Err(_) => return None,
        }
    }
}

struct CacheLockGuard(fs::File);

impl Drop for CacheLockGuard {
    fn drop(&mut self) {
        // Lock released automatically when `self.0` is closed.
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_deterministic() {
        let tmp = tempfile::tempdir().unwrap();
        let tmpl = tmp.path().join("test.sh");
        fs::write(&tmpl, "#!/bin/sh\necho '{}'").unwrap();

        let mut params = HashMap::new();
        params.insert("lr".into(), "0.01".into());
        params.insert("bs".into(), "32".into());

        let k1 = ExperimentCache::cache_key(&tmpl, "test.sh", &params);
        let k2 = ExperimentCache::cache_key(&tmpl, "test.sh", &params);
        assert_eq!(k1, k2, "cache key should be deterministic");
    }

    #[test]
    fn cache_key_changes_when_content_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let tmpl = tmp.path().join("test.sh");
        fs::write(&tmpl, "#!/bin/sh\necho '{}'").unwrap();

        let params = HashMap::new();
        let k1 = ExperimentCache::cache_key(&tmpl, "test.sh", &params);

        // Change content
        fs::write(&tmpl, "#!/bin/sh\necho '{\"x\": 1}'").unwrap();
        let k2 = ExperimentCache::cache_key(&tmpl, "test.sh", &params);

        assert_ne!(k1, k2, "cache key should change when template content changes");
    }

    #[test]
    fn cache_key_depends_on_params() {
        let tmp = tempfile::tempdir().unwrap();
        let tmpl = tmp.path().join("test.sh");
        fs::write(&tmpl, "#!/bin/sh\necho '{}'").unwrap();

        let mut params_a = HashMap::new();
        params_a.insert("lr".into(), "0.01".into());

        let mut params_b = HashMap::new();
        params_b.insert("lr".into(), "0.001".into());

        let ka = ExperimentCache::cache_key(&tmpl, "test.sh", &params_a);
        let kb = ExperimentCache::cache_key(&tmpl, "test.sh", &params_b);
        assert_ne!(ka, kb, "different param values should produce different keys");
    }

    #[test]
    fn set_and_get() {
        let tmp = tempfile::tempdir().unwrap();
        let artifacts_dir = tmp.path().join("artifacts/smoke");
        fs::create_dir_all(&artifacts_dir).unwrap();

        let cache = ExperimentCache::new(&artifacts_dir, false);
        let key = "test-key-123".to_string();
        let val = json!({"accuracy": 0.85});

        cache.set(key.clone(), val.clone());
        let retrieved = cache.get(&key);
        assert_eq!(retrieved, Some(val));
    }

    #[test]
    fn no_cache_skips_get() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ExperimentCache::new(&tmp.path(), true);

        // Set something first
        cache.set("k".into(), json!("v"));
        assert!(cache.get("k").is_none(), "no_cache should skip cache reads");
    }

    #[test]
    fn flush_persists_to_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let artifacts_dir = tmp.path().join("artifacts/smoke");
        fs::create_dir_all(&artifacts_dir).unwrap();

        let cache = ExperimentCache::new(&artifacts_dir, false);
        cache.set("k1".into(), json!("v1"));
        cache.set("k2".into(), json!("v2"));

        // Flush to disk
        cache.flush();

        // New cache instance on same dir should load from disk
        let cache2 = ExperimentCache::new(&artifacts_dir, false);
        assert_eq!(cache2.get("k1"), Some(json!("v1")));
        assert_eq!(cache2.get("k2"), Some(json!("v2")));
    }

    #[test]
    fn flush_noop_when_not_dirty() {
        let tmp = tempfile::tempdir().unwrap();
        let artifacts_dir = tmp.path().join("artifacts/smoke");
        fs::create_dir_all(&artifacts_dir).unwrap();
        let path = artifacts_dir.join("cache.json");

        let cache = ExperimentCache::new(&artifacts_dir, false);
        cache.set("k".into(), json!("v"));
        cache.flush(); // first flush writes
        let mtime1 = std::fs::metadata(&path).ok()
            .and_then(|m| m.modified().ok());

        // Second flush with no new changes should NOT write to disk
        cache.flush();
        let mtime2 = std::fs::metadata(&path).ok()
            .and_then(|m| m.modified().ok());

        assert_eq!(mtime1, mtime2, "flush should be noop when cache is not dirty");
    }
}
