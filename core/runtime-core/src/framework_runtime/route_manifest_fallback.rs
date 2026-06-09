//! SKILL_MANIFEST 冷表回退路由（Roadmap v5 P7：自 CLI common 下沉至 B3）。

use std::path::{Path, PathBuf};

use crate::route::{
    literal_framework_alias_decision, load_records_from_manifest, read_json, route_task,
    should_accept_manifest_fallback, should_retry_with_manifest, RouteDecision, SkillRecord,
};
use serde_json::Value;

fn repo_root_from_cargo_manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn repo_root_for_runtime_path(runtime_path: &Path) -> PathBuf {
    let parent = runtime_path.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name().and_then(|name| name.to_str()) == Some("skills") {
        parent
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(repo_root_from_cargo_manifest_dir)
    } else {
        parent.to_path_buf()
    }
}

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
                repo_root_from_cargo_manifest_dir()
                    .join("skills")
                    .join("SKILL_MANIFEST.json"),
            )
            .filter(|path| path.exists())
        });
    Ok(fallback)
}

pub fn resolve_runtime_declared_manifest_fallback(
    runtime_path: &Path,
) -> Result<Option<PathBuf>, String> {
    let runtime_payload = read_json(runtime_path)?;
    let declared = runtime_payload
        .get("scope")
        .and_then(Value::as_object)
        .and_then(|scope| scope.get("fallback_manifest"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
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
            .unwrap_or_else(|| repo_root_from_cargo_manifest_dir().join(declared))
    };
    Ok(Some(resolved))
}

#[allow(clippy::too_many_arguments)]
pub fn route_task_with_manifest_fallback(
    runtime_records: &[SkillRecord],
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
    host_id: Option<&str>,
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    let scoped_runtime = crate::route::filter_records_for_host(runtime_records, host_id)?;
    if let Some(decision) = literal_framework_alias_decision(&scoped_runtime, query, session_id) {
        crate::telemetry_emit::emit_route_decision(query, &decision, false);
        return Ok(decision);
    }
    let hot_decision = route_task(
        &scoped_runtime,
        query,
        session_id,
        allow_overlay,
        first_turn,
    )?;
    let should_retry = should_retry_with_manifest(&hot_decision);
    let Some(fallback_path) = manifest_fallback_path(runtime_path, manifest_path)? else {
        crate::telemetry_emit::emit_route_decision(query, &hot_decision, false);
        return Ok(hot_decision);
    };
    let full_records = match load_records_from_manifest(&fallback_path)
        .and_then(|records| crate::route::filter_records_for_host(records, host_id))
    {
        Ok(records) => records,
        Err(err) if !should_retry && manifest_path.is_none() => {
            let mut degraded = hot_decision;
            degraded
                .reasons
                .push(format!("Manifest fallback unavailable: {err}"));
            crate::telemetry_emit::emit_route_decision(query, &degraded, false);
            return Ok(degraded);
        }
        Err(err) => return Err(err),
    };
    if let Some(decision) = literal_framework_alias_decision(&full_records, query, session_id) {
        crate::telemetry_emit::emit_route_decision(query, &decision, false);
        return Ok(decision);
    }
    let full_decision = route_task(&full_records, query, session_id, allow_overlay, first_turn)?;
    if should_accept_manifest_fallback(
        &hot_decision,
        &full_decision,
        &scoped_runtime,
        should_retry,
        manifest_path.is_some(),
    ) {
        let reroute = full_decision.selected_skill != hot_decision.selected_skill;
        crate::telemetry_emit::emit_route_decision(query, &full_decision, reroute);
        return Ok(full_decision);
    }
    crate::telemetry_emit::emit_route_decision(query, &hot_decision, false);
    Ok(hot_decision)
}
