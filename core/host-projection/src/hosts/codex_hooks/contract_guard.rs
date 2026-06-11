//! Contract guard: compares a caller-provided expected contract digest against
//! the live Rust `framework contract-summary` payload, failing closed on drift
//! unless the caller sets an explicit contract update intent.

use super::CODEX_HOOK_AUTHORITY;
use crate::hooks::build_framework_contract_summary_envelope;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;

pub(super) fn run_codex_contract_guard(
    repo_root: &Path,
    payload: &Value,
) -> Result<Option<Value>, String> {
    let envelope = build_framework_contract_summary_envelope(repo_root)?;
    let summary = envelope
        .get("contract_summary")
        .ok_or_else(|| "framework contract summary missing contract_summary".to_string())?;
    let drift_flags = detect_contract_drift(summary, payload);
    let explicit_update = payload_bool(payload, "contract_update_intent")
        || payload_bool(payload, "allow_contract_update")
        || payload_bool(payload, "explicit_contract_update");
    let live_digest = summary
        .get("contract_digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let decision = if !drift_flags.is_empty() && !explicit_update {
        "block"
    } else {
        "approve"
    };
    let reason = if drift_flags.is_empty() {
        "contract guard passed; no drift detected".to_string()
    } else if explicit_update {
        format!(
            "contract guard observed drift but explicit update intent was provided: {}",
            drift_flags.join(", ")
        )
    } else {
        format!(
            "contract guard blocked drift without explicit contract update intent: {}",
            drift_flags.join(", ")
        )
    };
    let mut response = json!({
        "decision": decision,
        "authority": CODEX_HOOK_AUTHORITY,
        "contract_guard": {
            "schema_version": "router-rs-codex-contract-guard-v1",
            "live_contract_digest": live_digest,
            "drift_flags": drift_flags,
            "explicit_contract_update": explicit_update,
            "prompt_lines": summary.get("prompt_lines").cloned().unwrap_or(Value::Array(Vec::new())),
            "reason": reason,
        },
    });
    if decision == "block" {
        response["hookSpecificOutput"] = json!({
            "hookEventName": "ContractGuard",
            "permissionDecision": "deny",
            "permissionDecisionReason": response["contract_guard"]["reason"].clone(),
        });
    }
    Ok(Some(response))
}

fn detect_contract_drift(summary: &Value, payload: &Value) -> Vec<String> {
    let mut flags = Vec::new();
    let live_digest = summary
        .get("contract_digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(expected) = payload_string(payload, "expected_contract_digest")
        .or_else(|| payload_string(payload, "contract_digest"))
    {
        let expected = expected.strip_prefix("sha256:").unwrap_or(&expected);
        if !expected.is_empty() && expected != live_digest {
            flags.push("contract_digest_drift".to_string());
        }
    }

    let live_owner = summary
        .get("primary_owner")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(proposed_owner) = payload_string(payload, "proposed_primary_owner")
        .or_else(|| payload_string(payload, "primary_owner"))
    {
        if !live_owner.is_empty() && proposed_owner != live_owner {
            flags.push("owner_drift".to_string());
        }
    }

    let contract_active = summary
        .get("contract_guard")
        .and_then(|guard| guard.get("contract_active"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if contract_active {
        let live_task = summary
            .get("continuity")
            .and_then(|continuity| continuity.get("task"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(proposed_task) =
            payload_string(payload, "proposed_task").or_else(|| payload_string(payload, "task"))
        {
            if !live_task.is_empty() && proposed_task != live_task {
                flags.push("scope_drift".to_string());
            }
        }

        let live_goal = scalar_contract_text(summary.get("goal"));
        if let Some(proposed_goal) =
            payload_string(payload, "proposed_goal").or_else(|| payload_string(payload, "goal"))
        {
            if !live_goal.is_empty() && proposed_goal != live_goal {
                flags.push("scope_drift".to_string());
            }
        }

        let live_evidence = string_array(summary.get("evidence_required"));
        let proposed_evidence_exists = payload.get("proposed_evidence_required").is_some();
        let proposed_evidence = string_array(payload.get("proposed_evidence_required"));
        let drops_evidence = payload_bool(payload, "drops_evidence_required");
        let evidence_changed = proposed_evidence_exists
            && normalized_string_set(&proposed_evidence) != normalized_string_set(&live_evidence);
        if (drops_evidence && !live_evidence.is_empty()) || evidence_changed {
            flags.push("evidence_drift".to_string());
        }
    }

    flags.sort();
    flags.dedup();
    flags
}

fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn payload_bool(payload: &Value, key: &str) -> bool {
    payload.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn scalar_contract_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => String::new(),
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn normalized_string_set(values: &[String]) -> Vec<String> {
    let mut deduped = HashSet::new();
    let mut normalized = values
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .filter_map(|item| {
            let lower = item.to_ascii_lowercase();
            deduped.insert(lower.clone()).then_some(lower)
        })
        .collect::<Vec<_>>();
    normalized.sort();
    normalized
}
