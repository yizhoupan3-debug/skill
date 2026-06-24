//! Simplified routing entrypoint (formerly route-with-SKILL_MANIFEST-fallback).
//!
//! `route_task_with_manifest_fallback` is a routing-engine-based entrypoint
//! that filters records for host, checks for a literal framework alias decision,
//! and runs `route_task` on the filtered records. All manifest fallback logic
//! has been removed — `SKILL_ROUTING_RUNTIME.json` is the single source of truth.

use std::path::{Path, PathBuf};

use routing_engine::route::{
    RouteDecision, SkillRecord, filter_records_for_host, literal_framework_alias_decision,
    read_json, route_task,
};
use runtime_infra::telemetry_emit;

pub fn manifest_fallback_path(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    if let Some(path) = manifest_path {
        if path.exists() {
            return Ok(Some(path.to_path_buf()));
        }
        return Err(format!("manifest path does not exist: {}", path.display()));
    }
    let runtime_declared_fallback = match runtime_path {
        Some(path) => resolve_runtime_declared_manifest_fallback(path)?,
        None => None,
    };
    let fallback = runtime_declared_fallback
        .filter(|path| path.exists())
        .or_else(|| {
            runtime_path
                .and_then(Path::parent)
                .map(|parent| parent.join("SKILL_MANIFEST.json"))
                .filter(|path| path.exists())
        })
        .or_else(|| {
            Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../skills")
                    .join("SKILL_MANIFEST.json"),
            )
            .filter(|path| path.exists())
        });
    Ok(fallback)
}

fn repo_root_for_runtime_path(runtime_path: &Path) -> PathBuf {
    let parent = runtime_path.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name().and_then(|name| name.to_str()) == Some("skills") {
        parent
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    } else {
        parent.to_path_buf()
    }
}

pub fn resolve_runtime_declared_manifest_fallback(
    runtime_path: &Path,
) -> Result<Option<PathBuf>, String> {
    let runtime_payload = read_json(runtime_path)?;
    let declared = runtime_payload
        .get("scope")
        .and_then(|s| s.as_object())
        .and_then(|scope| scope.get("fallback_manifest"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let Some(declared) = declared else {
        return Ok(None);
    };
    let resolved = if Path::new(declared).is_absolute() {
        PathBuf::from(declared)
    } else if declared == "skills" || declared.starts_with("skills/") {
        repo_root_for_runtime_path(runtime_path).join(declared)
    } else {
        runtime_path
            .parent()
            .map(|parent| parent.join(declared))
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../").join(declared))
    };
    Ok(Some(resolved))
}

pub fn route_task_with_manifest_fallback(
    runtime_records: &[SkillRecord],
    host_id: Option<&str>,
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    let scoped_runtime = filter_records_for_host(runtime_records, host_id)?;
    if let Some(decision) = literal_framework_alias_decision(&scoped_runtime, query, session_id) {
        telemetry_emit::emit_route_decision(query, &decision, false, 0, "");
        return Ok(decision);
    }
    let decision = route_task(&scoped_runtime, query, session_id, allow_overlay, first_turn)?;
    telemetry_emit::emit_route_decision(query, &decision, false, 0, "");
    Ok(decision)
}
