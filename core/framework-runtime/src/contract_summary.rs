//! Framework contract summary building.
//!
//! Functions for building the contract summary envelope (`build_framework_contract_summary_envelope`)
//! including the SHA-256 digest, host harness fragment, and prompt-line helpers.

use crate::constants::{
    FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION, FRAMEWORK_RUNTIME_AUTHORITY,
};
use crate::json_io::read_json_strict;
use crate::json_value::{
    nonempty_string, value_string_list, value_text,
};
use crate::runtime_view;
use hex;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;
use tracing::instrument;

use crate::util::{count_evidence_rows, parse_session_summary};

#[instrument(level = "debug", skip_all)]
pub fn build_framework_contract_summary_envelope(repo_root: &Path) -> Result<Value, String> {
    let snapshot = runtime_view::load_framework_runtime_view(repo_root, None, None);
    let continuity = runtime_view::classify_runtime_continuity(&snapshot);
    let contract = supervisor_contract(&snapshot.supervisor_state);
    let workspace = runtime_view::workspace_name_from_root(repo_root);
    let continuity_route = continuity
        .get("route")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let primary_owner = {
        let direct = value_text(snapshot.supervisor_state.get("primary_owner"));
        if direct.is_empty() {
            continuity_route.first().map(|item| value_text(Some(item)))
        } else {
            Some(direct)
        }
    };
    let blocker_list = snapshot
        .supervisor_state
        .get("blockers")
        .and_then(Value::as_object)
        .and_then(|blockers| blockers.get("open_blockers"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let is_active = continuity.get("state").and_then(Value::as_str) == Some("active")
        && continuity.get("can_resume").and_then(Value::as_bool) == Some(true);
    let goal = if is_active {
        contract.get("goal").cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let scope = if is_active {
        value_string_list(contract.get("scope"))
    } else {
        Vec::<String>::new()
    };
    let forbidden_scope = if is_active {
        value_string_list(contract.get("forbidden_scope"))
    } else {
        Vec::<String>::new()
    };
    let acceptance_criteria = if is_active {
        value_string_list(contract.get("acceptance_criteria"))
    } else {
        Vec::<String>::new()
    };
    let evidence_required = if is_active {
        value_string_list(contract.get("evidence_required"))
    } else {
        Vec::<String>::new()
    };
    let active_phase = if is_active {
        nonempty_string(snapshot.supervisor_state.get("active_phase"))
    } else {
        Option::<String>::None
    };
    let next_actions = if is_active {
        continuity
            .get("next_actions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::<Value>::new()
    };
    let open_blockers = if is_active {
        blocker_list
    } else {
        Vec::<String>::new()
    };
    let session_summary: Map<String, Value> = parse_session_summary(&snapshot.session_summary_text);
    let evidence_count = count_evidence_rows(&snapshot.evidence_index);
    let contract_digest_input = json!({
        "workspace": workspace.clone(),
        "continuity_state": continuity.get("state").cloned().unwrap_or(Value::Null),
        "task": continuity.get("task").cloned().unwrap_or(Value::Null),
        "goal": goal,
        "scope": scope,
        "forbidden_scope": forbidden_scope,
        "acceptance_criteria": acceptance_criteria,
        "evidence_required": evidence_required,
        "active_phase": active_phase,
        "primary_owner": primary_owner.clone(),
        "next_actions": next_actions,
        "open_blockers": open_blockers,
        "trace_skills": continuity_route.clone(),
        "evidence_count": evidence_count,
    });
    let contract_digest = stable_json_sha256(&contract_digest_input)?;
    let session_summary_value = Value::Object(session_summary.clone());
    let host_harness = build_host_harness_summary_fragment(repo_root)?;
    let prompt_lines = build_contract_guard_prompt_lines(
        &contract_digest,
        &continuity,
        &contract_digest_input,
        &session_summary_value,
        snapshot.current_root.as_path(),
    );
    Ok(json!({
        "schema_version": FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION,
        "authority": FRAMEWORK_RUNTIME_AUTHORITY,
        "contract_summary": {
            "ok": true,
            "workspace": workspace,
            "contract_digest": contract_digest,
            "contract_digest_algorithm": "sha256",
            "contract_guard": {
                "contract_active": is_active,
                "drift_classes": ["scope_drift", "owner_drift", "evidence_drift", "contract_digest_drift"],
                "fail_closed_when": [
                    "expected contract_digest differs from live contract_digest",
                    "proposed owner differs from primary_owner without explicit contract update intent",
                    "proposed goal/task changes while continuity is active",
                    "verification/evidence requirements are dropped before completion"
                ],
                "update_requires_explicit_user_intent": true
            },
            "prompt_lines": prompt_lines,
            "continuity": continuity,
            "goal": contract_digest_input.get("goal").cloned().unwrap_or(Value::Null),
            "scope": contract_digest_input.get("scope").cloned().unwrap_or(Value::Array(Vec::new())),
            "forbidden_scope": contract_digest_input.get("forbidden_scope").cloned().unwrap_or(Value::Array(Vec::new())),
            "acceptance_criteria": contract_digest_input.get("acceptance_criteria").cloned().unwrap_or(Value::Array(Vec::new())),
            "evidence_required": contract_digest_input.get("evidence_required").cloned().unwrap_or(Value::Array(Vec::new())),
            "active_phase": contract_digest_input.get("active_phase").cloned().unwrap_or(Value::Null),
            "primary_owner": primary_owner,
            "next_actions": contract_digest_input.get("next_actions").cloned().unwrap_or(Value::Array(Vec::new())),
            "open_blockers": contract_digest_input.get("open_blockers").cloned().unwrap_or(Value::Array(Vec::new())),
            "trace_skills": continuity_route,
            "session_summary": session_summary,
            "evidence_count": evidence_count,
            "artifacts_root": snapshot.current_root.display().to_string(),
            "host_harness": host_harness,
            "recent_completed_execution": continuity.get("recent_completed_execution").cloned().unwrap_or(Value::Null),
            "recovery_hints": continuity.get("recovery_hints").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
        }
    }))
}

fn stable_json_sha256(value: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|err| format!("serialize contract digest input failed: {err}"))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

struct CachedRegistry {
    content: Value,
    mtime: Option<SystemTime>,
}

static REGISTRY_CACHE: Mutex<Option<CachedRegistry>> = Mutex::new(None);

/// Machine-readable per-host harness surface from `RUNTIME_REGISTRY.json` (for contract-summary / audits).
fn build_host_harness_summary_fragment(repo_root: &Path) -> Result<Value, String> {
    let path = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
    if !path.is_file() {
        return Err(format!(
            "RUNTIME_REGISTRY missing at {} — cannot build host_harness fragment",
            path.display()
        ));
    }
    let mtime = fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());
    {
        let guard = REGISTRY_CACHE.lock().expect("registry cache");
        if let Some(ref cached) = *guard
            && cached.mtime == mtime {
                return Ok(cached.content.clone());
            }
    }
    let v = read_json_strict(&path)?;
    let projections = v
        .get("host_projections")
        .and_then(Value::as_object)
        .ok_or_else(|| "RUNTIME_REGISTRY missing host_projections".to_string())?;
    let mut hosts: Vec<String> = projections.keys().cloned().collect();
    hosts.sort();
    let mut out = Map::new();
    for host in hosts {
        let proj = projections
            .get(&host)
            .and_then(Value::as_object)
            .ok_or_else(|| format!("host_projections.{host} must be an object"))?;
        out.insert(
            host,
            json!({
                "harness_capabilities": proj.get("harness_capabilities").cloned().unwrap_or(Value::Null),
                "harness_capability_exceptions": proj.get("harness_capability_exceptions").cloned().unwrap_or(Value::Null),
            }),
        );
    }
    let result = Value::Object(out);
    {
        let mut guard = REGISTRY_CACHE.lock().expect("registry cache");
        *guard = Some(CachedRegistry { content: result.clone(), mtime });
    }
    Ok(result)
}

fn build_contract_guard_prompt_lines(
    contract_digest: &str,
    continuity: &Value,
    digest_input: &Value,
    session_summary: &Value,
    artifact_root: &Path,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("contract_digest: sha256:{contract_digest}"));
    lines.push(format!(
        "continuity: state={} can_resume={}",
        value_text(continuity.get("state")),
        continuity
            .get("can_resume")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    ));
    let task = value_text(continuity.get("task"));
    if !task.is_empty() {
        lines.push(format!("task: {task}"));
    } else if let Some(task) = nonempty_string(session_summary.get("task")) {
        lines.push(format!("task: {task}"));
    }
    if let Some(owner) = nonempty_string(digest_input.get("primary_owner")) {
        lines.push(format!("owner: {owner}"));
    }
    if let Some(phase) = nonempty_string(digest_input.get("active_phase")) {
        lines.push(format!("phase: {phase}"));
    }
    for (label, key) in [
        ("goal", "goal"),
        ("scope", "scope"),
        ("forbidden_scope", "forbidden_scope"),
        ("acceptance", "acceptance_criteria"),
        ("evidence", "evidence_required"),
        ("blockers", "open_blockers"),
    ] {
        let line = compact_contract_value_line(label, digest_input.get(key));
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines.push(format!("artifacts: {}", artifact_root.display()));
    lines.truncate(12);
    lines
}

fn compact_contract_value_line(label: &str, value: Option<&Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    match value {
        Value::Null => String::new(),
        Value::String(text) if text.trim().is_empty() => String::new(),
        Value::String(text) => format!("{label}: {}", compact_contract_text(text, 140)),
        Value::Array(items) if items.is_empty() => String::new(),
        Value::Array(items) => {
            let joined = items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ");
            if joined.is_empty() {
                String::new()
            } else {
                format!("{label}: {}", compact_contract_text(&joined, 180))
            }
        }
        _ => {
            let text = value_text(Some(value));
            if text.is_empty() {
                String::new()
            } else {
                format!("{label}: {}", compact_contract_text(&text, 140))
            }
        }
    }
}

fn compact_contract_text(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().fold(String::new(), |mut acc, w| {
        if !acc.is_empty() {
            acc.push(' ');
        }
        acc.push_str(w);
        acc
    });
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut compact = normalized
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    compact.push_str("...");
    compact
}

fn supervisor_contract(state: &Map<String, Value>) -> Map<String, Value> {
    state
        .get("execution_contract")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}
