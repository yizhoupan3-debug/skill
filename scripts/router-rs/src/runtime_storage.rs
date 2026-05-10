use crate::cli::runtime_ops::{build_runtime_control_plane_payload, required_non_empty_string};
use crate::runtime_envelope_ids::{RUNTIME_STORAGE_AUTHORITY, RUNTIME_STORAGE_SCHEMA_VERSION};
use fs2::FileExt;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(any(target_os = "linux", target_os = "android"))]
const O_NOFOLLOW_FLAG: i32 = 0o400000;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const O_NOFOLLOW_FLAG: i32 = 0x0100;

const RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_SCHEMA_VERSION: &str =
    "router-rs-runtime-checkpoint-control-plane-v1";
const RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_AUTHORITY: &str =
    "rust-runtime-checkpoint-control-plane";
const RUNTIME_CHECKPOINT_CONTROL_PLANE_SCHEMA_VERSION: &str = "runtime-checkpoint-control-plane-v1";
const DEFAULT_TRACE_SERVICE_AUTHORITY: &str = "rust-runtime-control-plane";
const DEFAULT_TRACE_SERVICE_ROLE: &str = "trace-and-handoff";
const DEFAULT_TRACE_SERVICE_PROJECTION: &str = "rust-native-projection";
const DEFAULT_STATE_SERVICE_AUTHORITY: &str = "rust-runtime-control-plane";
const DEFAULT_STATE_SERVICE_ROLE: &str = "durable-background-state";
const DEFAULT_STATE_SERVICE_PROJECTION: &str = "rust-native-projection";
const SQLITE_TABLE_NAME: &str = "runtime_storage_payloads";
const RUNTIME_BACKEND_FAMILY_CATALOG_SCHEMA_VERSION: &str =
    "runtime-persistence-backend-family-catalog-v1";
const RUNTIME_BACKEND_FAMILY_PARITY_SCHEMA_VERSION: &str =
    "runtime-persistence-backend-family-parity-v1";

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeBackendCapabilities {
    pub(crate) backend_family: &'static str,
    pub(crate) supports_atomic_replace: bool,
    pub(crate) supports_compaction: bool,
    pub(crate) supports_snapshot_delta: bool,
    pub(crate) supports_remote_event_transport: bool,
    pub(crate) supports_consistent_append: bool,
    pub(crate) supports_sqlite_wal: bool,
}

pub(crate) fn runtime_backend_capabilities(
    backend_family: &str,
) -> Result<RuntimeBackendCapabilities, String> {
    match normalized_backend_family(backend_family).as_str() {
        "filesystem" | "file" => Ok(RuntimeBackendCapabilities {
            backend_family: "filesystem",
            supports_atomic_replace: true,
            supports_compaction: false,
            supports_snapshot_delta: false,
            supports_remote_event_transport: true,
            supports_consistent_append: true,
            supports_sqlite_wal: false,
        }),
        "sqlite" | "sqlite3" => Ok(RuntimeBackendCapabilities {
            backend_family: "sqlite",
            supports_atomic_replace: true,
            supports_compaction: true,
            supports_snapshot_delta: true,
            supports_remote_event_transport: true,
            supports_consistent_append: true,
            supports_sqlite_wal: true,
        }),
        "memory" | "in_memory" | "regression" | "regression_double" => {
            Ok(RuntimeBackendCapabilities {
                backend_family: "memory",
                supports_atomic_replace: false,
                supports_compaction: false,
                supports_snapshot_delta: false,
                supports_remote_event_transport: true,
                supports_consistent_append: false,
                supports_sqlite_wal: false,
            })
        }
        other => Err(format!("unsupported runtime backend family: {other:?}")),
    }
}

pub(crate) fn runtime_backend_capabilities_payload(backend_family: &str) -> Result<Value, String> {
    let capabilities = runtime_backend_capabilities(backend_family)?;
    Ok(json!({
        "backend_family": capabilities.backend_family,
        "supports_atomic_replace": capabilities.supports_atomic_replace,
        "supports_compaction": capabilities.supports_compaction,
        "supports_snapshot_delta": capabilities.supports_snapshot_delta,
        "supports_remote_event_transport": capabilities.supports_remote_event_transport,
        "supports_consistent_append": capabilities.supports_consistent_append,
        "supports_sqlite_wal": capabilities.supports_sqlite_wal,
    }))
}

pub(crate) fn runtime_backend_family_catalog_payload() -> Value {
    let families = ["filesystem", "sqlite"]
        .into_iter()
        .filter_map(|family| runtime_backend_capabilities_payload(family).ok())
        .collect::<Vec<_>>();

    json!({
        "schema_version": RUNTIME_BACKEND_FAMILY_CATALOG_SCHEMA_VERSION,
        "authority": RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_AUTHORITY,
        "owner": "rust-runtime-checkpoint-control-plane",
        "default_backend_family": "filesystem",
        "strongest_local_backend_family": "sqlite",
        "families": families,
        "test_only_backend_families": ["memory"],
        "selection_rule": "store and checkpointer must resolve to one normalized backend_family before persistence operations",
    })
}

pub(crate) fn runtime_backend_family_parity_payload(
    store_backend_family: Option<&str>,
    checkpointer_backend_family: Option<&str>,
    trace_backend_family: Option<&str>,
    state_backend_family: Option<&str>,
) -> Result<Value, String> {
    let store = store_backend_family.unwrap_or("filesystem");
    let checkpointer = checkpointer_backend_family.unwrap_or(store);
    let trace = trace_backend_family.unwrap_or(checkpointer);
    let state = state_backend_family.unwrap_or(store);
    let store_capabilities = runtime_backend_capabilities(store)?;
    let checkpointer_capabilities = runtime_backend_capabilities(checkpointer)?;
    let trace_capabilities = runtime_backend_capabilities(trace)?;
    let state_capabilities = runtime_backend_capabilities(state)?;
    let normalized_store = store_capabilities.backend_family;
    let normalized_checkpointer = checkpointer_capabilities.backend_family;
    let normalized_trace = trace_capabilities.backend_family;
    let normalized_state = state_capabilities.backend_family;
    let aligned = normalized_store == normalized_checkpointer
        && normalized_store == normalized_trace
        && normalized_store == normalized_state;
    let mismatch_reason = if aligned {
        Value::Null
    } else {
        Value::String(
            "store, checkpointer, trace, and state must share one backend_family".to_string(),
        )
    };

    Ok(json!({
        "schema_version": RUNTIME_BACKEND_FAMILY_PARITY_SCHEMA_VERSION,
        "authority": RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_AUTHORITY,
        "store_backend_family": normalized_store,
        "checkpointer_backend_family": normalized_checkpointer,
        "trace_backend_family": normalized_trace,
        "state_backend_family": normalized_state,
        "aligned": aligned,
        "mismatch_reason": mismatch_reason,
        "compaction_eligible": aligned
            && checkpointer_capabilities.supports_compaction
            && checkpointer_capabilities.supports_snapshot_delta
            && state_capabilities.supports_compaction
            && state_capabilities.supports_snapshot_delta,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeStorageRequestPayload {
    pub(crate) operation: String,
    pub(crate) path: String,
    pub(crate) backend_family: String,
    pub(crate) sqlite_db_path: Option<String>,
    pub(crate) storage_root: Option<String>,
    pub(crate) payload_text: Option<String>,
    pub(crate) expected_sha256: Option<String>,
    pub(crate) max_bytes: Option<usize>,
    pub(crate) tail_lines: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeStorageResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) operation: String,
    pub(crate) path: String,
    pub(crate) backend_family: String,
    pub(crate) sqlite_db_path: Option<String>,
    pub(crate) storage_root: Option<String>,
    pub(crate) backend_capabilities: Value,
    pub(crate) exists: bool,
    pub(crate) payload_text: Option<String>,
    pub(crate) bytes_written: Option<usize>,
    pub(crate) bytes_returned: Option<usize>,
    pub(crate) payload_sha256: Option<String>,
    pub(crate) verified: Option<bool>,
    pub(crate) truncated: Option<bool>,
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedStorageBackend {
    Filesystem,
    Memory,
    Sqlite {
        db_path: PathBuf,
        storage_root: PathBuf,
    },
}

fn normalize_runtime_path(value: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(value.trim());
    if candidate.as_os_str().is_empty() {
        return Err("runtime storage path must be non-empty".to_string());
    }
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(candidate))
            .map_err(|err| format!("resolve runtime storage path failed: {err}"))?
    };
    canonicalize_or_clean_absolute_path(&absolute)
}

fn clean_absolute_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "runtime storage path must be absolute after resolution: {}",
            path.display()
        ));
    }
    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => cleaned.push(prefix.as_os_str()),
            Component::RootDir => cleaned.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(segment) => cleaned.push(segment),
            Component::ParentDir => {
                if !cleaned.pop() {
                    return Err(format!(
                        "runtime storage path escapes filesystem root: {}",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(cleaned)
}

fn canonicalize_or_clean_absolute_path(path: &Path) -> Result<PathBuf, String> {
    clean_absolute_path(path)
}

/// Resolve symlinks on the longest existing ancestor of `path`, then re-attach
/// any non-existing tail components verbatim. The returned path reflects the
/// real filesystem location after symlink resolution and is suitable for
/// containment checks against a canonical storage root, even when the final
/// target (or some intermediate components) does not yet exist.
fn canonicalize_existing_ancestors(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "runtime storage path must be absolute before symlink resolution: {}",
            path.display()
        ));
    }

    let mut current = path.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(_) => break,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let Some(file_name) = current.file_name().map(|name| name.to_os_string()) else {
                    return Err(format!(
                        "runtime storage path has no existing ancestor: {}",
                        path.display()
                    ));
                };
                tail.push(file_name);
                if !current.pop() {
                    return Err(format!(
                        "runtime storage path has no existing ancestor: {}",
                        path.display()
                    ));
                }
            }
            Err(err) => {
                return Err(format!(
                    "stat runtime storage path {} failed: {err}",
                    current.display()
                ));
            }
        }
    }

    let canonical = current.canonicalize().map_err(|err| {
        format!(
            "canonicalize runtime storage ancestor {} failed: {err}",
            current.display()
        )
    })?;

    let mut result = canonical;
    for name in tail.iter().rev() {
        result.push(name);
    }
    Ok(result)
}

fn resolve_runtime_storage_path_with_root(
    request_path: &str,
    request_storage_root: Option<&str>,
) -> Result<(PathBuf, PathBuf), String> {
    let storage_root = match request_storage_root {
        Some(value) => normalize_runtime_path(value)?,
        None => {
            let cwd = std::env::current_dir()
                .map_err(|err| format!("resolve current dir failed: {err}"))?;
            canonicalize_or_clean_absolute_path(&cwd)?
        }
    };
    let trimmed_path = request_path.trim();
    if trimmed_path.is_empty() {
        return Err("runtime storage path must be non-empty".to_string());
    }
    let candidate = PathBuf::from(trimmed_path);
    let absolute_candidate = if candidate.is_absolute() {
        candidate
    } else {
        storage_root.join(candidate)
    };
    let resolved_path = canonicalize_or_clean_absolute_path(&absolute_candidate)?;
    if !resolved_path.starts_with(&storage_root) {
        return Err(format!(
            "runtime storage path {} must stay under storage root {}",
            resolved_path.display(),
            storage_root.display()
        ));
    }

    // Real-path containment: resolve any symlinks along the existing parent
    // chain on both sides before comparing. A lexical `starts_with` alone is
    // insufficient because a symlink in the parent directory chain (e.g. a
    // pre-existing `escape -> /outside` link inside `storage_root`) would
    // otherwise let writes leak outside `storage_root` even though every
    // textual component still appears to live under it.
    let canonical_storage_root = canonicalize_existing_ancestors(&storage_root)?;
    let canonical_resolved_path = canonicalize_existing_ancestors(&resolved_path)?;
    if !canonical_resolved_path.starts_with(&canonical_storage_root) {
        return Err(format!(
            "runtime storage path {} must stay under storage root {} after symlink resolution",
            canonical_resolved_path.display(),
            canonical_storage_root.display()
        ));
    }

    Ok((resolved_path, storage_root))
}

/// Pick the effective `storage_root` string for a runtime_storage request.
///
/// Order of resolution:
///   1. explicit non-empty `storage_root` from the request,
///   2. for sqlite/sqlite3 backends without an explicit root, fall back to
///      `sqlite_db_path.parent()` to preserve the historical default
///      semantics for sqlite-backed runtime storage,
///   3. opt-in host-aware fallback **only** when the caller explicitly
///      sets `ROUTER_RS_STORAGE_ROOT`. We deliberately do NOT silently
///      consult `CODEX_HOME` / `CURSOR_HOME` here, because callers in
///      codex/cursor environments routinely have those env vars set and
///      pass relative `path` arguments expecting cwd anchoring; redirecting
///      writes to the host home directory would be a silent breaking
///      change. Callers that want host-home anchoring must pass
///      `storage_root` explicitly in the request payload.
///   4. otherwise return `None` so the caller falls back to the
///      current working directory (legacy default for non-sqlite backends).
fn effective_storage_root_for_request(request: &RuntimeStorageRequestPayload) -> Option<String> {
    if let Some(value) = request.storage_root.as_deref() {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let backend_family = normalized_backend_family(&request.backend_family);
    if matches!(backend_family.as_str(), "sqlite" | "sqlite3") {
        if let Some(db_path_str) = request.sqlite_db_path.as_deref() {
            let trimmed = db_path_str.trim();
            if !trimmed.is_empty() {
                if let Ok(normalized) = normalize_runtime_path(trimmed) {
                    if let Some(parent) = normalized.parent() {
                        if !parent.as_os_str().is_empty() {
                            return Some(parent.display().to_string());
                        }
                    }
                }
            }
        }
    }
    explicit_storage_root_override()
}

/// Read the explicit `ROUTER_RS_STORAGE_ROOT` override. Unlike `CODEX_HOME`
/// or `CURSOR_HOME`, this env var exists solely to point router-rs at a
/// storage root and is therefore safe to consult silently. Returns `None`
/// when the var is unset or empty.
fn explicit_storage_root_override() -> Option<String> {
    match std::env::var("ROUTER_RS_STORAGE_ROOT") {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => None,
    }
}

fn normalized_backend_family(value: &str) -> String {
    value.trim().to_lowercase().replace('-', "_")
}

fn stable_memory_key(path: &Path) -> Result<String, String> {
    Ok(normalize_runtime_path(&path.display().to_string())?
        .display()
        .to_string())
}

fn payload_sha256(payload_text: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(payload_text.as_bytes());
    format!("{:x}", digest.finalize())
}

fn stream_sha256_hex_reader(reader: &mut impl Read) -> Result<String, std::io::Error> {
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65_536];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn stream_sha256_hex_path(path: &Path) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    stream_sha256_hex_reader(&mut file)
}

/// Post-append digest: full payload hash when readable; `None` when read is not permitted
/// (for example write-only log files) so append can still succeed without materializing
/// the entire file as a `String`.
fn digest_after_append_text(
    path: &Path,
    backend: &ResolvedStorageBackend,
    constrained_storage_root: &Path,
) -> Result<Option<String>, String> {
    match backend {
        ResolvedStorageBackend::Filesystem => match stream_sha256_hex_path(path) {
            Ok(hex) => Ok(Some(hex)),
            Err(err) if err.kind() == ErrorKind::PermissionDenied => Ok(None),
            Err(err) => Err(format!(
                "runtime_storage append_text digest read failed for {}: {err}",
                path.display()
            )),
        },
        ResolvedStorageBackend::Memory => {
            let artifact_path = memory_artifact_path(path)?;
            match stream_sha256_hex_path(&artifact_path) {
                Ok(hex) => Ok(Some(hex)),
                Err(err) if err.kind() == ErrorKind::PermissionDenied => Ok(None),
                Err(err) => Err(format!(
                    "runtime_storage append_text digest read failed for {}: {err}",
                    artifact_path.display()
                )),
            }
        }
        ResolvedStorageBackend::Sqlite {
            db_path,
            storage_root: _,
        } => {
            let full = sqlite_read_text(path, db_path, constrained_storage_root)?;
            Ok(Some(payload_sha256(&full)))
        }
    }
}

fn memory_storage_root() -> Result<PathBuf, String> {
    let cwd =
        std::env::current_dir().map_err(|err| format!("resolve current dir failed: {err}"))?;
    let mut digest = Sha256::new();
    digest.update(cwd.display().to_string().as_bytes());
    let namespace = format!("{:x}", digest.finalize());
    Ok(std::env::temp_dir()
        .join("router-rs-runtime-memory-v1")
        .join(namespace))
}

fn memory_artifact_path(path: &Path) -> Result<PathBuf, String> {
    let stable_key = stable_memory_key(path)?;
    let mut digest = Sha256::new();
    digest.update(stable_key.as_bytes());
    let key = format!("{:x}", digest.finalize());
    Ok(memory_storage_root()?.join(format!("{key}.payload")))
}

fn env_checkpoint_storage_db_path() -> Option<PathBuf> {
    std::env::var("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE")
        .ok()
        .and_then(|value| normalize_runtime_path(&value).ok())
}

fn runtime_storage_db_name_candidates() -> Vec<String> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();
    for candidate in [
        std::env::var("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE").ok(),
        Some("runtime_checkpoint_store.sqlite3".to_string()),
    ]
    .into_iter()
    .flatten()
    {
        if seen.insert(candidate.clone()) {
            ordered.push(candidate);
        }
    }
    ordered
}

fn sqlite_connection(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|err| {
        format!(
            "open sqlite runtime storage failed for {}: {err}",
            path.display()
        )
    })?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|err| format!("enable sqlite runtime storage WAL failed: {err}"))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|err| format!("set sqlite runtime storage synchronous mode failed: {err}"))?;
    Ok(conn)
}

fn ensure_runtime_storage_sqlite_schema(conn: &Connection) -> Result<(), String> {
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {SQLITE_TABLE_NAME} (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)"
        ),
        [],
    )
    .map_err(|err| format!("ensure sqlite runtime storage schema failed: {err}"))?;
    Ok(())
}

fn sqlite_lookup_key(path: &Path, storage_root: &Path) -> Result<String, String> {
    let resolved_path = normalize_runtime_path(&path.display().to_string())?;
    let resolved_root = normalize_runtime_path(&storage_root.display().to_string())?;
    let relative_path = resolved_path
        .strip_prefix(&resolved_root)
        .map_err(|_| {
            format!(
                "sqlite runtime storage path {} must stay under storage root {}",
                resolved_path.display(),
                resolved_root.display()
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");
    // Namescape keys by resolved storage root so one sqlite db can safely serve
    // multiple session roots without payload collisions.
    let root_scope = resolved_root.display().to_string().replace('\\', "/");
    Ok(format!("{root_scope}::{relative_path}"))
}

fn sqlite_payload_exists(path: &Path, db_path: &Path, storage_root: &Path) -> Result<bool, String> {
    let stable_key = sqlite_lookup_key(path, storage_root)?;
    let conn = sqlite_connection(db_path)?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT 1 FROM {SQLITE_TABLE_NAME} WHERE payload_key = ?1 LIMIT 1"
        ))
        .map_err(|err| format!("prepare sqlite exists query failed: {err}"))?;
    let exists = stmt
        .query_row(params![stable_key], |row| row.get::<_, i64>(0))
        .optional()
        .map_err(|err| format!("run sqlite exists query failed: {err}"))?
        .is_some();
    Ok(exists)
}

fn sqlite_read_text(path: &Path, db_path: &Path, storage_root: &Path) -> Result<String, String> {
    let stable_key = sqlite_lookup_key(path, storage_root)?;
    let conn = sqlite_connection(db_path)?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT payload_text FROM {SQLITE_TABLE_NAME} WHERE payload_key = ?1 LIMIT 1"
        ))
        .map_err(|err| format!("prepare sqlite read query failed: {err}"))?;
    stmt.query_row(params![stable_key], |row| row.get::<_, String>(0))
        .map_err(|err| format!("read sqlite payload failed for {}: {err}", path.display()))
}

fn sqlite_write_text(
    path: &Path,
    db_path: &Path,
    storage_root: &Path,
    payload_text: &str,
) -> Result<(), String> {
    let stable_key = sqlite_lookup_key(path, storage_root)?;
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create sqlite parent directory for {} failed: {err}",
                db_path.display()
            )
        })?;
    }
    let conn = sqlite_connection(db_path)?;
    ensure_runtime_storage_sqlite_schema(&conn)?;
    conn.execute(
        &format!(
            "INSERT INTO {SQLITE_TABLE_NAME} (payload_key, payload_text) VALUES (?1, ?2)
             ON CONFLICT(payload_key) DO UPDATE SET payload_text = excluded.payload_text"
        ),
        params![stable_key, payload_text],
    )
    .map_err(|err| format!("write sqlite payload failed for {}: {err}", path.display()))?;
    Ok(())
}

fn sqlite_append_text(
    path: &Path,
    db_path: &Path,
    storage_root: &Path,
    payload_text: &str,
) -> Result<(), String> {
    let stable_key = sqlite_lookup_key(path, storage_root)?;
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create sqlite parent directory for {} failed: {err}",
                db_path.display()
            )
        })?;
    }
    let conn = sqlite_connection(db_path)?;
    ensure_runtime_storage_sqlite_schema(&conn)?;
    conn.execute(
        &format!(
            "INSERT INTO {SQLITE_TABLE_NAME} (payload_key, payload_text) VALUES (?1, ?2)
             ON CONFLICT(payload_key) DO UPDATE
             SET payload_text = {SQLITE_TABLE_NAME}.payload_text || excluded.payload_text"
        ),
        params![stable_key, payload_text],
    )
    .map_err(|err| format!("append sqlite payload failed for {}: {err}", path.display()))?;
    Ok(())
}

/// Symlink policy for filesystem `write_text` / `append_text`:
/// reject when the final path already exists as a symlink (`symlink_metadata`).
/// This avoids following a symlink on append and makes the write target explicit
/// (callers must write to a normal file path, not an alias).
fn filesystem_reject_symlink_write_target(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.is_symlink() {
                return Err(format!(
                    "runtime storage path {} must not be a symlink",
                    path.display()
                ));
            }
            Ok(())
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!(
            "stat runtime storage path {} failed: {err}",
            path.display()
        )),
    }
}

const FILESYSTEM_TEMP_CREATE_ATTEMPTS: u32 = 128;

/// RAII guard for a cross-process advisory lock keyed by an arbitrary
/// runtime path. The guard owns a sentinel `.router-rs.<filename>.lock`
/// file alongside the target; advisory `flock(LOCK_EX)` is held for the
/// lifetime of the guard and released on drop. The sentinel file is
/// intentionally left on disk so future acquisitions reuse the same
/// inode (avoiding TOCTOU races on lock-file creation).
pub(crate) struct RuntimePathLockGuard {
    _file: fs::File,
}

/// Acquire an exclusive cross-process lock for `path`. Multiple writers
/// (codex/cursor/test harness) racing on the same shared runtime artifact
/// (`background_state.json`, trace JSONL, supervisor state, etc.) will
/// serialize through this lock so read-modify-write sequences stay atomic
/// at the process boundary. The OS releases the lock if the process dies.
pub(crate) fn acquire_runtime_path_lock(path: &Path) -> Result<RuntimePathLockGuard, String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "runtime path {} has no parent directory for lock placement",
            path.display()
        )
    })?;
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create runtime lock parent directory failed for {}: {err}",
                path.display()
            )
        })?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime-storage");
    let lock_path = parent.join(format!(".router-rs.{file_name}.lock"));
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|err| {
            format!(
                "open runtime path lock {} failed: {err}",
                lock_path.display()
            )
        })?;
    file.lock_exclusive().map_err(|err| {
        format!(
            "acquire runtime path lock {} failed: {err}",
            lock_path.display()
        )
    })?;
    Ok(RuntimePathLockGuard { _file: file })
}

fn filesystem_atomic_temp_path(
    parent: &Path,
    file_name: &str,
    nanos: u128,
    pid: u32,
    attempt: u32,
) -> PathBuf {
    let mut digest = Sha256::new();
    digest.update(file_name.as_bytes());
    digest.update(b"\x1e");
    digest.update(nanos.to_le_bytes());
    digest.update(b"\x1e");
    digest.update(pid.to_le_bytes());
    digest.update(b"\x1e");
    digest.update(attempt.to_le_bytes());
    let tag = format!("{:x}", digest.finalize());
    parent.join(format!(".router-rs.{file_name}.{tag}.tmp"))
}

fn filesystem_write_text_inner(path: &Path, payload_text: &str, nanos: u128) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create runtime storage parent directory failed for {}: {err}",
                path.display()
            )
        })?;
    }
    // Cross-process lock: serialize concurrent writers (codex+cursor+tests)
    // sharing the same artifact path to prevent last-writer-wins overwrites.
    let _path_lock = acquire_runtime_path_lock(path)?;
    filesystem_reject_symlink_write_target(path)?;

    let parent = path.parent().ok_or_else(|| {
        format!(
            "runtime storage path {} has no parent directory",
            path.display()
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime-storage");
    let pid = std::process::id();

    let (tmp_path, mut file) = {
        let mut chosen: Option<(PathBuf, fs::File)> = None;
        for attempt in 0u32..FILESYSTEM_TEMP_CREATE_ATTEMPTS {
            let candidate = filesystem_atomic_temp_path(parent, file_name, nanos, pid, attempt);
            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&candidate)
            {
                Ok(file) => {
                    chosen = Some((candidate, file));
                    break;
                }
                Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(format!(
                        "create runtime storage temp file {} failed: {err}",
                        candidate.display()
                    ));
                }
            }
        }
        chosen.ok_or_else(|| {
            "exhausted runtime storage temp file create attempts (unexpected collision load)"
                .to_string()
        })?
    };

    let write_result = file
        .write_all(payload_text.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|err| {
            format!(
                "write runtime storage temp payload failed for {}: {err}",
                tmp_path.display()
            )
        });
    if let Err(err) = write_result {
        drop(file);
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }
    drop(file);

    if let Err(err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "replace runtime storage payload failed for {}: {err}",
            path.display()
        ));
    }
    Ok(())
}

fn filesystem_write_text(path: &Path, payload_text: &str) -> Result<(), String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system time before unix epoch: {err}"))?
        .as_nanos();
    filesystem_write_text_inner(path, payload_text, nanos)
}

fn filesystem_append_text(path: &Path, payload_text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create runtime storage parent directory failed for {}: {err}",
                path.display()
            )
        })?;
    }
    // Cross-process append lock prevents JSONL line-interleaving when codex
    // and cursor (or parallel tests) tail the same trace/event stream.
    let _path_lock = acquire_runtime_path_lock(path)?;
    filesystem_reject_symlink_write_target(path)?;
    let mut file = filesystem_open_append_text(path)?;
    file.write_all(payload_text.as_bytes()).map_err(|err| {
        format!(
            "append runtime storage payload failed for {}: {err}",
            path.display()
        )
    })?;
    file.sync_data().map_err(|err| {
        format!(
            "sync runtime storage append failed for {}: {err}",
            path.display()
        )
    })?;
    Ok(())
}

#[cfg(unix)]
fn filesystem_open_append_text(path: &Path) -> Result<fs::File, String> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .custom_flags(O_NOFOLLOW_FLAG)
        .open(path)
        .map_err(|err| {
            format!(
                "open runtime storage payload for append failed for {}: {err}",
                path.display()
            )
        })
}

#[cfg(not(unix))]
fn filesystem_open_append_text(path: &Path) -> Result<fs::File, String> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| {
            format!(
                "open runtime storage payload for append failed for {}: {err}",
                path.display()
            )
        })
}

fn slice_tail_by_max_bytes(payload: &str, max_bytes: usize) -> String {
    if payload.len() <= max_bytes {
        return payload.to_string();
    }
    let mut start = payload.len().saturating_sub(max_bytes);
    while start < payload.len() && !payload.is_char_boundary(start) {
        start += 1;
    }
    let mut limited = payload[start..].to_string();
    if limited.starts_with('\n') && limited.len() > 1 {
        limited.remove(0);
    }
    limited
}

fn slice_tail_by_lines(payload: &str, tail_lines: usize) -> String {
    if tail_lines == 0 {
        return String::new();
    }
    let mut starts = vec![0usize];
    for (idx, ch) in payload.char_indices() {
        if ch == '\n' && idx + 1 < payload.len() {
            starts.push(idx + 1);
        }
    }
    if starts.len() <= tail_lines {
        return payload.to_string();
    }
    payload[starts[starts.len() - tail_lines]..].to_string()
}

fn apply_read_limits(
    payload: String,
    max_bytes: Option<usize>,
    tail_lines: Option<usize>,
) -> (String, bool) {
    let original_len = payload.len();
    let mut limited = payload;
    if let Some(lines) = tail_lines {
        limited = slice_tail_by_lines(&limited, lines);
    }
    if let Some(max) = max_bytes {
        limited = slice_tail_by_max_bytes(&limited, max);
    }
    let truncated = limited.len() < original_len;
    (limited, truncated)
}

pub(crate) fn storage_artifact_exists(
    path: &Path,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> bool {
    if path.exists() {
        return true;
    }
    match storage_backend {
        Some(ResolvedStorageBackend::Filesystem) => false,
        Some(ResolvedStorageBackend::Memory) => memory_artifact_path(path)
            .map(|artifact_path| artifact_path.exists())
            .unwrap_or(false),
        Some(ResolvedStorageBackend::Sqlite {
            db_path,
            storage_root,
        }) => sqlite_payload_exists(path, db_path, storage_root).unwrap_or(false),
        None => false,
    }
}

pub(crate) fn storage_read_text(
    path: &Path,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<String, String> {
    if path.exists() {
        return fs::read_to_string(path)
            .map_err(|err| format!("read artifact failed for {}: {err}", path.display()));
    }
    match storage_backend {
        Some(ResolvedStorageBackend::Filesystem) | None => {
            Err(format!("artifact does not exist: {}", path.display()))
        }
        Some(ResolvedStorageBackend::Memory) => fs::read_to_string(memory_artifact_path(path)?)
            .map_err(|err| {
                format!(
                    "read memory storage payload failed for {}: {err}",
                    path.display()
                )
            }),
        Some(ResolvedStorageBackend::Sqlite {
            db_path,
            storage_root,
        }) => sqlite_read_text(path, db_path, storage_root),
    }
}

pub(crate) fn resolve_storage_backend(paths: &[PathBuf]) -> Option<ResolvedStorageBackend> {
    if paths.is_empty() {
        return None;
    }
    if paths.iter().any(|path| path.exists()) {
        return Some(ResolvedStorageBackend::Filesystem);
    }

    let mut roots = Vec::new();
    let mut seen_roots = HashSet::new();
    for path in paths {
        let mut candidates = Vec::new();
        let parent = path.parent();
        let parent_name = parent
            .and_then(|value| value.file_name())
            .and_then(|name| name.to_str());
        let grandparent = parent.and_then(Path::parent);
        let grandparent_name = grandparent
            .and_then(|value| value.file_name())
            .and_then(|name| name.to_str());

        let file_name = path.file_name().and_then(|name| name.to_str());

        if parent_name == Some("runtime_event_transports")
            || parent_name == Some("trace_compaction")
        {
            if let Some(root) = grandparent {
                candidates.push(root.to_path_buf());
            }
            if let Some(root) = grandparent.and_then(Path::parent) {
                candidates.push(root.to_path_buf());
            }
        }
        if matches!(
            file_name,
            Some("TRACE_RESUME_MANIFEST.json")
                | Some("TRACE_EVENTS.jsonl")
                | Some("ATTACHED_RUNTIME_EVENT_HANDOFF.json")
        ) {
            if let Some(root) = path.parent() {
                candidates.push(root.to_path_buf());
            }
            if let Some(root) = path.parent().and_then(Path::parent) {
                candidates.push(root.to_path_buf());
            }
        }
        if grandparent_name == Some("trace_compaction") {
            if let Some(root) = grandparent.and_then(Path::parent) {
                candidates.push(root.to_path_buf());
            }
        }
        if let Some(parent) = path.parent() {
            candidates.push(parent.to_path_buf());
        }
        for candidate in candidates {
            let normalized = normalize_runtime_path(&candidate.display().to_string()).ok()?;
            if seen_roots.insert(normalized.clone()) {
                roots.push(normalized);
            }
        }
    }

    if let Some(db_path) =
        env_checkpoint_storage_db_path().filter(|path| path.is_absolute() && path.exists())
    {
        for root in &roots {
            let backend = ResolvedStorageBackend::Sqlite {
                db_path: db_path.clone(),
                storage_root: root.clone(),
            };
            if paths
                .iter()
                .any(|path| storage_artifact_exists(path, Some(&backend)))
            {
                return Some(backend);
            }
        }
    }

    let db_name_candidates = runtime_storage_db_name_candidates();
    for root in &roots {
        for db_name in &db_name_candidates {
            let db_path = root.join(db_name);
            if !db_path.exists() {
                continue;
            }
            let backend = ResolvedStorageBackend::Sqlite {
                db_path,
                storage_root: root.clone(),
            };
            if paths
                .iter()
                .any(|path| storage_artifact_exists(path, Some(&backend)))
            {
                return Some(backend);
            }
        }
    }

    None
}

fn resolve_runtime_storage_backend(
    request: &RuntimeStorageRequestPayload,
    constrained_storage_root: &Path,
) -> Result<
    (
        ResolvedStorageBackend,
        String,
        Option<String>,
        Option<String>,
    ),
    String,
> {
    let backend_family = normalized_backend_family(&request.backend_family);
    let capabilities = runtime_backend_capabilities(&backend_family)?;
    match capabilities.backend_family {
        "filesystem" => Ok((
            ResolvedStorageBackend::Filesystem,
            capabilities.backend_family.to_string(),
            None,
            None,
        )),
        "memory" => Ok((
            ResolvedStorageBackend::Memory,
            capabilities.backend_family.to_string(),
            None,
            None,
        )),
        "sqlite" => {
            let db_path = request
                .sqlite_db_path
                .as_ref()
                .ok_or_else(|| "runtime_storage sqlite backend requires sqlite_db_path".to_string())
                .and_then(|value| normalize_runtime_path(value))?;
            let storage_root = constrained_storage_root.to_path_buf();
            Ok((
                ResolvedStorageBackend::Sqlite {
                    db_path: db_path.clone(),
                    storage_root: storage_root.clone(),
                },
                capabilities.backend_family.to_string(),
                Some(db_path.display().to_string()),
                Some(storage_root.display().to_string()),
            ))
        }
        other => Err(format!("unsupported runtime storage backend: {other}")),
    }
}

pub(crate) fn runtime_storage_operation(
    request: RuntimeStorageRequestPayload,
) -> Result<RuntimeStorageResponsePayload, String> {
    let effective_storage_root = effective_storage_root_for_request(&request);
    let (path, constrained_storage_root) =
        resolve_runtime_storage_path_with_root(&request.path, effective_storage_root.as_deref())?;
    let (backend, backend_family, sqlite_db_path, storage_root) =
        resolve_runtime_storage_backend(&request, &constrained_storage_root)?;
    let operation = request.operation.trim().to_lowercase();
    let expected_sha256 = request.expected_sha256.clone();
    let payload_text = request.payload_text;
    let max_bytes = request.max_bytes;
    let tail_lines = request.tail_lines;

    let (
        exists,
        resolved_payload_text,
        bytes_written,
        bytes_returned,
        payload_digest,
        verified,
        truncated,
    ) = match operation.as_str() {
        "exists" => (
            storage_artifact_exists(&path, Some(&backend)),
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        "read_text" => {
            let payload = storage_read_text(&path, Some(&backend))?;
            let digest = payload_sha256(&payload);
            let (limited_payload, is_truncated) = apply_read_limits(payload, max_bytes, tail_lines);
            let verified = expected_sha256
                .as_deref()
                .map(|expected| expected.eq_ignore_ascii_case(&digest));
            (
                true,
                Some(limited_payload.clone()),
                None,
                Some(limited_payload.len()),
                Some(digest),
                verified,
                Some(is_truncated),
            )
        }
        "verify_text" => {
            let expected = expected_sha256
                .or_else(|| payload_text.as_deref().map(payload_sha256))
                .ok_or_else(|| {
                    "runtime_storage verify_text requires expected_sha256 or payload_text"
                        .to_string()
                })?;
            if !storage_artifact_exists(&path, Some(&backend)) {
                (false, None, None, None, None, Some(false), None)
            } else {
                let payload = storage_read_text(&path, Some(&backend))?;
                let digest = payload_sha256(&payload);
                (
                    true,
                    None,
                    None,
                    None,
                    Some(digest.clone()),
                    Some(expected.eq_ignore_ascii_case(&digest)),
                    None,
                )
            }
        }
        "write_text" => {
            let payload = payload_text
                .ok_or_else(|| "runtime_storage write_text requires payload_text".to_string())?;
            let digest = payload_sha256(&payload);
            match &backend {
                ResolvedStorageBackend::Filesystem => filesystem_write_text(&path, &payload)?,
                ResolvedStorageBackend::Memory => {
                    let artifact_path = memory_artifact_path(&path)?;
                    if let Some(parent) = artifact_path.parent() {
                        fs::create_dir_all(parent).map_err(|err| {
                            format!(
                                "create memory storage parent directory failed for {}: {err}",
                                artifact_path.display()
                            )
                        })?;
                    }
                    fs::write(&artifact_path, payload.as_bytes()).map_err(|err| {
                        format!(
                            "write memory storage payload failed for {}: {err}",
                            path.display()
                        )
                    })?;
                }
                ResolvedStorageBackend::Sqlite { db_path, .. } => {
                    sqlite_write_text(&path, db_path, &constrained_storage_root, &payload)?
                }
            }
            (
                true,
                None,
                Some(payload.len()),
                None,
                Some(digest),
                None,
                None,
            )
        }
        "append_text" => {
            let payload = payload_text
                .ok_or_else(|| "runtime_storage append_text requires payload_text".to_string())?;
            let bytes_written = payload.len();
            match &backend {
                ResolvedStorageBackend::Filesystem => {
                    filesystem_append_text(&path, &payload)?;
                }
                ResolvedStorageBackend::Memory => {
                    let artifact_path = memory_artifact_path(&path)?;
                    if let Some(parent) = artifact_path.parent() {
                        fs::create_dir_all(parent).map_err(|err| {
                            format!(
                                "create memory storage parent directory failed for {}: {err}",
                                artifact_path.display()
                            )
                        })?;
                    }
                    let mut file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&artifact_path)
                        .map_err(|err| {
                            format!(
                                "open memory storage payload for append failed for {}: {err}",
                                path.display()
                            )
                        })?;
                    file.write_all(payload.as_bytes()).map_err(|err| {
                        format!(
                            "append memory storage payload failed for {}: {err}",
                            path.display()
                        )
                    })?;
                }
                ResolvedStorageBackend::Sqlite { db_path, .. } => {
                    sqlite_append_text(&path, db_path, &constrained_storage_root, &payload)?;
                }
            }
            let digest = digest_after_append_text(&path, &backend, &constrained_storage_root)?;
            (true, None, Some(bytes_written), None, digest, None, None)
        }
        other => return Err(format!("unsupported runtime_storage operation: {other:?}")),
    };

    Ok(RuntimeStorageResponsePayload {
        schema_version: RUNTIME_STORAGE_SCHEMA_VERSION.to_string(),
        authority: RUNTIME_STORAGE_AUTHORITY.to_string(),
        operation,
        path: path.display().to_string(),
        backend_family,
        sqlite_db_path,
        storage_root,
        backend_capabilities: runtime_backend_capabilities_payload(&request.backend_family)?,
        exists,
        payload_text: resolved_payload_text,
        bytes_written,
        bytes_returned,
        payload_sha256: payload_digest,
        verified,
        truncated,
    })
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("router-rs-{prefix}-{nonce}"));
        fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    #[test]
    fn effective_storage_root_does_not_silently_consult_codex_or_cursor_home() {
        // Regression: an earlier revision had `effective_storage_root_for_request`
        // fall back to `CODEX_HOME` / `CURSOR_HOME` when the caller did not
        // pin a `storage_root`. That made codex-CLI processes write to
        // `~/.codex/...` instead of cwd for relative paths — a silent
        // breaking change. Only `ROUTER_RS_STORAGE_ROOT` (an explicit
        // router-rs-only knob) is allowed as an env-driven fallback.
        let prior_router = std::env::var("ROUTER_RS_STORAGE_ROOT").ok();
        let prior_codex = std::env::var("CODEX_HOME").ok();
        let prior_cursor = std::env::var("CURSOR_HOME").ok();
        std::env::remove_var("ROUTER_RS_STORAGE_ROOT");
        std::env::set_var("CODEX_HOME", "/tmp/router-rs-test-codex-home");
        std::env::set_var("CURSOR_HOME", "/tmp/router-rs-test-cursor-home");
        let request = RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: "artifacts/x.json".to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: None,
            payload_text: None,
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let resolved = effective_storage_root_for_request(&request);
        assert!(
            resolved.is_none(),
            "CODEX_HOME / CURSOR_HOME must NOT be silently used as storage_root, got: {resolved:?}"
        );

        // Sanity: explicit ROUTER_RS_STORAGE_ROOT IS honored.
        std::env::set_var("ROUTER_RS_STORAGE_ROOT", "/tmp/router-rs-test-explicit");
        let resolved = effective_storage_root_for_request(&request);
        assert_eq!(resolved.as_deref(), Some("/tmp/router-rs-test-explicit"));

        // Cleanup test env so we don't leak state to other tests.
        match prior_router {
            Some(v) => std::env::set_var("ROUTER_RS_STORAGE_ROOT", v),
            None => std::env::remove_var("ROUTER_RS_STORAGE_ROOT"),
        }
        match prior_codex {
            Some(v) => std::env::set_var("CODEX_HOME", v),
            None => std::env::remove_var("CODEX_HOME"),
        }
        match prior_cursor {
            Some(v) => std::env::set_var("CURSOR_HOME", v),
            None => std::env::remove_var("CURSOR_HOME"),
        }
    }

    #[test]
    fn runtime_storage_allows_write_inside_storage_root() {
        let root = unique_temp_dir("runtime-storage-inside-root");
        let request = RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: "artifacts/output.json".to_string(),
            backend_family: "memory".to_string(),
            sqlite_db_path: None,
            storage_root: Some(root.display().to_string()),
            payload_text: Some("{\"ok\":true}".to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let response =
            runtime_storage_operation(request).expect("write within storage root should pass");
        assert!(response.exists);
        assert_eq!(response.bytes_written, Some("{\"ok\":true}".len()));
    }

    #[test]
    fn runtime_storage_rejects_parent_escape_path() {
        let root = unique_temp_dir("runtime-storage-parent-escape");
        let request = RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: "../escape.txt".to_string(),
            backend_family: "memory".to_string(),
            sqlite_db_path: None,
            storage_root: Some(root.display().to_string()),
            payload_text: Some("nope".to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let error = runtime_storage_operation(request).expect_err("parent escape must be rejected");
        assert!(error.contains("must stay under storage root"));
    }

    #[test]
    fn runtime_storage_rejects_absolute_path_outside_storage_root_for_sqlite() {
        let root = unique_temp_dir("runtime-storage-absolute-reject");
        let db_path = root.join("runtime.sqlite3");
        let outside = std::env::temp_dir().join("runtime-storage-outside.txt");
        let request = RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: outside.display().to_string(),
            backend_family: "sqlite".to_string(),
            sqlite_db_path: Some(db_path.display().to_string()),
            storage_root: Some(root.display().to_string()),
            payload_text: Some("nope".to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let error =
            runtime_storage_operation(request).expect_err("absolute outside root must be rejected");
        assert!(error.contains("must stay under storage root"));
    }

    /// Simulates a collision on the first `create_new` temp name; second attempt must succeed.
    #[test]
    fn filesystem_write_text_retries_temp_on_create_new_collision() {
        let root = unique_temp_dir("runtime-storage-temp-collision");
        let target = root.join("payload.txt");
        let file_name = target
            .file_name()
            .and_then(|n| n.to_str())
            .expect("file name");
        let parent = target.parent().expect("parent");
        let nanos = 9_876_543_210u128;
        let pid = std::process::id();
        let blocking_tmp = super::filesystem_atomic_temp_path(parent, file_name, nanos, pid, 0);
        fs::write(&blocking_tmp, b"block").expect("seed blocking temp");

        super::filesystem_write_text_inner(&target, "ok", nanos)
            .expect("write should retry past first temp collision");
        let body = fs::read_to_string(&target).expect("read back");
        assert_eq!(body, "ok");
        assert_eq!(
            fs::read_to_string(&blocking_tmp).expect("blocking file remains"),
            "block"
        );
        let _ = fs::remove_file(&target);
        let _ = fs::remove_file(&blocking_tmp);
    }

    #[cfg(unix)]
    #[test]
    fn runtime_storage_filesystem_rejects_symlink_write_path() {
        use std::os::unix::fs::symlink;

        let root = unique_temp_dir("runtime-storage-symlink-reject");
        let real = root.join("real.txt");
        fs::write(&real, b"x").expect("real file");
        let alias = root.join("alias.txt");
        let _ = fs::remove_file(&alias);
        symlink(&real, &alias).expect("create symlink");

        let request = RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: "alias.txt".to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: Some(root.display().to_string()),
            payload_text: Some("y".to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let err = runtime_storage_operation(request).expect_err("symlink path must be rejected");
        assert!(
            err.contains("must not be a symlink"),
            "unexpected error: {err}"
        );
    }

    /// A directory symlink in the parent chain that points outside the
    /// configured `storage_root` must not be traversable for writes. The
    /// lexical containment check passes (`<inside>/escape/leak.txt` still
    /// appears to live under `<inside>`), so this regression is only caught
    /// once the resolver compares canonicalized real paths.
    #[cfg(unix)]
    #[test]
    fn runtime_storage_rejects_parent_dir_symlink_escape() {
        use std::os::unix::fs::symlink;

        let outside = unique_temp_dir("runtime-storage-parent-symlink-outside");
        let inside = unique_temp_dir("runtime-storage-parent-symlink-inside");
        let link = inside.join("escape");
        let _ = fs::remove_file(&link);
        let _ = fs::remove_dir_all(&link);
        symlink(&outside, &link).expect("create dir symlink");

        let request = RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: "escape/leak.txt".to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: Some(inside.display().to_string()),
            payload_text: Some("leak".to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let err = runtime_storage_operation(request)
            .expect_err("parent-chain symlink escape must be rejected");
        assert!(
            err.contains("must stay under storage root"),
            "unexpected error: {err}"
        );
        assert!(
            err.contains("after symlink resolution"),
            "should be the canonical-path branch of the check, got: {err}"
        );
        let leaked = outside.join("leak.txt");
        assert!(
            !leaked.exists(),
            "no payload should have been written to the symlink target"
        );
    }

    /// Append must also honor the canonical containment check. We allow a
    /// pre-existing payload at the canonical real path, then try to append
    /// through a parent symlink that points outside `storage_root`. The
    /// resolver must reject the request before any append is attempted.
    #[cfg(unix)]
    #[test]
    fn runtime_storage_append_rejects_parent_dir_symlink_escape() {
        use std::os::unix::fs::symlink;

        let outside = unique_temp_dir("runtime-storage-append-symlink-outside");
        let inside = unique_temp_dir("runtime-storage-append-symlink-inside");
        let link = inside.join("escape");
        let _ = fs::remove_file(&link);
        let _ = fs::remove_dir_all(&link);
        symlink(&outside, &link).expect("create dir symlink");

        let prepared = outside.join("leak.txt");
        fs::write(&prepared, b"pre").expect("seed outside payload");

        let request = RuntimeStorageRequestPayload {
            operation: "append_text".to_string(),
            path: "escape/leak.txt".to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: Some(inside.display().to_string()),
            payload_text: Some("-leak".to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let err = runtime_storage_operation(request)
            .expect_err("append via parent-chain symlink escape must be rejected");
        assert!(
            err.contains("must stay under storage root"),
            "unexpected error: {err}"
        );
        let body = fs::read_to_string(&prepared).expect("outside payload remains");
        assert_eq!(body, "pre", "append must not have leaked into outside");
    }

    /// When the sqlite backend is selected without an explicit `storage_root`,
    /// the resolver must fall back to `sqlite_db_path.parent()` (historical
    /// semantics) instead of the process working directory.
    #[test]
    fn runtime_storage_sqlite_default_root_uses_db_parent() {
        let root = unique_temp_dir("runtime-storage-sqlite-default-root");
        let db_path = root.join("default.sqlite3");
        let canonical_root = root.canonicalize().expect("canonicalize root");
        let canonical_root_string = canonical_root.display().to_string();

        let write = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: "default.json".to_string(),
            backend_family: "sqlite".to_string(),
            sqlite_db_path: Some(db_path.display().to_string()),
            storage_root: None,
            payload_text: Some("{\"default\":true}".to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        })
        .expect("write should succeed via sqlite default storage_root");

        let resolved_root = write
            .storage_root
            .as_deref()
            .map(PathBuf::from)
            .map(|path| path.canonicalize().unwrap_or(path));
        assert_eq!(
            resolved_root.map(|p| p.display().to_string()),
            Some(canonical_root_string.clone()),
            "sqlite default storage_root should resolve to db parent"
        );
        assert_eq!(
            write.sqlite_db_path.as_deref(),
            Some(db_path.display().to_string().as_str())
        );
        assert!(
            db_path.exists(),
            "sqlite db should be created next to its parent"
        );

        let read = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "read_text".to_string(),
            path: "default.json".to_string(),
            backend_family: "sqlite".to_string(),
            sqlite_db_path: Some(db_path.display().to_string()),
            storage_root: None,
            payload_text: None,
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        })
        .expect("read should succeed via sqlite default storage_root");
        assert_eq!(read.payload_text.as_deref(), Some("{\"default\":true}"));
    }

    #[test]
    fn runtime_storage_append_text_returns_post_append_digest_for_all_backends() {
        let root = unique_temp_dir("runtime-storage-append-digest-parity");
        let rel_path = "runtime/payload.txt";
        let initial = "hello";
        let append = "-world";
        let expected = format!("{initial}{append}");
        let expected_digest = payload_sha256(&expected);

        for backend in ["filesystem", "memory", "sqlite"] {
            let backend_root = root.join(format!("backend-{backend}"));
            fs::create_dir_all(&backend_root).expect("create backend root");
            let db_path = backend_root.join("runtime.sqlite3");

            let write = RuntimeStorageRequestPayload {
                operation: "write_text".to_string(),
                path: rel_path.to_string(),
                backend_family: backend.to_string(),
                sqlite_db_path: (backend == "sqlite").then(|| db_path.display().to_string()),
                storage_root: Some(backend_root.display().to_string()),
                payload_text: Some(initial.to_string()),
                expected_sha256: None,
                max_bytes: None,
                tail_lines: None,
            };
            runtime_storage_operation(write).expect("seed write succeeds");

            let append_request = RuntimeStorageRequestPayload {
                operation: "append_text".to_string(),
                path: rel_path.to_string(),
                backend_family: backend.to_string(),
                sqlite_db_path: (backend == "sqlite").then(|| db_path.display().to_string()),
                storage_root: Some(backend_root.display().to_string()),
                payload_text: Some(append.to_string()),
                expected_sha256: None,
                max_bytes: None,
                tail_lines: None,
            };
            let append_response =
                runtime_storage_operation(append_request).expect("append request succeeds");
            assert_eq!(append_response.bytes_written, Some(append.len()));
            assert_eq!(
                append_response.payload_sha256.as_deref(),
                Some(expected_digest.as_str())
            );

            let read = RuntimeStorageRequestPayload {
                operation: "read_text".to_string(),
                path: rel_path.to_string(),
                backend_family: backend.to_string(),
                sqlite_db_path: (backend == "sqlite").then(|| db_path.display().to_string()),
                storage_root: Some(backend_root.display().to_string()),
                payload_text: None,
                expected_sha256: None,
                max_bytes: None,
                tail_lines: None,
            };
            let read_response = runtime_storage_operation(read).expect("read request succeeds");
            assert_eq!(
                read_response.payload_text.as_deref(),
                Some(expected.as_str())
            );
        }
    }

    #[test]
    fn runtime_storage_append_digest_uses_selected_backend_payload() {
        let root = unique_temp_dir("runtime-storage-append-digest-backend-selected");
        let rel_path = "runtime/payload.txt";
        let full_path = root.join(rel_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&full_path, "shadow-filesystem").expect("seed shadow filesystem payload");

        let write = RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: rel_path.to_string(),
            backend_family: "memory".to_string(),
            sqlite_db_path: None,
            storage_root: Some(root.display().to_string()),
            payload_text: Some("mem".to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        runtime_storage_operation(write).expect("memory write succeeds");

        let append_payload = "-append";
        let append = RuntimeStorageRequestPayload {
            operation: "append_text".to_string(),
            path: rel_path.to_string(),
            backend_family: "memory".to_string(),
            sqlite_db_path: None,
            storage_root: Some(root.display().to_string()),
            payload_text: Some(append_payload.to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let response = runtime_storage_operation(append).expect("memory append succeeds");
        let expected_mem = format!("mem{append_payload}");
        assert_eq!(
            response.payload_sha256.as_deref(),
            Some(payload_sha256(&expected_mem).as_str())
        );
        assert_ne!(
            response.payload_sha256.as_deref(),
            Some(payload_sha256("shadow-filesystem").as_str())
        );
    }

    #[test]
    fn runtime_storage_append_text_rejects_missing_payload_text() {
        let root = unique_temp_dir("runtime-storage-append-missing-payload");
        let request = RuntimeStorageRequestPayload {
            operation: "append_text".to_string(),
            path: "payload.txt".to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: Some(root.display().to_string()),
            payload_text: None,
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        };
        let err = runtime_storage_operation(request).expect_err("missing payload must fail");
        assert!(err.contains("append_text requires payload_text"));
    }

    #[test]
    fn runtime_storage_sqlite_append_isolated_by_storage_root() {
        let root = unique_temp_dir("runtime-storage-sqlite-append-isolation");
        let db_path = root.join("shared.sqlite3");
        let rel_path = "session/data.log";

        let session_a = root.join("session-a");
        let session_b = root.join("session-b");
        fs::create_dir_all(&session_a).expect("create session_a");
        fs::create_dir_all(&session_b).expect("create session_b");

        for (storage_root, write_body, append_body) in
            [(&session_a, "a0", "-a1"), (&session_b, "b0", "-b1")]
        {
            let write = RuntimeStorageRequestPayload {
                operation: "write_text".to_string(),
                path: rel_path.to_string(),
                backend_family: "sqlite".to_string(),
                sqlite_db_path: Some(db_path.display().to_string()),
                storage_root: Some(storage_root.display().to_string()),
                payload_text: Some(write_body.to_string()),
                expected_sha256: None,
                max_bytes: None,
                tail_lines: None,
            };
            runtime_storage_operation(write).expect("sqlite write succeeds");
            let append = RuntimeStorageRequestPayload {
                operation: "append_text".to_string(),
                path: rel_path.to_string(),
                backend_family: "sqlite".to_string(),
                sqlite_db_path: Some(db_path.display().to_string()),
                storage_root: Some(storage_root.display().to_string()),
                payload_text: Some(append_body.to_string()),
                expected_sha256: None,
                max_bytes: None,
                tail_lines: None,
            };
            runtime_storage_operation(append).expect("sqlite append succeeds");
        }

        let read_a = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "read_text".to_string(),
            path: rel_path.to_string(),
            backend_family: "sqlite".to_string(),
            sqlite_db_path: Some(db_path.display().to_string()),
            storage_root: Some(session_a.display().to_string()),
            payload_text: None,
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        })
        .expect("session_a read succeeds");
        let read_b = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "read_text".to_string(),
            path: rel_path.to_string(),
            backend_family: "sqlite".to_string(),
            sqlite_db_path: Some(db_path.display().to_string()),
            storage_root: Some(session_b.display().to_string()),
            payload_text: None,
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        })
        .expect("session_b read succeeds");

        assert_eq!(read_a.payload_text.as_deref(), Some("a0-a1"));
        assert_eq!(read_b.payload_text.as_deref(), Some("b0-b1"));
    }

    #[test]
    fn runtime_storage_read_text_supports_tail_lines_and_max_bytes() {
        let root = unique_temp_dir("runtime-storage-read-limits");
        let rel_path = "logs/runtime.log";
        let payload = "line-1\nline-2\nline-3\nline-4\n";
        runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: rel_path.to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: Some(root.display().to_string()),
            payload_text: Some(payload.to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        })
        .expect("write succeeds");

        let response = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "read_text".to_string(),
            path: rel_path.to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: Some(root.display().to_string()),
            payload_text: None,
            expected_sha256: None,
            max_bytes: Some(8),
            tail_lines: Some(2),
        })
        .expect("limited read succeeds");
        let expected_digest = payload_sha256(payload);
        assert_eq!(response.payload_text.as_deref(), Some("line-4\n"));
        assert_eq!(response.bytes_returned, Some("line-4\n".len()));
        assert_eq!(response.truncated, Some(true));
        assert_eq!(
            response.payload_sha256.as_deref(),
            Some(expected_digest.as_str())
        );
    }

    #[cfg(unix)]
    #[test]
    fn runtime_storage_filesystem_append_does_not_require_read_after_write() {
        use std::os::unix::fs::PermissionsExt;

        let root = unique_temp_dir("runtime-storage-append-no-readback");
        let rel_path = "logs/runtime.log";
        let absolute = root.join(rel_path);
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&absolute, b"seed").expect("seed payload");
        fs::set_permissions(&absolute, fs::Permissions::from_mode(0o200))
            .expect("set write-only permission");

        let response = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "append_text".to_string(),
            path: rel_path.to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: Some(root.display().to_string()),
            payload_text: Some("-tail".to_string()),
            expected_sha256: None,
            max_bytes: None,
            tail_lines: None,
        })
        .expect("append should not require read permission");
        assert_eq!(response.bytes_written, Some("-tail".len()));
        assert_eq!(response.payload_sha256, None);
    }
}

fn default_service_delegate_kind(service_name: &str, backend_family: &str) -> String {
    let normalized_backend = backend_family.trim().to_lowercase().replace('_', "-");
    format!("{normalized_backend}-{service_name}-store")
}

fn capability_bool(capabilities: &Map<String, Value>, field: &str, default: bool) -> bool {
    capabilities
        .get(field)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

fn path_value(paths: &Map<String, Value>, field: &str) -> Value {
    match paths.get(field) {
        Some(Value::String(value)) => Value::String(value.clone()),
        _ => Value::Null,
    }
}

fn build_service_projection_for_backend(
    control_plane_descriptor: Option<&Value>,
    service_name: &str,
    backend_family: &str,
    default_authority: &str,
    default_role: &str,
    default_projection: &str,
) -> Value {
    let descriptor = control_plane_descriptor.and_then(Value::as_object);
    let services = descriptor
        .and_then(|value| value.get("services"))
        .and_then(Value::as_object);
    let service = services
        .and_then(|value| value.get(service_name))
        .and_then(Value::as_object);

    let authority = service
        .and_then(|value| value.get("authority"))
        .and_then(Value::as_str)
        .unwrap_or(default_authority);
    let role = service
        .and_then(|value| value.get("role"))
        .and_then(Value::as_str)
        .unwrap_or(default_role);
    let projection = service
        .and_then(|value| value.get("projection"))
        .and_then(Value::as_str)
        .unwrap_or(default_projection);
    let delegate_kind = service
        .and_then(|value| value.get("delegate_kind"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| default_service_delegate_kind(service_name, backend_family));

    json!({
        "authority": authority,
        "role": role,
        "projection": projection,
        "delegate_kind": delegate_kind,
    })
}

pub(crate) fn build_checkpoint_control_plane_compiler_payload(
    payload: Value,
) -> Result<Value, String> {
    let control_plane_descriptor = payload.get("control_plane_descriptor");
    let paths = payload
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| "runtime checkpoint control plane requires paths".to_string())?;
    let capabilities = payload
        .get("capabilities")
        .and_then(Value::as_object)
        .ok_or_else(|| "runtime checkpoint control plane requires capabilities".to_string())?;
    let raw_backend_family = capabilities
        .get("backend_family")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "runtime checkpoint control plane capabilities must include backend_family".to_string()
        })?;
    let backend_capabilities = runtime_backend_capabilities(raw_backend_family)?;
    let backend_family = backend_capabilities.backend_family;
    let parity = runtime_backend_family_parity_payload(
        capabilities
            .get("store_backend_family")
            .and_then(Value::as_str),
        capabilities
            .get("checkpointer_backend_family")
            .and_then(Value::as_str)
            .or(Some(raw_backend_family)),
        capabilities
            .get("trace_backend_family")
            .and_then(Value::as_str),
        capabilities
            .get("state_backend_family")
            .and_then(Value::as_str),
    )?;
    if parity.get("aligned").and_then(Value::as_bool) != Some(true) {
        return Err(format!(
            "runtime checkpoint control plane backend family mismatch: {}",
            parity
                .get("mismatch_reason")
                .and_then(Value::as_str)
                .unwrap_or("backend families are not aligned")
        ));
    }

    let runtime_control_plane = build_runtime_control_plane_payload();
    let default_runtime_authority = runtime_control_plane
        .get("authority")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_TRACE_SERVICE_AUTHORITY);

    let descriptor = json!({
        "schema_version": RUNTIME_CHECKPOINT_CONTROL_PLANE_SCHEMA_VERSION,
        "runtime_control_plane_schema_version": control_plane_descriptor
            .and_then(|value| value.get("schema_version"))
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        "runtime_control_plane_authority": control_plane_descriptor
            .and_then(|value| value.get("authority"))
            .and_then(Value::as_str)
            .unwrap_or(default_runtime_authority),
        "trace_service": build_service_projection_for_backend(
            control_plane_descriptor,
            "trace",
            backend_family,
            DEFAULT_TRACE_SERVICE_AUTHORITY,
            DEFAULT_TRACE_SERVICE_ROLE,
            DEFAULT_TRACE_SERVICE_PROJECTION,
        ),
        "state_service": build_service_projection_for_backend(
            control_plane_descriptor,
            "state",
            backend_family,
            DEFAULT_STATE_SERVICE_AUTHORITY,
            DEFAULT_STATE_SERVICE_ROLE,
            DEFAULT_STATE_SERVICE_PROJECTION,
        ),
        "backend_family": backend_family,
        "supports_atomic_replace": capability_bool(
            capabilities,
            "supports_atomic_replace",
            backend_capabilities.supports_atomic_replace,
        ),
        "supports_compaction": capability_bool(
            capabilities,
            "supports_compaction",
            backend_capabilities.supports_compaction,
        ),
        "supports_snapshot_delta": capability_bool(
            capabilities,
            "supports_snapshot_delta",
            backend_capabilities.supports_snapshot_delta,
        ),
        "supports_remote_event_transport": capability_bool(
            capabilities,
            "supports_remote_event_transport",
            backend_capabilities.supports_remote_event_transport,
        ),
        "supports_consistent_append": capability_bool(
            capabilities,
            "supports_consistent_append",
            backend_capabilities.supports_consistent_append,
        ),
        "supports_sqlite_wal": capability_bool(
            capabilities,
            "supports_sqlite_wal",
            backend_capabilities.supports_sqlite_wal,
        ),
        "backend_family_catalog": runtime_backend_family_catalog_payload(),
        "backend_family_parity": parity,
        "trace_output_path": path_value(paths, "trace_output_path"),
        "event_stream_path": path_value(paths, "event_stream_path"),
        "resume_manifest_path": path_value(paths, "resume_manifest_path"),
        "background_state_path": required_non_empty_string(
            &Value::Object(paths.clone()),
            "background_state_path",
            "runtime checkpoint control plane",
        )?,
        "event_transport_dir": required_non_empty_string(
            &Value::Object(paths.clone()),
            "event_transport_dir",
            "runtime checkpoint control plane",
        )?,
    });

    Ok(json!({
        "schema_version": RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_SCHEMA_VERSION,
        "authority": RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_AUTHORITY,
        "checkpoint_control_plane": descriptor,
    }))
}
