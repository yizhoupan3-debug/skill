use super::control_plane::normalized_backend_family;
use super::types::BackgroundStateStore;
use super::types::*;
use crate::{SQLITE_TABLE_NAME, acquire_runtime_path_lock};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn read_persisted_state(
    state_path: &Path,
    backend_family: &str,
    sqlite_db_path: Option<&Path>,
    state_payload_text: Option<&str>,
) -> Result<Option<PersistedBackgroundState>, String> {
    match normalized_backend_family(backend_family).as_str() {
        "filesystem" | "file" => {
            if !state_path.is_file() {
                return Ok(None);
            }
            let text = fs::read_to_string(state_path).map_err(|err| err.to_string())?;
            let persisted = serde_json::from_str::<PersistedBackgroundState>(&text)
                .map_err(|err| err.to_string())?;
            Ok(Some(persisted))
        }
        "memory" => {
            let Some(text) = state_payload_text else {
                return Ok(None);
            };
            let persisted = serde_json::from_str::<PersistedBackgroundState>(text)
                .map_err(|err| err.to_string())?;
            Ok(Some(persisted))
        }
        "sqlite" | "sqlite3" => {
            let Some(db_path) = sqlite_db_path else {
                return Err(
                    "SQLite background state request is missing sqlite_db_path.".to_string()
                );
            };
            if !db_path.exists() {
                return Ok(None);
            }
            let storage_root = state_path.parent().ok_or_else(|| {
                "Background state path is missing a parent directory.".to_string()
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
                .map_err(|err| err.to_string())?
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
                .map_err(|err| err.to_string())?;
            Ok(Some(persisted))
        }
        other => Err(format!(
            "Unsupported durable background-state backend family: {:?}",
            other
        )),
    }
}

pub(super) fn write_persisted_state(
    state_path: &Path,
    backend_family: &str,
    sqlite_db_path: Option<&Path>,
    payload: &str,
) -> Result<(), String> {
    match normalized_backend_family(backend_family).as_str() {
        "filesystem" | "file" => {
            if let Some(parent) = state_path.parent() {
                fs::create_dir_all(parent).map_err(|err| err.to_string())?;
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
                .map_err(|err| err.to_string())?;
            use std::io::Write;
            file.write_all(payload.as_bytes())
                .map_err(|err| err.to_string())?;
            file.sync_all().map_err(|err| err.to_string())?;
            fs::rename(&tmp_path, state_path).map_err(|err| err.to_string())?;
            Ok(())
        }
        "sqlite" | "sqlite3" => {
            let Some(db_path) = sqlite_db_path else {
                return Err(
                    "SQLite background state request is missing sqlite_db_path.".to_string()
                );
            };
            let storage_root = state_path.parent().ok_or_else(|| {
                "Background state path is missing a parent directory.".to_string()
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
            .map_err(|err| err.to_string())?;
            Ok(())
        }
        other => Err(format!(
            "Unsupported durable background-state backend family: {:?}",
            other
        )),
    }
}

pub(super) fn sqlite_storage_key(storage_root: &Path, state_path: &Path) -> Result<String, String> {
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
        format!(
            "SQLite background state path {} must stay under storage root {}",
            resolved_state.display(),
            resolved_root.display()
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

pub(super) fn open_sqlite_connection(db_path: &Path) -> Result<std::rc::Rc<Connection>, String> {
    use std::cell::RefCell;
    use std::rc::Rc;
    thread_local! {
        static CACHED: RefCell<Option<(PathBuf, Rc<Connection>)>> = const { RefCell::new(None) };
    }
    CACHED.with(|cell| {
        let mut slot = cell.borrow_mut();
        if let Some((ref cached_path, ref cached_conn)) = *slot
            && cached_path == db_path {
                return Ok(Rc::clone(cached_conn));
            }
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let conn = Connection::open(db_path).map_err(|err| err.to_string())?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|err| err.to_string())?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|err| err.to_string())?;
        conn.execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS {SQLITE_TABLE_NAME} (
                    payload_key TEXT PRIMARY KEY,
                    payload_text TEXT NOT NULL
                )"
            ),
            [],
        )
        .map_err(|err| err.to_string())?;
        let shared = Rc::new(conn);
        let result = Rc::clone(&shared);
        *slot = Some((db_path.to_path_buf(), shared));
        Ok(result)
    })
}

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
pub fn handle_background_state_operation(payload: Value) -> Result<Value, String> {
    let request = serde_json::from_value::<BackgroundStateRequestPayload>(payload)
        .map_err(|err| format!("parse background state request failed: {err}"))?;
    if request.schema_version != BACKGROUND_STATE_REQUEST_SCHEMA_VERSION {
        return Err(format!(
            "unknown background state request schema_version: {}",
            request.schema_version
        ));
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
            let job_id = request
                .job_id
                .as_deref()
                .ok_or_else(|| "Background state apply_mutation is missing job_id.".to_string())?;
            let mutation = request.mutation.as_ref().ok_or_else(|| {
                "Background state apply_mutation is missing mutation.".to_string()
            })?;
            let (job, persisted_payload_text) = store.apply_mutation(job_id, mutation)?;
            response["job"] = serde_json::to_value(job).map_err(|err| err.to_string())?;
            if let Some(payload_text) = persisted_payload_text {
                response["persisted_payload_text"] = Value::String(payload_text);
            }
            response["state"] = store.snapshot_payload();
            response["health"] = store.health_payload();
        }
        "get" => {
            let job_id = request
                .job_id
                .as_deref()
                .ok_or_else(|| "Background state get is missing job_id.".to_string())?;
            response["job"] = store
                .get(job_id)
                .map(|job| serde_json::to_value(job).map_err(|err| err.to_string()))
                .transpose()?
                .unwrap_or(Value::Null);
        }
        "get_active_job" => {
            let session_id = request.session_id.as_deref().ok_or_else(|| {
                "Background state get_active_job is missing session_id.".to_string()
            })?;
            response["active_job_id"] = store
                .active_job(session_id)
                .map(Value::String)
                .unwrap_or(Value::Null);
        }
        "arbitrate_session_takeover" => {
            let arbitration_operation =
                request.arbitration_operation.as_deref().ok_or_else(|| {
                    "Background state arbitration is missing arbitration_operation.".to_string()
                })?;
            let session_id = request
                .session_id
                .as_deref()
                .ok_or_else(|| "Background state arbitration is missing session_id.".to_string())?;
            let incoming_job_id = request.incoming_job_id.as_deref().ok_or_else(|| {
                "Background state arbitration is missing incoming_job_id.".to_string()
            })?;
            let (takeover, persisted_payload_text) = store.arbitrate_session_takeover(
                arbitration_operation,
                session_id,
                incoming_job_id,
            )?;
            response["takeover"] = serde_json::to_value(takeover).map_err(|err| err.to_string())?;
            if let Some(payload_text) = persisted_payload_text {
                response["persisted_payload_text"] = Value::String(payload_text);
            }
            response["state"] = store.snapshot_payload();
            response["health"] = store.health_payload();
        }
        "reserve" | "claim" | "release" => {
            let session_id = request
                .session_id
                .as_deref()
                .ok_or_else(|| "Background state arbitration is missing session_id.".to_string())?;
            let incoming_job_id = request.incoming_job_id.as_deref().ok_or_else(|| {
                "Background state arbitration is missing incoming_job_id.".to_string()
            })?;
            let (takeover, persisted_payload_text) = store.arbitrate_session_takeover(
                &request.operation,
                session_id,
                incoming_job_id,
            )?;
            response["takeover"] = serde_json::to_value(takeover).map_err(|err| err.to_string())?;
            if let Some(payload_text) = persisted_payload_text {
                response["persisted_payload_text"] = Value::String(payload_text);
            }
            response["state"] = store.snapshot_payload();
            response["health"] = store.health_payload();
        }
        "parallel_group_summary" => {
            let parallel_group_id = request.parallel_group_id.as_deref().ok_or_else(|| {
                "Background state parallel_group_summary is missing parallel_group_id.".to_string()
            })?;
            response["parallel_group_summary"] = store
                .parallel_group_summary(parallel_group_id)
                .map(|summary| serde_json::to_value(summary).map_err(|err| err.to_string()))
                .transpose()?
                .unwrap_or(Value::Null);
        }
        "parallel_group_summaries" => {
            response["parallel_group_summaries"] =
                serde_json::to_value(store.parallel_group_summaries())
                    .map_err(|err| err.to_string())?;
        }
        "health" => {}
        other => {
            return Err(format!(
                "unsupported background state operation: {:?}",
                other
            ));
        }
    }
    Ok(response)
}
