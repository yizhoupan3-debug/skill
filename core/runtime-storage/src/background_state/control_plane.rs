use super::types::BACKGROUND_STATE_CONTROL_PLANE_SCHEMA_VERSION;
use crate::{
    DEFAULT_STATE_SERVICE_AUTHORITY, DEFAULT_STATE_SERVICE_PROJECTION, DEFAULT_STATE_SERVICE_ROLE,
    runtime_backend_capabilities,
};
use crate::runtime_storage::RuntimeBackendCapabilities;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn backend_capabilities(
    backend_family: &str,
) -> Result<RuntimeBackendCapabilities, String> {
    runtime_backend_capabilities(backend_family).map_err(|err| {
        format!("Unsupported durable background-state backend family: {err}")
    })
}

pub(super) fn normalized_backend_family(value: &str) -> String {
    value.trim().to_lowercase().replace('-', "_")
}

pub(super) fn background_delegate_kind(backend_family: &str) -> String {
    format!(
        "{}-state-store",
        backend_family.trim().to_lowercase().replace('_', "-")
    )
}

pub(super) fn build_state_control_plane(
    control_plane_descriptor: Option<&Value>,
    backend_family: &str,
    state_path: &Path,
) -> Result<Value, String> {
    let normalized_backend = normalized_backend_family(backend_family);
    let capabilities = backend_capabilities(&normalized_backend)?;
    let runtime_caps = runtime_backend_capabilities(&normalized_backend)?;
    let mut payload = json!({
        "schema_version": BACKGROUND_STATE_CONTROL_PLANE_SCHEMA_VERSION,
        "runtime_control_plane_schema_version": control_plane_descriptor
            .and_then(|value| value.get("schema_version"))
            .cloned()
            .unwrap_or(Value::Null),
        "runtime_control_plane_authority": control_plane_descriptor
            .and_then(|value| value.get("authority"))
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_STATE_SERVICE_AUTHORITY),
        "service": "state",
        "authority": DEFAULT_STATE_SERVICE_AUTHORITY,
        "role": DEFAULT_STATE_SERVICE_ROLE,
        "projection": DEFAULT_STATE_SERVICE_PROJECTION,
        "delegate_kind": background_delegate_kind(&normalized_backend),
        "transport_family": "checkpoint-artifact",
        "health_family": "runtime-health",
        "backend_family": normalized_backend,
        "supports_atomic_replace": capabilities.supports_atomic_replace,
        "supports_compaction": capabilities.supports_compaction,
        "supports_snapshot_delta": capabilities.supports_snapshot_delta,
        "supports_remote_event_transport": capabilities.supports_remote_event_transport,
        "supports_consistent_append": runtime_caps.supports_consistent_append,
        "supports_sqlite_wal": runtime_caps.supports_sqlite_wal,
        "state_path": state_path.to_string_lossy(),
    });
    if let Some(Value::Object(descriptor)) = control_plane_descriptor
        && let Some(Value::Object(services)) = descriptor.get("services")
            && let Some(Value::Object(service)) = services.get("state") {
                for field in ["authority", "role", "projection", "delegate_kind"] {
                    if let Some(value) = service.get(field) {
                        payload[field] = value.clone();
                    }
                }
            }
    if payload.get("delegate_kind").and_then(Value::as_str) == Some("filesystem-state-store")
        && normalized_backend != "filesystem"
    {
        payload["delegate_kind"] = Value::String(background_delegate_kind(&normalized_backend));
    }
    Ok(payload)
}
