//! MCP tool registry: loads MCP_TOOL_REGISTRY.json and provides tool records.
//!
//! Caching layer uses TTL-based invalidation (default 60s) so edits to the JSON
//! file are picked up at most 60s after the last load, without requiring a restart
//! or manual `invalidate_tool_cache` call.
//!
//! ## Registry scope
//!
//! MCP_TOOL_REGISTRY.json contains tools that should be accessible via NL routing.
//! The following browser-mcp session/background/runtime tools are intentionally
//! NOT registered here — they are internal MCP protocol tools dispatched by the
//! browser-mcp server directly, not user-facing routing targets:
//!
//!   session_launch, session_list, session_inspect, session_terminate,
//!   session_mark_blocked, session_resume_due, session_classify_block,
//!   background_inspect, background_list, background_terminate,
//!   runtime_heartbeat, get_attached_runtime_events

use std::collections::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::tool_types::{McpToolInputSchema, McpToolRecord};

use core_errors::FrameworkError;

pub const EXPECTED_SCHEMA: &str = "mcp-tool-registry-v2";

/// Default TTL for the cached registry (60 seconds).
const CACHE_TTL_SECS: u64 = 60;

/// Maximum consecutive reload failures before propagating the error.
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

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

    // v2 object format: direct serde deserialization
    let records: Vec<McpToolRecord> = tools
        .iter()
        .map(|tool| {
            serde_json::from_value::<McpToolRecord>(tool.clone()).map_err(FrameworkError::Json)
        })
        .collect::<Result<Vec<_>, _>>()?;

    // ── MTR-4: duplicate slug detection ──
    let mut seen = HashSet::new();
    for record in &records {
        if !seen.insert(&record.slug) {
            return Err(FrameworkError::validation(format!(
                "duplicate tool slug in registry: {}",
                record.slug
            )));
        }
    }

    // ── Precompute routing token sets ──
    let records: Vec<McpToolRecord> = records
        .into_iter()
        .map(|mut r| {
            let slug_lower = r.slug.to_lowercase();
            let display_name_lower = r.display_name.to_lowercase();

            r.slug_lower = slug_lower.clone();
            r.display_name_lower = display_name_lower.clone();
            r.name_tokens = slug_lower
                .split(['-', '_'])
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string())
                .collect();
            r.keyword_tokens = r
                .trigger_hints
                .iter()
                .flat_map(|hint| {
                    core_state_utils::text_utils::tokenize_cjk_aware(&hint.to_lowercase())
                })
                .collect();
            r.desc_tokens = core_state_utils::text_utils::tokenize_cjk_aware(
                &r.description.to_lowercase(),
            )
            .into_iter()
            .collect();
            r.alias_tokens = core_state_utils::text_utils::tokenize_cjk_aware(&display_name_lower)
                .into_iter()
                .collect();
            r
        })
        .collect();

    // ── MTR-8: input_schema structural validation ──
    for record in &records {
        if let Some(ref schema) = record.input_schema_json {
            validate_input_schema(schema, &record.slug)?;
        }
    }

    Ok(records)
}

/// Validate the structure of an `McpToolInputSchema`.
fn validate_input_schema(schema: &McpToolInputSchema, slug: &str) -> Result<(), FrameworkError> {
    if schema.schema_type != "object" {
        return Err(FrameworkError::validation(format!(
            "tool '{slug}' input_schema type must be 'object', got '{}'",
            schema.schema_type
        )));
    }
    for req in &schema.required {
        if !schema.properties.contains_key(req) {
            return Err(FrameworkError::validation(format!(
                "tool '{slug}' input_schema required field '{req}' not found in properties"
            )));
        }
    }
    Ok(())
}

/// Cache entry: the loaded records plus metadata for staleness tracking.
struct CacheEntry {
    records: Vec<McpToolRecord>,
    loaded_at: Instant,
    consecutive_failures: u32,
    /// Hash of the file content at load time. Used alongside TTL to detect
    /// in-place file modifications — if the hash matches the cached entry,
    /// TTL is refreshed without re-parsing.
    content_hash: u64,
}

/// Compute a content fingerprint for the registry file.
/// Uses SipHash-2-4 (via std's DefaultHasher) — fast and adequate for
/// cache-invalidation purposes; no cryptographic guarantees needed.
fn compute_content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Process-level TTL cache, keyed by registry path.
/// Safe to call from multiple threads.
static CACHED: OnceLock<std::sync::RwLock<HashMap<PathBuf, CacheEntry>>> = OnceLock::new();

fn cache() -> &'static std::sync::RwLock<HashMap<PathBuf, CacheEntry>> {
    CACHED.get_or_init(|| std::sync::RwLock::new(HashMap::new()))
}

/// Load tool records with process-level TTL caching.
///
/// First call loads from disk; subsequent calls within `CACHE_TTL_SECS` return
/// the cached result. After the TTL expires, the next call reloads from disk.
/// On reload failure, returns stale cache data (with a warning) instead of
/// propagating the error, so transient I/O issues don't disrupt routing.
/// After `MAX_CONSECUTIVE_FAILURES` consecutive failures for a given path,
/// the error is propagated to the caller.
pub fn load_tool_records_cached(
    registry_path: &Path,
) -> Result<Vec<McpToolRecord>, FrameworkError> {
    let now = Instant::now();
    let path_buf = registry_path.to_path_buf();

    // Fast path: check cache (read lock)  — MTR-9: log lock poisoning
    {
        let guard = match cache().read() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::debug!("tool registry cache read lock poisoned, recovering");
                poisoned.into_inner()
            }
        };
        if let Some(entry) = guard.get(&path_buf)
            && now.duration_since(entry.loaded_at) < Duration::from_secs(CACHE_TTL_SECS)
        {
            return Ok(entry.records.clone());
        }
    }

    // TTL expired or cache empty: reload from disk (write lock)
    // ── MTR-6: lock poisoning gets a warning ──
    let mut guard = cache().write().unwrap_or_else(|poisoned| {
        tracing::warn!("tool registry cache write lock poisoned, recovering");
        poisoned.into_inner()
    });

    // Double-check after acquiring write lock (another thread may have reloaded)
    if let Some(entry) = guard.get(&path_buf)
        && now.duration_since(entry.loaded_at) < Duration::from_secs(CACHE_TTL_SECS)
    {
        return Ok(entry.records.clone());
    }

    // ── Content-hash short-circuit ──
    // If the file content hash matches the cached entry, the file hasn't changed.
    // Refresh the TTL (loaded_at) so subsequent callers hit the fast path,
    // without re-parsing the JSON. This prevents tools/list from diverging from
    // dispatch_tool behavior when the registry file is updated at runtime.
    let content_hash = fs::read_to_string(registry_path)
        .ok()
        .as_deref()
        .map(compute_content_hash)
        .unwrap_or(0);
    if content_hash != 0 {
        let should_rehash = guard.get(&path_buf)
            .map(|entry| entry.content_hash == content_hash)
            .unwrap_or(false);
        if should_rehash {
            let records = guard.get(&path_buf).map(|entry| entry.records.clone()).unwrap_or_default();
            guard.insert(
                path_buf,
                CacheEntry {
                    records: records.clone(),
                    loaded_at: Instant::now(),
                    consecutive_failures: 0,
                    content_hash,
                },
            );
            return Ok(records);
        }
    }

    match load_tool_records(registry_path) {
        Ok(records) => {
            guard.insert(
                path_buf,
                CacheEntry {
                    records: records.clone(),
                    loaded_at: Instant::now(),
                    consecutive_failures: 0,
                    content_hash,
                },
            );
            Ok(records)
        }
        Err(e) => {
            // ── MTR-1: consecutive failure tracking ──
            let stale = guard.get(&path_buf).map(|entry| CacheEntry {
                records: entry.records.clone(),
                loaded_at: entry.loaded_at,
                consecutive_failures: entry.consecutive_failures + 1,
                content_hash: entry.content_hash,
            });
            match stale {
                Some(CacheEntry {
                    records,
                    loaded_at,
                    consecutive_failures: failures,
                    ..
                }) if failures <= MAX_CONSECUTIVE_FAILURES => {
                    // Within threshold: return stale data with a warning
                    tracing::warn!(
                        "tool registry reload failed ({failures}/{MAX_CONSECUTIVE_FAILURES}), using stale cache: {e}"
                    );
                    guard.insert(
                        path_buf,
                        CacheEntry {
                            records: records.clone(),
                            loaded_at,
                            consecutive_failures: failures,
                            content_hash: 0, // hash unknown; next TTL expiry will re-check
                        },
                    );
                    Ok(records)
                }
                Some(CacheEntry {
                    records,
                    loaded_at,
                    consecutive_failures: failures,
                    ..
                }) => {
                    tracing::error!(
                        "tool registry reload failed {} consecutive times, propagating error: {e}",
                        MAX_CONSECUTIVE_FAILURES
                    );
                    guard.insert(
                        path_buf,
                        CacheEntry {
                            records: records.clone(),
                            loaded_at,
                            consecutive_failures: failures,
                            content_hash: 0,
                        },
                    );
                    Err(e)
                }
                None => Err(e),
            }
        }
    }
}

/// Manually invalidate the TTL cache, forcing the next `load_tool_records_cached`
/// call to reload from disk. Useful for testing or after explicit edits.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn invalidate_tool_cache() {
    match cache().write() {
        Ok(mut guard) => {
            guard.clear();
        }
        Err(poisoned) => {
            tracing::warn!("tool registry cache lock poisoned during invalidation, recovering");
            drop(poisoned.into_inner());
        }
    }
}

/// Invalidate the cache entry for a specific path.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn invalidate_tool_cache_for_path(registry_path: &Path) {
    let path_buf = registry_path.to_path_buf();
    match cache().write() {
        Ok(mut guard) => {
            guard.remove(&path_buf);
        }
        Err(poisoned) => {
            tracing::warn!(
                "tool registry cache lock poisoned during path invalidation, recovering"
            );
            drop(poisoned.into_inner());
        }
    }
}

#[cfg(test)]
pub(crate) fn set_cache_entry_for_test(
    path: PathBuf,
    records: Vec<McpToolRecord>,
    failures: u32,
    age: Duration,
) {
    let mut guard = cache().write().unwrap_or_else(|poisoned| {
        tracing::warn!("tool registry cache write lock poisoned during test, recovering");
        poisoned.into_inner()
    });
    guard.insert(
        path,
        CacheEntry {
            records,
            loaded_at: Instant::now() - age,
            consecutive_failures: failures,
            content_hash: 0,
        },
    );
}
