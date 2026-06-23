use crate::runtime_envelope_ids::{
    RUNTIME_CONTROL_PLANE_AUTHORITY, RUNTIME_STORAGE_AUTHORITY, RUNTIME_STORAGE_SCHEMA_VERSION,
};
use framework_kernel::json_value::required_non_empty_string;
use super::backend::{
    RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_AUTHORITY, normalized_backend_family,
    runtime_backend_capabilities, runtime_backend_capabilities_payload,
    runtime_backend_family_catalog_payload, runtime_backend_family_parity_payload,
};
use super::filesystem::{
    MEMORY_APPEND_MUTEX, filesystem_append_text, filesystem_write_text, memory_artifact_path,
};
use super::paths::{
    effective_storage_root_for_request, normalize_runtime_path, payload_sha256,
    resolve_runtime_storage_path_with_root, stream_sha256_hex_path,
};
use super::sqlite::{
    env_checkpoint_storage_db_path, runtime_storage_db_name_candidates, sqlite_append_text,
    sqlite_payload_exists, sqlite_read_text, sqlite_write_text,
};
use super::{
    DEFAULT_STATE_SERVICE_AUTHORITY, DEFAULT_STATE_SERVICE_PROJECTION, DEFAULT_STATE_SERVICE_ROLE,
    ResolvedStorageBackend, RuntimeStorageRequestPayload, RuntimeStorageResponsePayload,
};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_SCHEMA_VERSION: &str =
    "router-rs-runtime-checkpoint-control-plane-v1";
const RUNTIME_CHECKPOINT_CONTROL_PLANE_SCHEMA_VERSION: &str = "runtime-checkpoint-control-plane-v1";
const DEFAULT_TRACE_SERVICE_AUTHORITY: &str = "rust-runtime-control-plane";
const DEFAULT_TRACE_SERVICE_ROLE: &str = "trace-and-handoff";
const DEFAULT_TRACE_SERVICE_PROJECTION: &str = "rust-native-projection";

#[tracing::instrument(level = "debug", skip_all)]
pub fn digest_after_append_text(
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
pub fn slice_tail_by_max_bytes(payload: &str, max_bytes: usize) -> String {
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

pub fn slice_tail_by_lines(payload: &str, tail_lines: usize) -> String {
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

pub fn apply_read_limits(
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn storage_artifact_exists(
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn storage_read_text(
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_storage_backend(paths: &[PathBuf]) -> Option<ResolvedStorageBackend> {
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
        if grandparent_name == Some("trace_compaction")
            && let Some(root) = grandparent.and_then(Path::parent) {
                candidates.push(root.to_path_buf());
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_runtime_storage_backend(
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn runtime_storage_operation(
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
                    let _memory_append_guard = MEMORY_APPEND_MUTEX
                        .get_or_init(|| Mutex::new(()))
                        .lock()
                        .map_err(|_| {
                            "runtime_storage memory append mutex poisoned (parallel append aborted)"
                                .to_string()
                        })?;
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
pub fn default_service_delegate_kind(service_name: &str, backend_family: &str) -> String {
    let normalized_backend = backend_family.trim().to_lowercase().replace('_', "-");
    format!("{normalized_backend}-{service_name}-store")
}

pub fn capability_bool(capabilities: &Map<String, Value>, field: &str, default: bool) -> bool {
    capabilities
        .get(field)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

pub fn path_value(paths: &Map<String, Value>, field: &str) -> Value {
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_checkpoint_control_plane_compiler_payload(payload: Value) -> Result<Value, String> {
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

    // Inlined: build_runtime_control_plane_payload().authority == RUNTIME_CONTROL_PLANE_AUTHORITY
    let default_runtime_authority = RUNTIME_CONTROL_PLANE_AUTHORITY;

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
        )
        .map_err(|e| e.to_string())?,
        "event_transport_dir": required_non_empty_string(
            &Value::Object(paths.clone()),
            "event_transport_dir",
            "runtime checkpoint control plane",
        )
        .map_err(|e| e.to_string())?,
    });

    Ok(json!({
        "schema_version": RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_SCHEMA_VERSION,
        "authority": RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_AUTHORITY,
        "checkpoint_control_plane": descriptor,
    }))
}
