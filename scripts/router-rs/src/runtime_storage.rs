use crate::{
    build_runtime_control_plane_payload, required_non_empty_string, RUNTIME_STORAGE_AUTHORITY,
    RUNTIME_STORAGE_SCHEMA_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub(crate) payload_sha256: Option<String>,
    pub(crate) verified: Option<bool>,
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
    if candidate.is_absolute() {
        return Ok(candidate);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(candidate))
        .map_err(|err| format!("resolve runtime storage path failed: {err}"))
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

fn sqlite_lookup_keys(path: &Path, storage_root: &Path) -> Result<(String, String), String> {
    let resolved_path = normalize_runtime_path(&path.display().to_string())?;
    let resolved_root = normalize_runtime_path(&storage_root.display().to_string())?;
    let stable_key = resolved_path
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
    let legacy_key = resolved_path.display().to_string();
    Ok((stable_key, legacy_key))
}

fn sqlite_payload_exists(path: &Path, db_path: &Path, storage_root: &Path) -> Result<bool, String> {
    let (stable_key, legacy_key) = sqlite_lookup_keys(path, storage_root)?;
    let conn = sqlite_connection(db_path)?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT 1 FROM {SQLITE_TABLE_NAME} WHERE payload_key = ?1 OR payload_key = ?2 LIMIT 1"
        ))
        .map_err(|err| format!("prepare sqlite exists query failed: {err}"))?;
    let exists = stmt
        .query_row(params![stable_key, legacy_key], |row| row.get::<_, i64>(0))
        .optional()
        .map_err(|err| format!("run sqlite exists query failed: {err}"))?
        .is_some();
    Ok(exists)
}

fn sqlite_read_text(path: &Path, db_path: &Path, storage_root: &Path) -> Result<String, String> {
    let (stable_key, legacy_key) = sqlite_lookup_keys(path, storage_root)?;
    let conn = sqlite_connection(db_path)?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT payload_text FROM {SQLITE_TABLE_NAME} WHERE payload_key = ?1 OR payload_key = ?2 LIMIT 1"
        ))
        .map_err(|err| format!("prepare sqlite read query failed: {err}"))?;
    stmt.query_row(params![stable_key, legacy_key], |row| {
        row.get::<_, String>(0)
    })
    .map_err(|err| format!("read sqlite payload failed for {}: {err}", path.display()))
}

fn sqlite_write_text(
    path: &Path,
    db_path: &Path,
    storage_root: &Path,
    payload_text: &str,
) -> Result<(), String> {
    let (stable_key, _) = sqlite_lookup_keys(path, storage_root)?;
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
    let (stable_key, _) = sqlite_lookup_keys(path, storage_root)?;
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

fn filesystem_write_text(path: &Path, payload_text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create runtime storage parent directory failed for {}: {err}",
                path.display()
            )
        })?;
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system time before unix epoch: {err}"))?
        .as_nanos();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime-storage");
    let tmp_path = path.with_file_name(format!(".{file_name}.tmp-{}-{nonce}", std::process::id()));
    fs::write(&tmp_path, payload_text.as_bytes()).map_err(|err| {
        format!(
            "write runtime storage temp payload failed for {}: {err}",
            tmp_path.display()
        )
    })?;
    fs::rename(&tmp_path, path).map_err(|err| {
        format!(
            "replace runtime storage payload failed for {}: {err}",
            path.display()
        )
    })?;
    Ok(())
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
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| {
            format!(
                "open runtime storage payload for append failed for {}: {err}",
                path.display()
            )
        })?;
    file.write_all(payload_text.as_bytes()).map_err(|err| {
        format!(
            "append runtime storage payload failed for {}: {err}",
            path.display()
        )
    })?;
    Ok(())
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
            let storage_root = match request.storage_root.clone() {
                Some(value) => normalize_runtime_path(&value)?,
                None => db_path.parent().map(Path::to_path_buf).ok_or_else(|| {
                    format!(
                        "runtime_storage sqlite db path {} must have a parent directory",
                        db_path.display()
                    )
                })?,
            };
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
    let path = normalize_runtime_path(&request.path)?;
    let (backend, backend_family, sqlite_db_path, storage_root) =
        resolve_runtime_storage_backend(&request)?;
    let operation = request.operation.trim().to_lowercase();
    let expected_sha256 = request.expected_sha256.clone();
    let payload_text = request.payload_text;

    let (exists, resolved_payload_text, bytes_written, payload_digest, verified) = match operation
        .as_str()
    {
        "exists" => (
            storage_artifact_exists(&path, Some(&backend)),
            None,
            None,
            None,
            None,
        ),
        "read_text" => {
            let payload = storage_read_text(&path, Some(&backend))?;
            let digest = payload_sha256(&payload);
            let verified = expected_sha256
                .as_deref()
                .map(|expected| expected.eq_ignore_ascii_case(&digest));
            (true, Some(payload), None, Some(digest), verified)
        }
        "verify_text" => {
            let expected = expected_sha256
                .or_else(|| payload_text.as_deref().map(payload_sha256))
                .ok_or_else(|| {
                    "runtime_storage verify_text requires expected_sha256 or payload_text"
                        .to_string()
                })?;
            if !storage_artifact_exists(&path, Some(&backend)) {
                (false, None, None, None, Some(false))
            } else {
                let payload = storage_read_text(&path, Some(&backend))?;
                let digest = payload_sha256(&payload);
                (
                    true,
                    None,
                    None,
                    Some(digest.clone()),
                    Some(expected.eq_ignore_ascii_case(&digest)),
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
                ResolvedStorageBackend::Sqlite {
                    db_path,
                    storage_root,
                } => sqlite_write_text(&path, db_path, storage_root, &payload)?,
            }
            (true, None, Some(payload.len()), Some(digest), None)
        }
        "append_text" => {
            let payload = payload_text
                .ok_or_else(|| "runtime_storage append_text requires payload_text".to_string())?;
            let bytes_written = payload.len();
            match &backend {
                ResolvedStorageBackend::Filesystem => filesystem_append_text(&path, &payload)?,
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
                ResolvedStorageBackend::Sqlite {
                    db_path,
                    storage_root,
                } => sqlite_append_text(&path, db_path, storage_root, &payload)?,
            }
            let stored_payload = storage_read_text(&path, Some(&backend))?;
            (
                true,
                None,
                Some(bytes_written),
                Some(payload_sha256(&stored_payload)),
                None,
            )
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
        payload_sha256: payload_digest,
        verified,
    })
}

fn default_service_delegate_kind(service_name: &str, backend_family: &str) -> String {
    let normalized_backend = backend_family.trim().to_lowercase().replace('_', "-");
    format!("{normalized_backend}-{service_name}-store")
}

fn coerce_legacy_service_delegate_kind(
    delegate_kind: &str,
    service_name: &str,
    backend_family: &str,
) -> String {
    let legacy_delegate = format!("filesystem-{service_name}-store");
    if backend_family == "filesystem" || delegate_kind != legacy_delegate {
        return delegate_kind.to_string();
    }
    default_service_delegate_kind(service_name, backend_family)
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
        .map(|value| coerce_legacy_service_delegate_kind(value, service_name, backend_family))
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
