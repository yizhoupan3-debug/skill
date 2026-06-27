//! Runtime event transport / handoff / checkpoint resume manifest descriptors.
//!
//! Also includes write-payload helpers moved from `cli/runtime_ops.inc`
//! to break the `cli ↔ framework_runtime` cyclic dependency.

use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::path::Path;

use fr_utils::io_utils::validate_write_path;
use rt_storage::runtime_envelope_ids::{
    CHECKPOINT_RESUME_MANIFEST_AUTHORITY, CHECKPOINT_RESUME_MANIFEST_SCHEMA_VERSION,
    CHECKPOINT_MANIFEST_WRITE_AUTHORITY, CHECKPOINT_MANIFEST_WRITE_SCHEMA_VERSION,
    RUNTIME_CONTROL_PLANE_AUTHORITY, TRACE_DESCRIPTOR_AUTHORITY, TRACE_DESCRIPTOR_SCHEMA_VERSION,
    TRANSPORT_BINDING_WRITE_AUTHORITY, TRANSPORT_BINDING_WRITE_SCHEMA_VERSION,
};

use fr_utils::json_value::{
    nested_bool, nested_non_empty_string, optional_bool, optional_non_empty_string,
    required_non_empty_string,
};

fn build_attach_target_payload(
    session_id: &str,
    job_id: Option<&str>,
    endpoint_kind: &str,
    subscribe_method: &str,
    describe_method: &str,
    cleanup_method: &str,
    handoff_method: &str,
) -> Value {
    json!({
        "endpoint_kind": endpoint_kind,
        "subscribe_method": subscribe_method,
        "describe_method": describe_method,
        "cleanup_method": cleanup_method,
        "handoff_method": handoff_method,
        "session_id": session_id,
        "job_id": job_id,
    })
}

fn build_replay_anchor_payload(
    latest_cursor: Value,
    resume_mode: &str,
    replay_supported: bool,
) -> Value {
    json!({
        "anchor_kind": "trace_replay_cursor",
        "cursor_schema_version": "runtime-trace-cursor-v1",
        "resume_mode": resume_mode,
        "latest_cursor": latest_cursor,
        "replay_supported": replay_supported,
    })
}

fn build_transport_health_payload(payload: &Value) -> Value {
    if payload.get("transport_health").is_some() {
        return payload
            .get("transport_health")
            .cloned()
            .unwrap_or(Value::Null);
    }
    if payload.get("control_plane").is_none() {
        return Value::Null;
    }
    json!({
        "backend_family": nested_non_empty_string(payload, &["control_plane", "backend_family"]),
        "supports_atomic_replace": nested_bool(payload, &["control_plane", "supports_atomic_replace"]),
        "supports_compaction": nested_bool(payload, &["control_plane", "supports_compaction"]),
        "supports_snapshot_delta": nested_bool(payload, &["control_plane", "supports_snapshot_delta"]),
        "supports_remote_event_transport": nested_bool(payload, &["control_plane", "supports_remote_event_transport"]),
    })
}

fn build_trace_transport_payload(
    payload: &Value,
    session_id: String,
    job_id: Option<String>,
) -> Value {
    let stream_key = job_id.as_deref().unwrap_or(session_id.as_str());
    let endpoint_kind = optional_non_empty_string(payload, "endpoint_kind")
        .unwrap_or_else(|| "runtime_method".to_string());
    let subscribe_method = optional_non_empty_string(payload, "subscribe_method")
        .unwrap_or_else(|| "subscribe_runtime_events".to_string());
    let describe_method = optional_non_empty_string(payload, "describe_method")
        .unwrap_or_else(|| "describe_runtime_event_transport".to_string());
    let cleanup_method = optional_non_empty_string(payload, "cleanup_method")
        .unwrap_or_else(|| "cleanup_runtime_events".to_string());
    let handoff_method = optional_non_empty_string(payload, "handoff_method")
        .unwrap_or_else(|| "describe_runtime_event_handoff".to_string());
    let resume_mode = optional_non_empty_string(payload, "resume_mode")
        .unwrap_or_else(|| "after_event_id".to_string());
    let replay_supported = optional_bool(payload, "replay_supported").unwrap_or(true);
    let latest_cursor = payload.get("latest_cursor").cloned().unwrap_or(Value::Null);
    let binding_backend_family = optional_non_empty_string(payload, "binding_backend_family")
        .or_else(|| nested_non_empty_string(payload, &["control_plane", "backend_family"]));
    let control_plane_authority = optional_non_empty_string(payload, "control_plane_authority")
        .or_else(|| {
            nested_non_empty_string(payload, &["control_plane", "trace_service", "authority"])
        });
    let control_plane_role = optional_non_empty_string(payload, "control_plane_role")
        .or_else(|| nested_non_empty_string(payload, &["control_plane", "trace_service", "role"]));
    let control_plane_projection = optional_non_empty_string(payload, "control_plane_projection")
        .or_else(|| {
            nested_non_empty_string(payload, &["control_plane", "trace_service", "projection"])
        });
    let control_plane_delegate_kind =
        optional_non_empty_string(payload, "control_plane_delegate_kind").or_else(|| {
            nested_non_empty_string(
                payload,
                &["control_plane", "trace_service", "delegate_kind"],
            )
        });
    let attach_target = build_attach_target_payload(
        &session_id,
        job_id.as_deref(),
        &endpoint_kind,
        &subscribe_method,
        &describe_method,
        &cleanup_method,
        &handoff_method,
    );
    let replay_anchor =
        build_replay_anchor_payload(latest_cursor.clone(), &resume_mode, replay_supported);

    json!({
        "schema_version": "runtime-event-transport-v1",
        "stream_id": format!("stream::{stream_key}"),
        "session_id": session_id,
        "job_id": job_id,
        "transport_contract_kind": optional_non_empty_string(payload, "transport_contract_kind").unwrap_or_else(|| "runtime_event_stream".to_string()),
        "transport_family": optional_non_empty_string(payload, "transport_family").unwrap_or_else(|| "host-facing-transport".to_string()),
        "transport_kind": optional_non_empty_string(payload, "transport_kind").unwrap_or_else(|| "poll".to_string()),
        "endpoint_kind": endpoint_kind,
        "ownership_lane": optional_non_empty_string(payload, "ownership_lane").unwrap_or_else(|| "rust-contract-lane".to_string()),
        "producer_owner": optional_non_empty_string(payload, "producer_owner").unwrap_or_else(|| "rust-control-plane".to_string()),
        "producer_authority": optional_non_empty_string(payload, "producer_authority").unwrap_or_else(|| RUNTIME_CONTROL_PLANE_AUTHORITY.to_string()),
        "exporter_owner": optional_non_empty_string(payload, "exporter_owner").unwrap_or_else(|| "rust-control-plane".to_string()),
        "exporter_authority": optional_non_empty_string(payload, "exporter_authority").unwrap_or_else(|| RUNTIME_CONTROL_PLANE_AUTHORITY.to_string()),
        "remote_capable": optional_bool(payload, "remote_capable")
            .or_else(|| nested_bool(payload, &["control_plane", "supports_remote_event_transport"]))
            .unwrap_or(true),
        "remote_attach_supported": optional_bool(payload, "remote_attach_supported")
            .or_else(|| nested_bool(payload, &["control_plane", "supports_remote_event_transport"]))
            .unwrap_or(true),
        "handoff_supported": optional_bool(payload, "handoff_supported").unwrap_or(true),
        "handoff_method": handoff_method,
        "subscribe_method": subscribe_method,
        "cleanup_method": cleanup_method,
        "describe_method": describe_method,
        "handoff_kind": optional_non_empty_string(payload, "handoff_kind").unwrap_or_else(|| "artifact_handoff".to_string()),
        "binding_refresh_mode": optional_non_empty_string(payload, "binding_refresh_mode").unwrap_or_else(|| "describe_or_checkpoint".to_string()),
        "binding_artifact_format": optional_non_empty_string(payload, "binding_artifact_format").unwrap_or_else(|| "json".to_string()),
        "binding_backend_family": binding_backend_family,
        "binding_artifact_path": optional_non_empty_string(payload, "binding_artifact_path"),
        "resume_mode": resume_mode,
        "heartbeat_supported": optional_bool(payload, "heartbeat_supported").unwrap_or(true),
        "cleanup_semantics": optional_non_empty_string(payload, "cleanup_semantics").unwrap_or_else(|| "stream_cache_only".to_string()),
        "cleanup_preserves_replay": optional_bool(payload, "cleanup_preserves_replay").unwrap_or(true),
        "replay_reseed_supported": optional_bool(payload, "replay_reseed_supported").unwrap_or(true),
        "chunk_schema_version": optional_non_empty_string(payload, "chunk_schema_version").unwrap_or_else(|| "runtime-event-stream-v1".to_string()),
        "cursor_schema_version": optional_non_empty_string(payload, "cursor_schema_version").unwrap_or_else(|| "runtime-trace-cursor-v1".to_string()),
        "latest_cursor": latest_cursor,
        "replay_supported": replay_supported,
        "attach_target": payload.get("attach_target").cloned().unwrap_or(attach_target),
        "replay_anchor": payload.get("replay_anchor").cloned().unwrap_or(replay_anchor),
        "control_plane_authority": control_plane_authority,
        "control_plane_role": control_plane_role,
        "control_plane_projection": control_plane_projection,
        "control_plane_delegate_kind": control_plane_delegate_kind,
        "transport_health": build_transport_health_payload(payload),
    })
}

pub fn build_trace_transport_descriptor(payload: Value) -> Result<Value, FrameworkError> {
    let session_id = required_non_empty_string(&payload, "session_id", "describe transport")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    Ok(json!({
        "schema_version": TRACE_DESCRIPTOR_SCHEMA_VERSION,
        "authority": TRACE_DESCRIPTOR_AUTHORITY,
        "transport": build_trace_transport_payload(&payload, session_id, job_id),
    }))
}

pub fn build_trace_handoff_descriptor(payload: Value) -> Result<Value, FrameworkError> {
    let session_id = required_non_empty_string(&payload, "session_id", "describe handoff")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    let transport_source = payload.get("transport").cloned().unwrap_or(Value::Null);
    let transport_session_id = optional_non_empty_string(&transport_source, "session_id")
        .unwrap_or_else(|| session_id.clone());
    let transport_job_id =
        optional_non_empty_string(&transport_source, "job_id").or_else(|| job_id.clone());
    let transport = if transport_source.is_object() {
        build_trace_transport_payload(
            &transport_source,
            transport_session_id.clone(),
            transport_job_id.clone(),
        )
    } else {
        build_trace_transport_payload(&payload, session_id.clone(), job_id.clone())
    };
    let checkpoint_backend_family =
        optional_non_empty_string(&payload, "checkpoint_backend_family")
            .or_else(|| optional_non_empty_string(&transport, "binding_backend_family"))
            .or_else(|| nested_non_empty_string(&payload, &["control_plane", "backend_family"]))
            .unwrap_or_else(|| "filesystem".to_string());
    let trace_stream_path = optional_non_empty_string(&payload, "trace_stream_path");
    let resume_manifest_path = optional_non_empty_string(&payload, "resume_manifest_path");
    let recovery_artifacts = payload
        .get("recovery_artifacts")
        .cloned()
        .filter(Value::is_array)
        .unwrap_or_else(|| {
            let mut ordered: Vec<String> = Vec::new();
            if let Some(path) = optional_non_empty_string(&transport, "binding_artifact_path") {
                ordered.push(path);
            }
            if let Some(path) = resume_manifest_path.clone() {
                ordered.push(path);
            }
            if let Some(path) = trace_stream_path.clone() {
                ordered.push(path);
            }
            json!(ordered)
        });

    Ok(json!({
        "schema_version": TRACE_DESCRIPTOR_SCHEMA_VERSION,
        "authority": TRACE_DESCRIPTOR_AUTHORITY,
        "handoff": {
            "schema_version": "runtime-event-handoff-v1",
            "stream_id": transport.get("stream_id").cloned().unwrap_or_else(|| Value::String(format!("stream::{}", transport_job_id.as_deref().unwrap_or(transport_session_id.as_str())))),
            "session_id": session_id,
            "job_id": job_id,
            "checkpoint_backend_family": checkpoint_backend_family,
            "trace_stream_path": trace_stream_path,
            "resume_manifest_path": resume_manifest_path,
            "remote_attach_strategy": optional_non_empty_string(&payload, "remote_attach_strategy").unwrap_or_else(|| "transport_descriptor_then_replay".to_string()),
            "cleanup_preserves_replay": transport.get("cleanup_preserves_replay").and_then(Value::as_bool).unwrap_or(true),
            "attach_target": transport.get("attach_target").cloned().unwrap_or(Value::Null),
            "replay_anchor": transport.get("replay_anchor").cloned().unwrap_or(Value::Null),
            "recovery_artifacts": recovery_artifacts,
            "control_plane": payload.get("control_plane").cloned().unwrap_or(Value::Null),
            "transport": transport,
        },
    }))
}

pub fn build_checkpoint_resume_manifest(payload: Value) -> Result<Value, FrameworkError> {
    let session_id =
        required_non_empty_string(&payload, "session_id", "checkpoint resume manifest")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    let status =
        optional_non_empty_string(&payload, "status").unwrap_or_else(|| "running".to_string());
    let generation = payload
        .get("generation")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let mut resume_manifest = json!({
        "schema_version": "runtime-resume-manifest-v1",
        "session_id": session_id,
        "job_id": job_id,
        "status": status,
        "generation": generation,
        "trace_output_path": optional_non_empty_string(&payload, "trace_output_path"),
        "trace_stream_path": optional_non_empty_string(&payload, "trace_stream_path"),
        "event_transport_path": optional_non_empty_string(&payload, "event_transport_path"),
        "background_state_path": optional_non_empty_string(&payload, "background_state_path"),
        "latest_cursor": payload.get("latest_cursor").cloned().unwrap_or(Value::Null),
        "artifact_paths": payload
            .get("artifact_paths")
            .cloned()
            .filter(Value::is_array)
            .unwrap_or_else(|| json!([])),
        "parallel_group": payload.get("parallel_group").cloned().unwrap_or(Value::Null),
        "supervisor_projection": payload.get("supervisor_projection").cloned().unwrap_or(Value::Null),
        "control_plane": payload.get("control_plane").cloned().unwrap_or(Value::Null),
    });
    if let Some(updated_at) = optional_non_empty_string(&payload, "updated_at")
        && let Some(map) = resume_manifest.as_object_mut() {
            map.insert("updated_at".to_string(), Value::String(updated_at));
        }
    Ok(json!({
        "schema_version": CHECKPOINT_RESUME_MANIFEST_SCHEMA_VERSION,
        "authority": CHECKPOINT_RESUME_MANIFEST_AUTHORITY,
        "resume_manifest": resume_manifest,
    }))
}

// ── Write payload helpers (moved from `cli/runtime_ops.inc`) ──

fn write_json_payload(path: &Path, payload: &Value) -> Result<usize, FrameworkError> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|err| FrameworkError::validation(format!("serialize persisted payload failed: {err}")))?
    );
    write_text_payload(path, &serialized)
}

pub fn write_transport_binding_payload(payload: Value) -> Result<Value, FrameworkError> {
    let path = required_non_empty_string(&payload, "path", "write transport binding")?;
    let session_id = required_non_empty_string(&payload, "session_id", "write transport binding")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    let transport = build_trace_transport_payload(&payload, session_id, job_id);
    let bytes_written = write_json_payload(Path::new(&path), &transport)?;
    Ok(json!({
        "schema_version": TRANSPORT_BINDING_WRITE_SCHEMA_VERSION,
        "authority": TRANSPORT_BINDING_WRITE_AUTHORITY,
        "path": path,
        "bytes_written": bytes_written,
    }))
}

pub fn write_checkpoint_resume_manifest_payload(payload: Value) -> Result<Value, FrameworkError> {
    let path = required_non_empty_string(&payload, "path", "write checkpoint resume manifest")?;
    let manifest = build_checkpoint_resume_manifest(payload)?
        .get("resume_manifest")
        .cloned()
        .ok_or_else(|| {
            FrameworkError::validation(
                "checkpoint resume manifest payload missing resume_manifest".to_string(),
            )
        })?;
    let bytes_written = write_json_payload(Path::new(&path), &manifest)?;
    Ok(json!({
        "schema_version": CHECKPOINT_MANIFEST_WRITE_SCHEMA_VERSION,
        "authority": CHECKPOINT_MANIFEST_WRITE_AUTHORITY,
        "path": path,
        "bytes_written": bytes_written,
    }))
}

pub fn write_text_payload(path: &Path, payload: &str) -> Result<usize, FrameworkError> {
    validate_write_path(path, None)?;
    let bytes = payload.len();
    core_state_utils::atomic_write::write_atomic_text(path, payload)?;
    Ok(bytes)
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── build_trace_transport_payload ──

    #[test]
    fn describe_transport_requires_session_id() {
        let err = build_trace_transport_descriptor(json!({})).expect_err("missing session_id");
        assert!(err.to_string().contains("session_id"));
    }

    #[test]
    fn transport_payload_defaults_poll_runtime_method() {
        let payload = build_trace_transport_payload(&json!({}), "sess-1".to_string(), None);
        assert_eq!(
            payload.get("transport_kind").and_then(Value::as_str),
            Some("poll")
        );
        assert_eq!(
            payload.get("stream_id").and_then(Value::as_str),
            Some("stream::sess-1")
        );
    }

    #[test]
    fn transport_payload_with_job_id_uses_job_stream_key() {
        let payload =
            build_trace_transport_payload(&json!({}), "sess-1".to_string(), Some("job-42".to_string()));
        assert_eq!(
            payload.get("stream_id").and_then(Value::as_str),
            Some("stream::job-42")
        );
    }

    #[test]
    fn transport_payload_overrides_defaults_from_input() {
        let input = json!({
            "transport_kind": "websocket",
            "endpoint_kind": "custom_rpc",
            "subscribe_method": "sub_events",
            "describe_method": "desc_events",
            "cleanup_method": "clean_events",
            "handoff_method": "handoff_events",
            "resume_mode": "from_start",
            "replay_supported": false,
        });
        let payload = build_trace_transport_payload(&input, "sess-1".to_string(), None);
        assert_eq!(payload["transport_kind"], "websocket");
        assert_eq!(payload["endpoint_kind"], "custom_rpc");
        assert_eq!(payload["subscribe_method"], "sub_events");
        assert_eq!(payload["describe_method"], "desc_events");
        assert_eq!(payload["cleanup_method"], "clean_events");
        assert_eq!(payload["handoff_method"], "handoff_events");
        assert_eq!(payload["resume_mode"], "from_start");
        assert_eq!(payload["replay_supported"], false);
    }

    #[test]
    fn transport_payload_inherits_control_plane_backend() {
        let input = json!({
            "control_plane": {
                "backend_family": "s3",
                "supports_atomic_replace": true,
                "supports_compaction": true,
                "supports_snapshot_delta": false,
                "supports_remote_event_transport": true,
            }
        });
        let payload = build_trace_transport_payload(&input, "sess-1".to_string(), None);
        assert_eq!(payload["binding_backend_family"], "s3");
        assert_eq!(payload["remote_capable"], true);
        assert_eq!(payload["remote_attach_supported"], true);
        assert!(payload.get("transport_health").is_some());
        let health = &payload["transport_health"];
        assert_eq!(health["backend_family"], "s3");
    }

    #[test]
    fn transport_payload_uses_attach_target_fallback_when_missing() {
        let payload = build_trace_transport_payload(&json!({}), "sess-1".to_string(), None);
        let attach = &payload["attach_target"];
        assert_eq!(attach["session_id"], "sess-1");
        assert_eq!(attach["endpoint_kind"], "runtime_method");
        assert_eq!(attach["subscribe_method"], "subscribe_runtime_events");
    }

    #[test]
    fn transport_payload_uses_provided_attach_target() {
        let input = json!({
            "attach_target": {"session_id": "custom", "endpoint_kind": "grpc"}
        });
        let payload = build_trace_transport_payload(&input, "sess-1".to_string(), None);
        assert_eq!(payload["attach_target"]["session_id"], "custom");
        assert_eq!(payload["attach_target"]["endpoint_kind"], "grpc");
    }

    #[test]
    fn transport_payload_replay_anchor_with_cursor() {
        let input = json!({
            "latest_cursor": {"event_id": "evt-99", "offset": 100},
            "resume_mode": "after_event_id",
        });
        let payload = build_trace_transport_payload(&input, "sess-1".to_string(), None);
        let anchor = &payload["replay_anchor"];
        assert_eq!(anchor["anchor_kind"], "trace_replay_cursor");
        assert_eq!(anchor["resume_mode"], "after_event_id");
        assert_eq!(anchor["latest_cursor"]["event_id"], "evt-99");
    }

    #[test]
    fn transport_payload_control_plane_trace_service_fields() {
        let input = json!({
            "control_plane": {
                "trace_service": {
                    "authority": "trace-auth",
                    "role": "observer",
                    "projection": "proj-x",
                    "delegate_kind": "fanout",
                }
            }
        });
        let payload = build_trace_transport_payload(&input, "sess-1".to_string(), None);
        assert_eq!(payload["control_plane_authority"], "trace-auth");
        assert_eq!(payload["control_plane_role"], "observer");
        assert_eq!(payload["control_plane_projection"], "proj-x");
        assert_eq!(payload["control_plane_delegate_kind"], "fanout");
    }

    #[test]
    fn transport_payload_health_noop_when_no_health_and_no_control_plane() {
        let payload = build_trace_transport_payload(&json!({}), "sess-1".to_string(), None);
        assert_eq!(payload["transport_health"], Value::Null);
    }

    #[test]
    fn transport_payload_health_from_explicit_health_block() {
        let input = json!({
            "transport_health": {"custom_field": "yes"}
        });
        let payload = build_trace_transport_payload(&input, "sess-1".to_string(), None);
        assert_eq!(payload["transport_health"]["custom_field"], "yes");
    }

    // ── build_trace_transport_descriptor ──

    #[test]
    fn transport_descriptor_top_level_schema() {
        let input = json!({"session_id": "sess-1"});
        let desc = build_trace_transport_descriptor(input).expect("descriptor");
        assert!(desc.get("schema_version").and_then(Value::as_str).is_some());
        assert!(desc.get("authority").and_then(Value::as_str).is_some());
        assert!(desc.get("transport").is_some());
    }

    // ── build_trace_handoff_descriptor ──

    #[test]
    fn handoff_requires_session_id() {
        let err = build_trace_handoff_descriptor(json!({})).expect_err("missing session_id");
        assert!(err.to_string().contains("session_id"));
    }

    #[test]
    fn handoff_descriptor_top_level_schema() {
        let input = json!({"session_id": "sess-1"});
        let desc = build_trace_handoff_descriptor(input).expect("handoff");
        assert!(desc.get("schema_version").and_then(Value::as_str).is_some());
        assert!(desc.get("authority").and_then(Value::as_str).is_some());
        let handoff = &desc["handoff"];
        assert_eq!(handoff["session_id"], "sess-1");
        assert_eq!(handoff["checkpoint_backend_family"], "filesystem");
        assert!(handoff["recovery_artifacts"].as_array().unwrap().is_empty());
    }

    #[test]
    fn handoff_inherits_transport_from_nested_transport_block() {
        let input = json!({
            "session_id": "sess-1",
            "transport": {
                "session_id": "transport-sess",
                "job_id": "transport-job",
                "binding_backend_family": "redis",
            }
        });
        let desc = build_trace_handoff_descriptor(input).expect("handoff");
        let handoff = &desc["handoff"];
        assert_eq!(handoff["checkpoint_backend_family"], "redis");
        assert_eq!(
            handoff["stream_id"],
            "stream::transport-job"
        );
    }

    #[test]
    fn handoff_recovery_artifacts_from_explicit_list() {
        let input = json!({
            "session_id": "sess-1",
            "recovery_artifacts": ["a.json", "b.json"]
        });
        let desc = build_trace_handoff_descriptor(input).expect("handoff");
        let artifacts = &desc["handoff"]["recovery_artifacts"];
        assert_eq!(artifacts.as_array().unwrap().len(), 2);
    }

    #[test]
    fn handoff_trace_stream_and_resume_paths() {
        let input = json!({
            "session_id": "sess-1",
            "trace_stream_path": "/tmp/stream.jsonl",
            "resume_manifest_path": "/tmp/manifest.json",
        });
        let desc = build_trace_handoff_descriptor(input).expect("handoff");
        let handoff = &desc["handoff"];
        assert_eq!(handoff["trace_stream_path"], "/tmp/stream.jsonl");
        assert_eq!(handoff["resume_manifest_path"], "/tmp/manifest.json");
    }

    #[test]
    fn handoff_recovery_artifacts_from_transport_binding_path() {
        let input = json!({
            "session_id": "sess-1",
            "transport": {
                "binding_artifact_path": "/tmp/binding.json",
            }
        });
        let desc = build_trace_handoff_descriptor(input).expect("handoff");
        let artifacts = &desc["handoff"]["recovery_artifacts"];
        let paths: Vec<&str> = artifacts
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(paths.contains(&"/tmp/binding.json"));
    }

    // ── build_checkpoint_resume_manifest ──

    #[test]
    fn resume_manifest_requires_session_id() {
        let err =
            build_checkpoint_resume_manifest(json!({})).expect_err("missing session_id");
        assert!(err.to_string().contains("session_id"));
    }

    #[test]
    fn resume_manifest_defaults_running_generation_zero() {
        let input = json!({"session_id": "sess-1"});
        let manifest = build_checkpoint_resume_manifest(input).expect("manifest");
        assert_eq!(
            manifest["resume_manifest"]["status"],
            "running"
        );
        assert_eq!(manifest["resume_manifest"]["generation"], 0);
    }

    #[test]
    fn resume_manifest_with_artifact_paths() {
        let input = json!({
            "session_id": "sess-1",
            "status": "paused",
            "generation": 3,
            "artifact_paths": ["/tmp/a.txt", "/tmp/b.txt"],
            "updated_at": "2026-06-26T12:00:00Z",
        });
        let manifest = build_checkpoint_resume_manifest(input).expect("manifest");
        let rm = &manifest["resume_manifest"];
        assert_eq!(rm["status"], "paused");
        assert_eq!(rm["generation"], 3);
        assert_eq!(rm["artifact_paths"].as_array().unwrap().len(), 2);
        assert_eq!(rm["updated_at"], "2026-06-26T12:00:00Z");
    }

    #[test]
    fn resume_manifest_top_level_schema() {
        let input = json!({"session_id": "sess-1"});
        let manifest = build_checkpoint_resume_manifest(input).expect("manifest");
        assert!(manifest.get("schema_version").and_then(Value::as_str).is_some());
        assert!(manifest.get("authority").and_then(Value::as_str).is_some());
        assert!(manifest.get("resume_manifest").is_some());
    }

    #[test]
    fn resume_manifest_control_plane_pass_through() {
        let input = json!({
            "session_id": "sess-1",
            "control_plane": {"key": "val"}
        });
        let manifest = build_checkpoint_resume_manifest(input).expect("manifest");
        assert_eq!(manifest["resume_manifest"]["control_plane"]["key"], "val");
    }

    // ── write_text_payload ──

    #[test]
    fn write_text_payload_writes_atomically() {
        let dir = std::env::temp_dir().join(format!(
            "trace-transport-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.txt");
        let written = write_text_payload(&path, "hello world").expect("write");
        assert_eq!(written, 11);
        let content = fs::read_to_string(&path).expect("read");
        assert_eq!(content, "hello world");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_text_payload_rejects_unsafe_path() {
        let err = write_text_payload(Path::new("/nonexistent/test.txt"), "data")
            .expect_err("should reject");
        assert!(!err.to_string().is_empty(), "error message must not be empty");
    }

    // ── write_transport_binding_payload ──

    #[test]
    fn write_transport_binding_requires_path() {
        let err =
            write_transport_binding_payload(json!({"session_id": "sess-1"}))
                .expect_err("missing path");
        assert!(err.to_string().contains("path"));
    }

    #[test]
    fn write_transport_binding_requires_session_id() {
        let err =
            write_transport_binding_payload(json!({"path": "/tmp/bind.json"}))
                .expect_err("missing session_id");
        assert!(err.to_string().contains("session_id"));
    }

    // ── write_checkpoint_resume_manifest_payload ──

    #[test]
    fn write_resume_manifest_requires_path() {
        let err = write_checkpoint_resume_manifest_payload(json!({"session_id": "sess-1"}))
            .expect_err("missing path");
        assert!(err.to_string().contains("path"));
    }
}
