//! MCP tool registry: loads MCP_TOOL_REGISTRY.json and provides tool records.
//!
//! Caching layer uses TTL-based invalidation (default 60s) so edits to the JSON
//! file are picked up at most 60s after the last load, without requiring a restart
//! or manual `invalidate_tool_cache` call.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::tool_types::McpToolRecord;

use core_errors::FrameworkError;

pub const EXPECTED_SCHEMA: &str = "mcp-tool-registry-v2";

/// Default TTL for the cached registry (10 seconds).
const CACHE_TTL_SECS: u64 = 10;

/// Load tool records from MCP_TOOL_REGISTRY.json (v2 object format only).
pub fn load_tool_records(registry_path: &Path) -> Result<Vec<McpToolRecord>, FrameworkError> {
    let content = fs::read_to_string(registry_path)?;
    let root: Value = serde_json::from_str(&content)?;

    let schema = root
        .get("schema_version")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if schema != EXPECTED_SCHEMA {
        return Err(FrameworkError::validation(format!(
            "MCP_TOOL_REGISTRY.json schema mismatch: expected {EXPECTED_SCHEMA}, got {schema}"
        )));
    }

    let tools = root
        .get("tools")
        .and_then(|v| v.as_array())
        .ok_or(FrameworkError::validation(
            "MCP_TOOL_REGISTRY.json missing 'tools' array",
        ))?;

    if tools.is_empty() {
        return Ok(Vec::new());
    }

    // v2 object format: direct serde deserialization
    tools
        .iter()
        .map(|tool| {
            serde_json::from_value::<McpToolRecord>(tool.clone()).map_err(FrameworkError::Json)
        })
        .collect()
}

/// Cache entry: the loaded records plus when they were loaded.
struct CacheEntry {
    records: Vec<McpToolRecord>,
    loaded_at: Instant,
}

/// Process-level TTL cache. Safe to call from multiple threads.
static CACHED: OnceLock<std::sync::RwLock<Option<CacheEntry>>> = OnceLock::new();

fn cache() -> &'static std::sync::RwLock<Option<CacheEntry>> {
    CACHED.get_or_init(|| std::sync::RwLock::new(None))
}

/// Load tool records with process-level TTL caching.
///
/// First call loads from disk; subsequent calls within `CACHE_TTL_SECS` return
/// the cached result. After the TTL expires, the next call reloads from disk.
/// On reload failure, returns stale cache data (with a warning) instead of
/// propagating the error, so transient I/O issues don't disrupt routing.
pub fn load_tool_records_cached(
    registry_path: &Path,
) -> Result<Vec<McpToolRecord>, FrameworkError> {
    let now = Instant::now();

    // Fast path: check cache (read lock)
    {
        if let Ok(guard) = cache().read()
            && let Some(entry) = guard.as_ref()
            && now.duration_since(entry.loaded_at) < Duration::from_secs(CACHE_TTL_SECS)
        {
            return Ok(entry.records.clone());
        }
    }

    // TTL expired or cache empty: reload from disk (write lock)
    {
        let mut guard = cache()
            .write()
            .map_err(|_| FrameworkError::lock("cache poisoned"))?;
        // Double-check after acquiring write lock (another thread may have reloaded)
        if let Some(entry) = guard.as_ref()
            && now.duration_since(entry.loaded_at) < Duration::from_secs(CACHE_TTL_SECS)
        {
            return Ok(entry.records.clone());
        }
        match load_tool_records(registry_path) {
            Ok(records) => {
                guard.replace(CacheEntry {
                    records: records.clone(),
                    loaded_at: Instant::now(),
                });
                Ok(records)
            }
            Err(e) => {
                // Reload failed: return stale data if available, otherwise propagate error
                if let Some(entry) = guard.as_ref() {
                    tracing::warn!("tool registry reload failed, using stale cache: {e}");
                    Ok(entry.records.clone())
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// Manually invalidate the TTL cache, forcing the next `load_tool_records_cached`
/// call to reload from disk. Useful for testing or after explicit edits.
pub fn invalidate_tool_cache() {
    if let Ok(mut guard) = cache().write() {
        *guard = None;
    }
}
