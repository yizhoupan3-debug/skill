use serde_json::{json, Value};

const RUNTIME_BACKEND_FAMILY_CATALOG_SCHEMA_VERSION: &str =
    "runtime-persistence-backend-family-catalog-v1";
const RUNTIME_BACKEND_FAMILY_PARITY_SCHEMA_VERSION: &str =
    "runtime-persistence-backend-family-parity-v1";
pub const RUNTIME_CHECKPOINT_CONTROL_PLANE_COMPILER_AUTHORITY: &str =
    "rust-runtime-checkpoint-control-plane";

#[derive(Debug, Clone, Copy)]
pub struct RuntimeBackendCapabilities {
    pub backend_family: &'static str,
    pub supports_atomic_replace: bool,
    pub supports_compaction: bool,
    pub supports_snapshot_delta: bool,
    pub supports_remote_event_transport: bool,
    pub supports_consistent_append: bool,
    pub supports_sqlite_wal: bool,
}

pub fn runtime_backend_capabilities(
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

pub fn runtime_backend_capabilities_payload(backend_family: &str) -> Result<Value, String> {
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

pub fn runtime_backend_family_catalog_payload() -> Value {
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

pub fn runtime_backend_family_parity_payload(
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
pub fn normalized_backend_family(value: &str) -> String {
    value.trim().to_lowercase().replace('-', "_")
}
