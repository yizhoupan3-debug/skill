//! Runtime event attach / subscribe / cleanup.

use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::runtime_envelope_ids::ATTACHED_RUNTIME_EVENT_ATTACH_AUTHORITY;
use crate::runtime_storage::{
    ResolvedStorageBackend, resolve_storage_backend, storage_artifact_exists, storage_read_text,
};
use crate::stdio_payload_types::TraceStreamReplayRequestPayload;

use super::json_value::{nested_non_empty_string, optional_bool, optional_non_empty_string};
use super::trace_stream_io::replay_trace_stream;

fn descriptor_mapping<'a>(
    attach_descriptor: &'a Value,
    field_name: &str,
) -> Result<Option<&'a Map<String, Value>>, String> {
    match attach_descriptor.get(field_name) {
        None => Ok(None),
        Some(Value::Object(map)) => Ok(Some(map)),
        Some(_) => Err(format!(
            "External runtime event attach descriptor field {field_name:?} must be a mapping."
        )),
    }
}

fn mapping_string(
    mapping: &Map<String, Value>,
    field_name: &str,
) -> Result<Option<String>, String> {
    match mapping.get(field_name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!(
            "External runtime event attach descriptor field {field_name:?} must be a string."
        )),
    }
}

fn merge_attach_path_values(
    explicit_value: Option<&str>,
    descriptor_value: Option<String>,
    field_name: &str,
) -> Result<Option<String>, String> {
    match (explicit_value, descriptor_value) {
        (None, descriptor) => Ok(descriptor),
        (Some(explicit), None) => Ok(Some(explicit.to_string())),
        (Some(explicit), Some(descriptor)) if explicit == descriptor => {
            Ok(Some(explicit.to_string()))
        }
        (Some(_), Some(_)) => Err(format!(
            "External runtime event attach received conflicting {field_name:?} values between direct args and attach_descriptor."
        )),
    }
}

struct NormalizedAttachRequest {
    binding_artifact_path: Option<String>,
    handoff_path: Option<String>,
    resume_manifest_path: Option<String>,
    trace_stream_path: Option<String>,
    binding_artifact_resolution: Option<String>,
    handoff_resolution: Option<String>,
    resume_manifest_resolution: Option<String>,
}

fn normalize_attach_request(payload: &Value) -> Result<NormalizedAttachRequest, String> {
    let explicit_binding_artifact_path =
        optional_non_empty_string(payload, "binding_artifact_path");
    let explicit_handoff_path = optional_non_empty_string(payload, "handoff_path");
    let explicit_resume_manifest_path = optional_non_empty_string(payload, "resume_manifest_path");
    let explicit_binding_artifact_path_ref = explicit_binding_artifact_path.as_ref().map(|_| ());
    let explicit_handoff_path_ref = explicit_handoff_path.as_ref().map(|_| ());
    let explicit_resume_manifest_path_ref = explicit_resume_manifest_path.as_ref().map(|_| ());
    let attach_descriptor = match payload.get("attach_descriptor") {
        None | Some(Value::Null) => {
            return Ok(NormalizedAttachRequest {
                binding_artifact_path: explicit_binding_artifact_path,
                handoff_path: explicit_handoff_path,
                resume_manifest_path: explicit_resume_manifest_path,
                trace_stream_path: None,
                binding_artifact_resolution: explicit_binding_artifact_path_ref
                    .map(|_| "explicit_request".to_string()),
                handoff_resolution: explicit_handoff_path_ref
                    .map(|_| "explicit_request".to_string()),
                resume_manifest_resolution: explicit_resume_manifest_path_ref
                    .map(|_| "explicit_request".to_string()),
            });
        }
        Some(descriptor) => descriptor,
    };
    if !attach_descriptor.is_object() {
        return Err("External runtime event attach descriptor must be a mapping.".to_string());
    }
    let schema_version = attach_descriptor
        .get("schema_version")
        .and_then(Value::as_str);
    if let Some(schema_version) = schema_version {
        if schema_version != "runtime-event-attach-descriptor-v1" {
            return Err(format!(
                "Unsupported runtime event attach descriptor schema: {schema_version:?}"
            ));
        }
    }
    let attach_mode = attach_descriptor.get("attach_mode").and_then(Value::as_str);
    if let Some(attach_mode) = attach_mode {
        if attach_mode != "process_external_artifact_replay" {
            return Err(format!(
                "Unsupported runtime event attach mode: {attach_mode:?}"
            ));
        }
    }
    let expected_scalars = [
        (
            "source_transport_method",
            "describe_runtime_event_transport",
        ),
        ("source_handoff_method", "describe_runtime_event_handoff"),
        ("attach_method", "attach_runtime_event_transport"),
        ("subscribe_method", "subscribe_attached_runtime_events"),
        ("cleanup_method", "cleanup_attached_runtime_event_transport"),
        ("resume_mode", "after_event_id"),
    ];
    for (field_name, expected) in expected_scalars {
        if let Some(value) = attach_descriptor.get(field_name).and_then(Value::as_str) {
            if value != expected {
                return Err(format!(
                    "External runtime event attach descriptor must use {field_name}={expected:?}."
                ));
            }
        }
    }
    if let Some(capabilities) = descriptor_mapping(attach_descriptor, "attach_capabilities")? {
        if capabilities.get("artifact_replay").and_then(Value::as_bool) != Some(true) {
            return Err(
                "External runtime event attach descriptor must advertise attach_capabilities.artifact_replay=True."
                    .to_string(),
            );
        }
        if !matches!(
            capabilities
                .get("live_remote_stream")
                .and_then(Value::as_bool),
            None | Some(false)
        ) {
            return Err(
                "External runtime event attach descriptor must advertise attach_capabilities.live_remote_stream=False."
                    .to_string(),
            );
        }
        if !matches!(
            capabilities
                .get("cleanup_preserves_replay")
                .and_then(Value::as_bool),
            None | Some(true)
        ) {
            return Err(
                "External runtime event attach descriptor must advertise attach_capabilities.cleanup_preserves_replay=True."
                    .to_string(),
            );
        }
    }
    let _ = descriptor_mapping(attach_descriptor, "requested_artifacts")?;
    let resolution_mapping = descriptor_mapping(attach_descriptor, "resolution")?;
    let resolved_mapping = descriptor_mapping(attach_descriptor, "resolved_artifacts")?
        .unwrap_or_else(|| {
            attach_descriptor
                .as_object()
                .expect("attach descriptor object")
        });
    let descriptor_binding = mapping_string(resolved_mapping, "binding_artifact_path")?;
    let descriptor_handoff = mapping_string(resolved_mapping, "handoff_path")?;
    let descriptor_resume = mapping_string(resolved_mapping, "resume_manifest_path")?;
    let descriptor_trace_stream = mapping_string(resolved_mapping, "trace_stream_path")?;
    let binding_artifact_path = merge_attach_path_values(
        explicit_binding_artifact_path.as_deref(),
        descriptor_binding,
        "binding_artifact_path",
    )?;
    let handoff_path = merge_attach_path_values(
        explicit_handoff_path.as_deref(),
        descriptor_handoff,
        "handoff_path",
    )?;
    let resume_manifest_path = merge_attach_path_values(
        explicit_resume_manifest_path.as_deref(),
        descriptor_resume,
        "resume_manifest_path",
    )?;
    Ok(NormalizedAttachRequest {
        binding_artifact_path,
        handoff_path,
        resume_manifest_path,
        trace_stream_path: descriptor_trace_stream,
        binding_artifact_resolution: if explicit_binding_artifact_path_ref.is_some() {
            Some("explicit_request".to_string())
        } else {
            resolution_mapping
                .as_ref()
                .map(|mapping| mapping_string(mapping, "binding_artifact_path"))
                .transpose()?
                .flatten()
        },
        handoff_resolution: if explicit_handoff_path_ref.is_some() {
            Some("explicit_request".to_string())
        } else {
            resolution_mapping
                .as_ref()
                .map(|mapping| mapping_string(mapping, "handoff_path"))
                .transpose()?
                .flatten()
        },
        resume_manifest_resolution: if explicit_resume_manifest_path_ref.is_some() {
            Some("explicit_request".to_string())
        } else {
            resolution_mapping
                .as_ref()
                .map(|mapping| mapping_string(mapping, "resume_manifest_path"))
                .transpose()?
                .flatten()
        },
    })
}

fn require_requested_artifact(
    path: &Option<PathBuf>,
    storage_backend: Option<&ResolvedStorageBackend>,
    field_name: &str,
) -> Result<(), String> {
    if let Some(path) = path {
        if !storage_artifact_exists(path, storage_backend) {
            return Err(format!(
                "External runtime event attach requested {field_name:?} that does not exist: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn load_json_artifact(
    path: &Option<PathBuf>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<Option<Value>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !storage_artifact_exists(path, storage_backend) {
        return Ok(None);
    }
    serde_json::from_str::<Value>(&storage_read_text(path, storage_backend)?)
        .map(Some)
        .map_err(|err| {
            format!(
                "parse runtime attach artifact failed for {}: {err}",
                path.display()
            )
        })
}

fn json_path(value: &Value, key: &str) -> Result<Option<PathBuf>, String> {
    normalize_optional_runtime_path(optional_non_empty_string(value, key))
}

fn nested_json_path(value: &Value, path: &[&str]) -> Result<Option<PathBuf>, String> {
    normalize_optional_runtime_path(nested_non_empty_string(value, path))
}

fn normalize_optional_runtime_path(value: Option<String>) -> Result<Option<PathBuf>, String> {
    value
        .map(|path| {
            let candidate = PathBuf::from(path.trim());
            if candidate.as_os_str().is_empty() {
                return Err("runtime attach path must be non-empty".to_string());
            }
            if candidate.is_absolute() {
                Ok(candidate)
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(candidate))
                    .map_err(|err| format!("resolve runtime attach path failed: {err}"))
            }
        })
        .transpose()
}

fn cached_current_dir() -> &'static PathBuf {
    static CACHED_CWD: OnceLock<PathBuf> = OnceLock::new();
    CACHED_CWD.get_or_init(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    cached_current_dir().join(path)
}

fn infer_resume_manifest_path(binding_artifact_path: &Path) -> PathBuf {
    let candidates = [
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_RESUME_MANIFEST.json")),
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_RESUME_MANIFEST.json")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| {
            binding_artifact_path
                .parent()
                .and_then(Path::parent)
                .map(|parent| parent.join("TRACE_RESUME_MANIFEST.json"))
                .unwrap_or_else(|| PathBuf::from("TRACE_RESUME_MANIFEST.json"))
        })
}

fn infer_trace_stream_from_binding_artifact(
    binding_artifact_path: Option<&Path>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Option<PathBuf> {
    let binding_artifact_path = binding_artifact_path?;
    let candidates = [
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_EVENTS.jsonl")),
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_EVENTS.jsonl")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|candidate| storage_artifact_exists(candidate, storage_backend))
}

fn validate_attached_runtime_alignment(
    transport: &Value,
    handoff: Option<&Value>,
    resume_manifest: Option<&Value>,
    binding_artifact_path: Option<&Path>,
    resume_manifest_path: Option<&Path>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<(), String> {
    let transport_stream_id = optional_non_empty_string(transport, "stream_id");
    let transport_session_id = optional_non_empty_string(transport, "session_id");
    let transport_job_id = optional_non_empty_string(transport, "job_id");

    if let Some(handoff) = handoff {
        if optional_non_empty_string(handoff, "stream_id") != transport_stream_id {
            return Err(
                "External runtime event attach rejected mismatched transport/handoff stream ids."
                    .to_string(),
            );
        }
        if optional_non_empty_string(handoff, "session_id") != transport_session_id
            || optional_non_empty_string(handoff, "job_id") != transport_job_id
        {
            return Err(
                "External runtime event attach rejected mismatched transport/handoff stream scope."
                    .to_string(),
            );
        }
        if let (Some(binding_artifact_path), Some(handoff_binding_path)) = (
            binding_artifact_path,
            nested_json_path(handoff, &["transport", "binding_artifact_path"])?,
        ) {
            if normalize_path_for_compare(&handoff_binding_path)
                != normalize_path_for_compare(binding_artifact_path)
            {
                return Err("External runtime event attach rejected mismatched transport/handoff binding artifact paths.".to_string());
            }
        }
        if let (Some(resume_manifest_path), Some(handoff_resume_manifest_path)) = (
            resume_manifest_path,
            json_path(handoff, "resume_manifest_path")?,
        ) {
            if normalize_path_for_compare(&handoff_resume_manifest_path)
                != normalize_path_for_compare(resume_manifest_path)
            {
                return Err("External runtime event attach rejected mismatched handoff/resume manifest paths.".to_string());
            }
        }
    }

    if let Some(resume_manifest) = resume_manifest {
        if optional_non_empty_string(resume_manifest, "session_id") != transport_session_id
            || optional_non_empty_string(resume_manifest, "job_id") != transport_job_id
        {
            return Err(
                "External runtime event attach rejected mismatched transport/resume stream scope."
                    .to_string(),
            );
        }
        if let (Some(binding_artifact_path), Some(resume_binding_path)) = (
            binding_artifact_path,
            json_path(resume_manifest, "event_transport_path")?,
        ) {
            if normalize_path_for_compare(&resume_binding_path)
                != normalize_path_for_compare(binding_artifact_path)
            {
                return Err("External runtime event attach rejected mismatched transport/resume binding artifact paths.".to_string());
            }
        }
        if let (Some(_handoff), Some(handoff_trace_stream_path), Some(resume_trace_stream_path)) = (
            handoff,
            handoff
                .map(|value| json_path(value, "trace_stream_path"))
                .transpose()?
                .flatten(),
            json_path(resume_manifest, "trace_stream_path")?,
        ) {
            if normalize_path_for_compare(&handoff_trace_stream_path)
                != normalize_path_for_compare(&resume_trace_stream_path)
            {
                return Err("External runtime event attach rejected mismatched handoff/resume trace stream paths.".to_string());
            }
        }
    }

    let binding_trace_stream_path =
        infer_trace_stream_from_binding_artifact(binding_artifact_path, storage_backend);
    if let (Some(binding_trace_stream_path), Some(_handoff), Some(handoff_trace_stream_path)) = (
        binding_trace_stream_path.as_ref(),
        handoff,
        handoff
            .map(|value| json_path(value, "trace_stream_path"))
            .transpose()?
            .flatten(),
    ) {
        if normalize_path_for_compare(&handoff_trace_stream_path)
            != normalize_path_for_compare(binding_trace_stream_path)
        {
            return Err("External runtime event attach rejected mismatched binding/handoff trace stream paths.".to_string());
        }
    }
    if let (
        Some(binding_trace_stream_path),
        Some(_resume_manifest),
        Some(resume_trace_stream_path),
    ) = (
        binding_trace_stream_path.as_ref(),
        resume_manifest,
        resume_manifest
            .map(|value| json_path(value, "trace_stream_path"))
            .transpose()?
            .flatten(),
    ) {
        if normalize_path_for_compare(&resume_trace_stream_path)
            != normalize_path_for_compare(binding_trace_stream_path)
        {
            return Err("External runtime event attach rejected mismatched binding/resume trace stream paths.".to_string());
        }
    }
    Ok(())
}

fn trace_stream_resolution(
    handoff: Option<&Value>,
    resume_manifest: Option<&Value>,
    binding_artifact_path: Option<&Path>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<Option<(PathBuf, String)>, String> {
    if let Some(handoff) = handoff {
        if let Some(path) = json_path(handoff, "trace_stream_path")? {
            return Ok(Some((path, "handoff_manifest".to_string())));
        }
    }
    if let Some(resume_manifest) = resume_manifest {
        if let Some(path) = json_path(resume_manifest, "trace_stream_path")? {
            return Ok(Some((path, "resume_manifest".to_string())));
        }
    }
    if let Some(path) =
        infer_trace_stream_from_binding_artifact(binding_artifact_path, storage_backend)
    {
        return Ok(Some((path, "binding_artifact_adjacency".to_string())));
    }
    Ok(None)
}

pub fn attach_runtime_event_transport(payload: Value) -> Result<Value, String> {
    let normalized_request = normalize_attach_request(&payload)?;
    let binding_artifact_path = normalized_request.binding_artifact_path;
    let handoff_path = normalized_request.handoff_path;
    let resume_manifest_path = normalized_request.resume_manifest_path;
    let descriptor_trace_stream_path =
        normalize_optional_runtime_path(normalized_request.trace_stream_path)?;
    if binding_artifact_path.is_none() && handoff_path.is_none() && resume_manifest_path.is_none() {
        return Err(
            "External runtime event attach requires a binding artifact, handoff manifest, or resume manifest path."
                .to_string(),
        );
    }

    let binding_path = normalize_optional_runtime_path(binding_artifact_path)?;
    let handoff_file = normalize_optional_runtime_path(handoff_path)?;
    let resume_file = normalize_optional_runtime_path(resume_manifest_path)?;
    let mut binding_source = normalized_request.binding_artifact_resolution;
    let handoff_source = normalized_request.handoff_resolution;
    let mut resume_source = normalized_request.resume_manifest_resolution;

    let requested_paths = [
        binding_path.as_deref(),
        handoff_file.as_deref(),
        resume_file.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(Path::to_path_buf)
    .collect::<Vec<_>>();
    let storage_backend = resolve_storage_backend(&requested_paths);
    require_requested_artifact(
        &binding_path,
        storage_backend.as_ref(),
        "binding_artifact_path",
    )?;
    require_requested_artifact(&handoff_file, storage_backend.as_ref(), "handoff_path")?;
    require_requested_artifact(
        &resume_file,
        storage_backend.as_ref(),
        "resume_manifest_path",
    )?;

    let handoff = load_json_artifact(&handoff_file, storage_backend.as_ref())?;
    let mut resume_manifest = load_json_artifact(&resume_file, storage_backend.as_ref())?;
    let mut resolved_resume_file = resume_file.clone();

    if resume_manifest.is_none() {
        if let Some(handoff_resume_path) = handoff
            .as_ref()
            .map(|payload| json_path(payload, "resume_manifest_path"))
            .transpose()?
            .flatten()
        {
            if storage_artifact_exists(&handoff_resume_path, storage_backend.as_ref()) {
                resolved_resume_file = Some(handoff_resume_path.clone());
                resume_manifest =
                    load_json_artifact(&Some(handoff_resume_path), storage_backend.as_ref())?;
                resume_source = Some("handoff_manifest".to_string());
            }
        }
    }

    let mut transport_path = binding_path.clone();
    if transport_path.is_none() {
        if let Some(resume_transport_path) = resume_manifest
            .as_ref()
            .map(|payload| json_path(payload, "event_transport_path"))
            .transpose()?
            .flatten()
        {
            if storage_artifact_exists(&resume_transport_path, storage_backend.as_ref()) {
                transport_path = Some(resume_transport_path);
                binding_source = Some("resume_manifest".to_string());
            }
        }
    }
    if transport_path.is_none() {
        if let Some(handoff_transport_path) = handoff
            .as_ref()
            .map(|payload| nested_json_path(payload, &["transport", "binding_artifact_path"]))
            .transpose()?
            .flatten()
        {
            if storage_artifact_exists(&handoff_transport_path, storage_backend.as_ref()) {
                transport_path = Some(handoff_transport_path);
                binding_source = Some("handoff_transport".to_string());
            }
        }
    }

    if transport_path.is_none() && handoff.is_none() {
        return Err(
            "External runtime event attach could not resolve a transport binding artifact from the provided manifests."
                .to_string(),
        );
    }

    let transport = if let Some(transport_path) = transport_path.as_ref() {
        load_json_artifact(
            &Some(transport_path.to_path_buf()),
            storage_backend.as_ref(),
        )?
        .ok_or_else(|| {
            "External runtime event attach could not load a transport descriptor.".to_string()
        })?
    } else {
        handoff
            .as_ref()
            .and_then(|payload| payload.get("transport").cloned())
            .ok_or_else(|| {
                "External runtime event attach could not load a transport descriptor.".to_string()
            })?
    };

    if resume_manifest.is_none() {
        if let Some(transport_path) = transport_path.as_ref() {
            let inferred_resume_path = infer_resume_manifest_path(transport_path);
            if storage_artifact_exists(&inferred_resume_path, storage_backend.as_ref()) {
                resolved_resume_file = Some(inferred_resume_path.clone());
                resume_manifest =
                    load_json_artifact(&Some(inferred_resume_path), storage_backend.as_ref())?;
            }
        }
    }

    validate_attached_runtime_alignment(
        &transport,
        handoff.as_ref(),
        resume_manifest.as_ref(),
        transport_path.as_deref(),
        resolved_resume_file.as_deref(),
        storage_backend.as_ref(),
    )?;

    let Some((trace_stream_path, trace_stream_source)) = trace_stream_resolution(
        handoff.as_ref(),
        resume_manifest.as_ref(),
        transport_path.as_deref(),
        storage_backend.as_ref(),
    )?
    else {
        return Err(
            "External runtime event replay requires a handoff or resume manifest with trace_stream_path, or a filesystem binding artifact adjacent to TRACE_EVENTS.jsonl."
                .to_string(),
        );
    };
    if let Some(descriptor_trace_stream_path) = descriptor_trace_stream_path.as_ref() {
        if normalize_path_for_compare(descriptor_trace_stream_path)
            != normalize_path_for_compare(&trace_stream_path)
        {
            return Err(
                "External runtime event attach descriptor must already match canonical 'resolved_artifacts.trace_stream_path'."
                    .to_string(),
            );
        }
    }
    if !storage_artifact_exists(&trace_stream_path, storage_backend.as_ref()) {
        return Err(format!(
            "External runtime event replay trace stream not found: {}",
            trace_stream_path.display()
        ));
    }

    let resume_mode = optional_non_empty_string(&transport, "resume_mode")
        .unwrap_or_else(|| "after_event_id".to_string());
    let artifact_backend_family = optional_non_empty_string(&transport, "binding_backend_family")
        .unwrap_or_else(|| "filesystem".to_string());
    let source_transport_method = "describe_runtime_event_transport";
    let source_handoff_method = "describe_runtime_event_handoff";
    let attach_method = "attach_runtime_event_transport";
    let subscribe_method = "subscribe_attached_runtime_events";
    let cleanup_method = "cleanup_attached_runtime_event_transport";
    let cleanup_semantics = "no_persisted_state";
    let recommended_entrypoint = "describe_runtime_event_handoff";
    let attach_descriptor = json!({
        "schema_version": "runtime-event-attach-descriptor-v1",
        "attach_mode": "process_external_artifact_replay",
        "artifact_backend_family": artifact_backend_family.clone(),
        "source_transport_method": source_transport_method,
        "source_handoff_method": source_handoff_method,
        "attach_method": attach_method,
        "subscribe_method": subscribe_method,
        "cleanup_method": cleanup_method,
        "resume_mode": resume_mode.clone(),
        "cleanup_semantics": cleanup_semantics,
        "attach_capabilities": {
            "artifact_replay": true,
            "live_remote_stream": false,
            "cleanup_preserves_replay": true,
        },
        "recommended_entrypoint": recommended_entrypoint,
        "requested_artifacts": {
            "binding_artifact_path": transport_path.as_ref().map(|path| path.display().to_string()),
            "handoff_path": handoff_file.as_ref().map(|path| path.display().to_string()),
            "resume_manifest_path": resolved_resume_file.as_ref().map(|path| path.display().to_string()),
        },
        "resolved_artifacts": {
            "binding_artifact_path": transport_path.as_ref().map(|path| path.display().to_string()),
            "handoff_path": handoff_file.as_ref().map(|path| path.display().to_string()),
            "resume_manifest_path": resolved_resume_file.as_ref().map(|path| path.display().to_string()),
            "trace_stream_path": trace_stream_path.display().to_string(),
        },
        "resolution": {
            "binding_artifact_path": binding_source,
            "handoff_path": handoff_source,
            "resume_manifest_path": resume_source,
            "trace_stream_path": trace_stream_source,
        },
    });

    Ok(json!({
        "attach_mode": "process_external_artifact_replay",
        "artifact_backend_family": artifact_backend_family,
        "source_handoff_method": source_handoff_method,
        "source_transport_method": source_transport_method,
        "attach_method": attach_method,
        "subscribe_method": subscribe_method,
        "cleanup_method": cleanup_method,
        "resume_mode": resume_mode,
        "transport": transport,
        "handoff": handoff,
        "resume_manifest": resume_manifest,
        "binding_artifact_path": transport_path.as_ref().map(|path| path.display().to_string()),
        "handoff_path": handoff_file.as_ref().map(|path| path.display().to_string()),
        "resume_manifest_path": resolved_resume_file.as_ref().map(|path| path.display().to_string()),
        "trace_stream_path": trace_stream_path.display().to_string(),
        "replay_supported": true,
        "cleanup_semantics": cleanup_semantics,
        "cleanup_preserves_replay": true,
        "authority": ATTACHED_RUNTIME_EVENT_ATTACH_AUTHORITY,
        "attach_descriptor": attach_descriptor,
    }))
}

pub fn subscribe_attached_runtime_events(payload: Value) -> Result<Value, String> {
    let attached = attach_runtime_event_transport(payload.clone())?;
    let transport = attached
        .get("transport")
        .ok_or_else(|| "attached runtime transport payload missing transport".to_string())?;
    let session_id = optional_non_empty_string(transport, "session_id")
        .ok_or_else(|| "attached runtime transport payload missing session_id".to_string())?;
    let job_id = optional_non_empty_string(transport, "job_id");
    let trace_stream_path = attached
        .get("trace_stream_path")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "attached runtime transport payload missing trace_stream_path".to_string()
        })?;
    // Enforce a hard limit cap to bound peak memory for large trace streams.
    const SUBSCRIBE_LIMIT_CAP: usize = 10_000;
    let requested_limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    let capped_limit = requested_limit
        .map(|value| value.min(SUBSCRIBE_LIMIT_CAP))
        .or(Some(SUBSCRIBE_LIMIT_CAP));
    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: Some(trace_stream_path.to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some(session_id.clone()),
        job_id: job_id.clone(),
        stream_scope_fields: None,
        after_event_id: optional_non_empty_string(&payload, "after_event_id"),
        limit: capped_limit,
    })?;
    let heartbeat = optional_bool(&payload, "heartbeat").unwrap_or(false);
    let has_more = replay.has_more;
    let replay_events_empty = replay.events.is_empty();
    let next_cursor = serde_json::to_value(&replay.next_cursor)
        .map_err(|err| format!("serialize attached runtime cursor failed: {err}"))?;
    // Serialize events directly instead of cloning the entire Vec.
    let events_value = serde_json::to_value(&replay.events)
        .map_err(|err| format!("serialize attached runtime events failed: {err}"))?;
    let after_event_id = optional_non_empty_string(&payload, "after_event_id");
    Ok(json!({
        "schema_version": "runtime-event-stream-v1",
        "session_id": session_id,
        "job_id": job_id,
        "events": events_value,
        "next_cursor": next_cursor,
        "has_more": has_more,
        "after_event_id": after_event_id,
        "heartbeat": if heartbeat && replay_events_empty {
            json!({
                "schema_version": "runtime-event-stream-heartbeat-v1",
                "kind": "runtime.stream.heartbeat",
                "status": "idle",
            })
        } else {
            Value::Null
        },
    }))
}

pub fn cleanup_attached_runtime_event_transport(payload: Value) -> Result<Value, String> {
    let attached = attach_runtime_event_transport(payload)?;
    Ok(json!({
        "authority": ATTACHED_RUNTIME_EVENT_ATTACH_AUTHORITY,
        "cleanup_semantics": "no_persisted_state",
        "cleanup_preserves_replay": true,
        "cleanup_method": "cleanup_attached_runtime_event_transport",
        "binding_artifact_path": attached.get("binding_artifact_path").cloned().unwrap_or(Value::Null),
        "trace_stream_path": attached.get("trace_stream_path").cloned().unwrap_or(Value::Null),
    }))
}
