//! Content store — hash-addressed, file-backed, lossless payload persistence.
//!
//! Maps a SHA-256 digest to a file on disk so the full text can be offloaded
//! from the prompt and retrieved on demand.  The store is content-addressed
//! (put → hash, get by hash) with automatic directory sharding by the first
//! two hex chars of the digest.
//!
//! Storage layout (under `artifact_root/content-store/`):
//!   {shard}/{hash}.json   — each file is `{"hash":"…","content":"…"}`
//!
//! Reuses `fr_utils::json_io::*` for all I/O (no new filesystem primitives).

use core_errors::FrameworkError;
use fr_utils::constants::CONTENT_STORE_DIR;
use fr_utils::json_io::write_json_if_changed;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Returned when `get` finds no file for the given hash.
#[derive(Debug)]
pub struct ContentNotFound(pub String);

// ── On-disk envelope ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct ContentEntry {
    hash: String,
    content: String,
}

// ── Store ─────────────────────────────────────────────────────────────────────

/// A lightweight, content-addressed store rooted at a given artifact directory.
///
/// All I/O delegates to `fr_utils::json_io::write_json_if_changed` (which uses
/// atomic-rename writes) and `core_state_utils::json_io::read_json_if_exists`
/// (which is re-exported through `fr_utils`).
pub struct ContentStore {
    store_root: PathBuf,
}

impl ContentStore {
    /// Create (or open) a content store rooted at `artifact_root/content-store/`.
    ///
    /// The directory is created lazily on the first `put` call, not here.
    pub fn new(artifact_root: &Path) -> Self {
        Self {
            store_root: artifact_root.join(CONTENT_STORE_DIR),
        }
    }

    /// Store `content` under its SHA-256 digest and return the hex hash.
    ///
    /// If an entry with the same hash already exists this is a no-op (idempotent).
    pub fn put(&self, content: &str) -> Result<String, FrameworkError> {
        let hash = hex_hash(content);
        let path = self.path_for(&hash);
        if path.exists() {
            return Ok(hash); // already stored, idempotent
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let entry = ContentEntry {
            hash: hash.clone(),
            content: content.to_string(),
        };
        let entry_value = serde_json::to_value(&entry)?;
        write_json_if_changed(&path, &entry_value)?;
        Ok(hash)
    }

    /// Retrieve content by its SHA-256 hex hash.
    ///
    /// Returns `Err(ContentNotFound(hash))` when the file does not exist.
    pub fn get(&self, hash: &str) -> Result<String, ContentNotFound> {
        use core_state_utils::json_io::read_json_if_exists;
        let path = self.path_for(hash);
        let value = read_json_if_exists(&path);
        if value.is_null() {
            return Err(ContentNotFound(hash.to_string()));
        }
        let entry: ContentEntry =
            serde_json::from_value(value).map_err(|e| ContentNotFound(format!("deserialize: {e}")))?;
        if entry.hash == hash {
            Ok(entry.content)
        } else {
            Err(ContentNotFound(format!(
                "hash mismatch at {}: expected {hash}",
                path.display()
            )))
        }
    }

    /// Check whether content for `hash` exists in the store.
    pub fn exists(&self, hash: &str) -> bool {
        self.path_for(hash).exists()
    }

    /// Remove entries older than `max_age`.
    ///
    /// Returns the number of removed files.
    pub fn remove_stale(&self, max_age: Duration) -> Result<usize, FrameworkError> {
        let now = SystemTime::now();
        let mut removed = 0usize;
        if !self.store_root.exists() {
            return Ok(0);
        }
        for shard in std::fs::read_dir(&self.store_root)? {
            let shard = shard?;
            if !shard.file_type()?.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(shard.path())? {
                let entry = entry?;
                let meta = entry.metadata()?;
                let age = now
                    .duration_since(meta.modified().unwrap_or_else(|_| SystemTime::UNIX_EPOCH))
                    .unwrap_or(Duration::ZERO);
                if age > max_age {
                    std::fs::remove_file(entry.path())?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    // ── helpers ──

    fn path_for(&self, hash: &str) -> PathBuf {
        let shard = if hash.len() >= 2 { &hash[..2] } else { hash };
        self.store_root.join(shard).join(format!("{hash}.json"))
    }
}

/// Compute the SHA-256 hex digest of `input`.
pub fn hex_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn tmp_store() -> (ContentStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ContentStore::new(dir.path());
        (store, dir)
    }

    #[test]
    fn roundtrip() {
        let (store, _dir) = tmp_store();
        let text = "Hello, 世界! 这是一段测试文本。";
        let hash = store.put(text).expect("put");
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256 hex
        let got = store.get(&hash).expect("get");
        assert_eq!(got, text);
    }

    #[test]
    fn idempotent_put() {
        let (store, _dir) = tmp_store();
        let text = "same content";
        let h1 = store.put(text).expect("put 1");
        let h2 = store.put(text).expect("put 2");
        assert_eq!(h1, h2);
    }

    #[test]
    fn get_missing() {
        let (store, _dir) = tmp_store();
        let err = store.get("0000000000000000000000000000000000000000000000000000000000000000");
        assert!(matches!(err, Err(ContentNotFound(_))));
    }

    #[test]
    fn exists_after_put() {
        let (store, _dir) = tmp_store();
        let hash = store.put("exists check").expect("put");
        assert!(store.exists(&hash));
        assert!(!store.exists("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"));
    }

    #[test]
    fn remove_stale_removes_old() {
        let (store, dir) = tmp_store();
        store.put("fresh").expect("put fresh");
        store.put("another").expect("put another");
        // All files are brand new — removing with zero duration should remove them
        let removed = store
            .remove_stale(Duration::from_secs(0))
            .expect("remove_stale");
        assert!(removed >= 2, "expected >= 2, got {removed}");
        // verify store directory still exists but content gone
        let read = store.get(&hex_hash("fresh"));
        assert!(matches!(read, Err(ContentNotFound(_))));
        // prevent unused warning
        let _ = dir;
    }
}
