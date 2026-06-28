use super::control_plane::normalized_backend_family;
use super::types::BackgroundStateStore;
use super::types::*;
use crate::{SQLITE_TABLE_NAME, acquire_runtime_path_lock};
use core_errors::FrameworkError;
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub(super) fn read_persisted_state(
    state_path: &Path,
    backend_family: &str,
    sqlite_db_path: Option<&Path>,
    state_payload_text: Option<&str>,
) -> Result<Option<PersistedBackgroundState>, FrameworkError> {
    match normalized_backend_family(backend_family).as_str() {
        "filesystem" | "file" => {
            if !state_path.is_file() {
                return Ok(None);
            }
            let text = fs::read_to_string(state_path).map_err(FrameworkError::Io)?;
            let persisted = serde_json::from_str::<PersistedBackgroundState>(&text)
                .map_err(FrameworkError::Json)?;
            Ok(Some(persisted))
        }
        "memory" => {
            let Some(text) = state_payload_text else {
                return Ok(None);
            };
            let persisted = serde_json::from_str::<PersistedBackgroundState>(text)
                .map_err(FrameworkError::Json)?;
            Ok(Some(persisted))
        }
        "sqlite" | "sqlite3" => {
            let Some(db_path) = sqlite_db_path else {
                return Err(FrameworkError::validation(
                    "SQLite background state request is missing sqlite_db_path.",
                ));
            };
            if !db_path.exists() {
                return Ok(None);
            }
            let storage_root = state_path.parent().ok_or_else(|| {
                FrameworkError::validation(
                    "Background state path is missing a parent directory.",
                )
            })?;
            let stable_key = sqlite_storage_key(storage_root, state_path)?;
            let legacy_key = state_path
                .canonicalize()
                .unwrap_or_else(|_| state_path.to_path_buf());
            let conn = open_sqlite_connection(db_path)?;
            let row: Option<String> = conn
                .query_row(
                    &format!("SELECT payload_text FROM {SQLITE_TABLE_NAME} WHERE payload_key = ?1"),
                    params![stable_key],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| FrameworkError::validation(e.to_string()))?
                .or_else(|| {
                    conn.query_row(
                        &format!(
                            "SELECT payload_text FROM {SQLITE_TABLE_NAME} WHERE payload_key = ?1"
                        ),
                        params![legacy_key.to_string_lossy().to_string()],
                        |row| row.get(0),
                    )
                    .optional()
                    .ok()
                    .flatten()
                });
            let Some(text) = row else {
                return Ok(None);
            };
            let persisted = serde_json::from_str::<PersistedBackgroundState>(&text)
                .map_err(FrameworkError::Json)?;
            Ok(Some(persisted))
        }
        other => Err(FrameworkError::unsupported(format!(
            "Unsupported durable background-state backend family: {:?}",
            other
        ))),
    }
}

pub(super) fn write_persisted_state(
    state_path: &Path,
    backend_family: &str,
    sqlite_db_path: Option<&Path>,
    payload: &str,
) -> Result<(), FrameworkError> {
    match normalized_backend_family(backend_family).as_str() {
        "filesystem" | "file" => {
            if let Some(parent) = state_path.parent() {
                fs::create_dir_all(parent).map_err(FrameworkError::Io)?;
            }
            let tmp_path = state_path.with_extension(
                state_path
                    .extension()
                    .map(|value| format!("{}.tmp", value.to_string_lossy()))
                    .unwrap_or_else(|| "tmp".to_string()),
            );
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(FrameworkError::Io)?;
            use std::io::Write;
            file.write_all(payload.as_bytes())
                .map_err(FrameworkError::Io)?;
            file.sync_all().map_err(FrameworkError::Io)?;
            fs::rename(&tmp_path, state_path).map_err(FrameworkError::Io)?;
            Ok(())
        }
        "sqlite" | "sqlite3" => {
            let Some(db_path) = sqlite_db_path else {
                return Err(FrameworkError::validation(
                    "SQLite background state request is missing sqlite_db_path.",
                ));
            };
            let storage_root = state_path.parent().ok_or_else(|| {
                FrameworkError::validation(
                    "Background state path is missing a parent directory.",
                )
            })?;
            let payload_key = sqlite_storage_key(storage_root, state_path)?;
            let conn = open_sqlite_connection(db_path)?;
            conn.execute(
                &format!(
                    "INSERT INTO {SQLITE_TABLE_NAME} (payload_key, payload_text) VALUES (?1, ?2)
                     ON CONFLICT(payload_key) DO UPDATE SET payload_text = excluded.payload_text"
                ),
                params![payload_key, payload],
            )
            .map_err(|e| FrameworkError::validation(e.to_string()))?;
            Ok(())
        }
        other => Err(FrameworkError::unsupported(format!(
            "Unsupported durable background-state backend family: {:?}",
            other
        ))),
    }
}

pub(super) fn sqlite_storage_key(
    storage_root: &Path,
    state_path: &Path,
) -> Result<String, FrameworkError> {
    let resolved_root = storage_root
        .canonicalize()
        .unwrap_or_else(|_| storage_root.to_path_buf());
    let resolved_state = if state_path.exists() {
        state_path
            .canonicalize()
            .unwrap_or_else(|_| state_path.to_path_buf())
    } else {
        state_path.to_path_buf()
    };
    let relative = resolved_state.strip_prefix(&resolved_root).map_err(|_| {
        FrameworkError::validation(format!(
            "SQLite background state path {} must stay under storage root {}",
            resolved_state.display(),
            resolved_root.display()
        ))
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

/// Re-export shared SQLite connection — delegates to `runtime_storage::sqlite::sqlite_connection`.
/// Maintains the `pub(super)` API for `background_state` internal callers.
pub(super) use crate::runtime_storage::sqlite_connection as open_sqlite_connection;

/// Returns true for operations that mutate the persisted store. Used by
/// `handle_background_state_operation` to decide whether to flush the
/// in-memory reaper cleanup as part of this operation's persist step
/// (mutating) versus deferring it to the next mutation (read-only).
pub(super) fn is_mutating_background_operation(op: &str) -> bool {
    matches!(
        op,
        "apply_mutation" | "arbitrate_session_takeover" | "reserve" | "claim" | "release"
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn handle_background_state_operation(payload: Value) -> Result<Value, FrameworkError> {
    let request =
        serde_json::from_value::<BackgroundStateRequestPayload>(payload).map_err(|err| {
            FrameworkError::validation(format!("parse background state request failed: {err}"))
        })?;
    if request.schema_version != BACKGROUND_STATE_REQUEST_SCHEMA_VERSION {
        return Err(FrameworkError::validation(format!(
            "unknown background state request schema_version: {}",
            request.schema_version
        )));
    }
    // Cross-process critical section: durable-backed state must serialize
    // load -> mutate -> persist so concurrent writers (codex+cursor+tests)
    // cannot clobber each other. We acquire an advisory lock on a sentinel
    // file keyed by `state_path` for both filesystem and sqlite backends:
    //   - filesystem: serializes the load+rename cycle on the JSON file.
    //   - sqlite: serializes the load+UPDATE cycle on the row keyed by
    //     `state_path`. SQLite's own row-level locking only protects a
    //     single SQL statement, not our higher-level read-modify-write
    //     compound operation; without this sentinel two concurrent
    //     handlers that both load, both reap, both mutate could interleave
    //     and lose updates. The sentinel file is independent of the sqlite
    //     db file, so it does not interfere with SQLite's own locks.
    // Memory backend has no cross-process surface and skips the advisory
    // lock entirely.
    let backend_family = request.backend_family.as_deref().unwrap_or("filesystem");
    let normalized_backend = normalized_backend_family(backend_family);
    let needs_path_lock = matches!(
        normalized_backend.as_str(),
        "filesystem" | "file" | "sqlite" | "sqlite3"
    );
    let _path_lock = if needs_path_lock {
        match request.state_path.as_deref() {
            Some(p) => Some(acquire_runtime_path_lock(Path::new(p))?),
            None => None,
        }
    } else {
        None
    };
    let mut store = BackgroundStateStore::load(&request)?;
    // Mutating operations fold the in-memory reaper cleanup into their
    // persist step. Read-only operations leave the store as-is so reads
    // remain disk-side-effect-free.
    if is_mutating_background_operation(&request.operation) {
        store.flush_reap_if_dirty();
    }
    let operation = request.operation.clone();
    let mut response = json!({
        "schema_version": BACKGROUND_STATE_STORE_SCHEMA_VERSION,
        "authority": BACKGROUND_STATE_STORE_AUTHORITY,
        "operation": operation,
        "state": store.snapshot_payload(),
        "health": store.health_payload(),
    });
    match request.operation.as_str() {
        "snapshot" => {}
        "apply_mutation" => {
            apply_mutation_handler(&mut store, &request, &mut response)?;
        }
        "get" => {
            get_handler(&store, &request, &mut response)?;
        }
        "get_active_job" => {
            get_active_job_handler(&store, &request, &mut response)?;
        }
        "arbitrate_session_takeover" => {
            arbitrate_session_takeover_handler(&mut store, &request, &mut response)?;
        }
        "reserve" | "claim" | "release" => {
            reserve_claim_release_handler(&mut store, &request, &mut response)?;
        }
        "parallel_group_summary" => {
            parallel_group_summary_handler(&store, &request, &mut response)?;
        }
        "parallel_group_summaries" => {
            parallel_group_summaries_handler(&store, &mut response)?;
        }
        "health" => {}
        other => {
            return Err(FrameworkError::unsupported(format!(
                "unsupported background state operation: {:?}",
                other
            )));
        }
    }
    Ok(response)
}

// ── Per-operation handlers ──

fn apply_mutation_handler(
    store: &mut BackgroundStateStore,
    request: &BackgroundStateRequestPayload,
    response: &mut Value,
) -> Result<(), FrameworkError> {
    let job_id = request
        .job_id
        .as_deref()
        .ok_or_else(|| FrameworkError::validation("Background state apply_mutation is missing job_id."))?;
    let mutation = request.mutation.as_ref().ok_or_else(|| {
        FrameworkError::validation("Background state apply_mutation is missing mutation.")
    })?;
    let (job, persisted_payload_text) = store.apply_mutation(job_id, mutation)?;
    response["job"] = serde_json::to_value(job).map_err(FrameworkError::Json)?;
    if let Some(payload_text) = persisted_payload_text {
        response["persisted_payload_text"] = Value::String(payload_text);
    }
    response["state"] = store.snapshot_payload();
    response["health"] = store.health_payload();
    Ok(())
}

fn get_handler(
    store: &BackgroundStateStore,
    request: &BackgroundStateRequestPayload,
    response: &mut Value,
) -> Result<(), FrameworkError> {
    let job_id = request
        .job_id
        .as_deref()
        .ok_or_else(|| FrameworkError::validation("Background state get is missing job_id."))?;
    response["job"] = store
        .get(job_id)
        .map(|job| serde_json::to_value(job).map_err(FrameworkError::Json))
        .transpose()?
        .unwrap_or(Value::Null);
    Ok(())
}

fn get_active_job_handler(
    store: &BackgroundStateStore,
    request: &BackgroundStateRequestPayload,
    response: &mut Value,
) -> Result<(), FrameworkError> {
    let session_id = request
        .session_id
        .as_deref()
        .ok_or_else(|| FrameworkError::validation("Background state get_active_job is missing session_id."))?;
    response["active_job_id"] = store
        .active_job(session_id)
        .map(Value::String)
        .unwrap_or(Value::Null);
    Ok(())
}

fn arbitrate_session_takeover_handler(
    store: &mut BackgroundStateStore,
    request: &BackgroundStateRequestPayload,
    response: &mut Value,
) -> Result<(), FrameworkError> {
    let arbitration_operation = request
        .arbitration_operation
        .as_deref()
        .ok_or_else(|| {
            FrameworkError::validation(
                "Background state arbitration is missing arbitration_operation.",
            )
        })?;
    let session_id = request
        .session_id
        .as_deref()
        .ok_or_else(|| FrameworkError::validation("Background state arbitration is missing session_id."))?;
    let incoming_job_id = request
        .incoming_job_id
        .as_deref()
        .ok_or_else(|| {
            FrameworkError::validation("Background state arbitration is missing incoming_job_id.")
        })?;
    let (takeover, persisted_payload_text) =
        store.arbitrate_session_takeover(arbitration_operation, session_id, incoming_job_id)?;
    response["takeover"] = serde_json::to_value(takeover).map_err(FrameworkError::Json)?;
    if let Some(payload_text) = persisted_payload_text {
        response["persisted_payload_text"] = Value::String(payload_text);
    }
    response["state"] = store.snapshot_payload();
    response["health"] = store.health_payload();
    Ok(())
}

fn reserve_claim_release_handler(
    store: &mut BackgroundStateStore,
    request: &BackgroundStateRequestPayload,
    response: &mut Value,
) -> Result<(), FrameworkError> {
    let session_id = request
        .session_id
        .as_deref()
        .ok_or_else(|| FrameworkError::validation("Background state operation is missing session_id."))?;
    let incoming_job_id = request
        .incoming_job_id
        .as_deref()
        .ok_or_else(|| {
            FrameworkError::validation("Background state operation is missing incoming_job_id.")
        })?;
    let (takeover, persisted_payload_text) =
        store.arbitrate_session_takeover(&request.operation, session_id, incoming_job_id)?;
    response["takeover"] = serde_json::to_value(takeover).map_err(FrameworkError::Json)?;
    if let Some(payload_text) = persisted_payload_text {
        response["persisted_payload_text"] = Value::String(payload_text);
    }
    response["state"] = store.snapshot_payload();
    response["health"] = store.health_payload();
    Ok(())
}

fn parallel_group_summary_handler(
    store: &BackgroundStateStore,
    request: &BackgroundStateRequestPayload,
    response: &mut Value,
) -> Result<(), FrameworkError> {
    let parallel_group_id = request
        .parallel_group_id
        .as_deref()
        .ok_or_else(|| {
            FrameworkError::validation(
                "Background state parallel_group_summary is missing parallel_group_id.",
            )
        })?;
    response["parallel_group_summary"] = store
        .parallel_group_summary(parallel_group_id)
        .map(|summary| serde_json::to_value(summary).map_err(FrameworkError::Json))
        .transpose()?
        .unwrap_or(Value::Null);
    Ok(())
}

fn parallel_group_summaries_handler(
    store: &BackgroundStateStore,
    response: &mut Value,
) -> Result<(), FrameworkError> {
    response["parallel_group_summaries"] =
        serde_json::to_value(store.parallel_group_summaries()).map_err(FrameworkError::Json)?;
    Ok(())
}