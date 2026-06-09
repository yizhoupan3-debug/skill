use router_rs::framework_error::FrameworkError;
use router_rs::framework_runtime::build_framework_contract_summary_envelope;
use router_rs::hook_common::read_stdin_payload;
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use super::install::{attach_codex_hook_observation, protected_generated_paths};
use super::lifecycle::run_codex_lifecycle_context_hook;
use super::{CODEX_HOOK_AUTHORITY, HOST_ENTRYPOINT_SYNC_HINT};

#[allow(dead_code)] // Codex audit CLI path; HostHook dispatch is canonical
pub fn run_codex_audit_hook(command: &str, repo_root: &Path) -> router_rs::framework_error::FrameworkResult<Option<Value>> {
    let _registry_guard = router_rs::runtime_registry::HookRegistryRepoGuard::new(repo_root);
    let canonical = canonical_codex_audit_command(command)?;
    let mut payload = match read_stdin_payload() {
        Ok(payload) => payload,
        Err(err) if canonical == "lifecycle-context" => {
            return Ok(attach_codex_hook_observation(Some(
                codex_lifecycle_input_error(&format!(
                    "Codex lifecycle hook input JSON invalid: {}",
                    err.to_hook_exit()
                )),
            )));
        }
        Err(err) => return Err(err),
    };
    if let Some(event_name) = codex_lifecycle_event_name(command) {
        if payload.is_object()
            && payload.get("hook_event_name").is_none()
            && payload.get("event").is_none()
        {
            payload["hook_event_name"] = json!(event_name);
        }
    }
    match canonical {
        "pre-tool-use" => Ok(attach_codex_hook_observation(run_codex_pre_tool_use(
            repo_root, &payload,
        )?)),
        "contract-guard" => Ok(attach_codex_hook_observation(run_codex_contract_guard(
            repo_root, &payload,
        )?)),
        "lifecycle-context" => Ok(attach_codex_hook_observation(
            run_codex_lifecycle_context_hook(repo_root, &payload)?,
        )),
        _ => Err(FrameworkError::unsupported(format!("Unsupported Codex audit command: {command}"))),
    }
}

pub fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn run_codex_pre_tool_use(repo_root: &Path, payload: &Value) -> router_rs::framework_error::FrameworkResult<Option<Value>> {
    run_pre_tool_use(repo_root, payload)
}

pub fn run_codex_contract_guard(repo_root: &Path, payload: &Value) -> router_rs::framework_error::FrameworkResult<Option<Value>> {
    let envelope = build_framework_contract_summary_envelope(repo_root)?;
    let summary = envelope
        .get("contract_summary")
        .ok_or_else(|| FrameworkError::not_found("framework contract summary missing contract_summary"))?;
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

pub fn canonical_codex_audit_command(command: &str) -> router_rs::framework_error::FrameworkResult<&'static str> {
    if let Some(event_name) = codex_lifecycle_event_name(command) {
        if event_name == "PreToolUse" {
            return Ok("pre-tool-use");
        }
        return Ok("lifecycle-context");
    }
    match command {
        "pre-tool-use" => Ok("pre-tool-use"),
        "contract-guard" => Ok("contract-guard"),
        "lifecycle-context" | "review-subagent-gate" => Ok("lifecycle-context"),
        _ => Err(FrameworkError::unsupported(format!("Unsupported Codex audit command: {command}"))),
    }
}

pub fn codex_lifecycle_event_name(command: &str) -> Option<&'static str> {
    match command.trim().to_ascii_lowercase().as_str() {
        "sessionstart" => Some("SessionStart"),
        "pretooluse" => Some("PreToolUse"),
        "userpromptsubmit" => Some("UserPromptSubmit"),
        "posttooluse" => Some("PostToolUse"),
        "stop" => Some("Stop"),
        "subagentstart" => Some("SubagentStart"),
        "subagentstop" => Some("SubagentStop"),
        _ => None,
    }
}

pub fn detect_contract_drift(summary: &Value, payload: &Value) -> Vec<String> {
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

pub fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn payload_bool(payload: &Value, key: &str) -> bool {
    payload.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub fn scalar_contract_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => String::new(),
    }
}

pub fn string_array(value: Option<&Value>) -> Vec<String> {
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

pub fn normalized_string_set(values: &[String]) -> Vec<String> {
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

pub fn block_codex_pre_tool_use(reason: String) -> Option<Value> {
    Some(json!({
        "decision": "block",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    }))
}

pub fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> router_rs::framework_error::FrameworkResult<Option<Value>> {
    let ctx = router_rs::hook_common::path_guard::PathGuardContext::from_repo(repo_root);
    let mut rel_paths = HashSet::new();
    for path in iter_payload_paths(payload) {
        rel_paths.insert(router_rs::hook_common::path_guard::relative_candidate_path(
            &path,
            Some(repo_root),
        ));
    }
    for path in rel_paths.iter().cloned().collect::<Vec<_>>() {
        if classify_protected_generated_path(repo_root, &path, &ctx).is_some() {
            let message = pre_tool_use_message(&path);
            return Ok(block_codex_pre_tool_use(message));
        }
    }
    if let Some(path) = bash_generated_write_target(payload, &ctx) {
        let message = pre_tool_use_message(&path);
        return Ok(block_codex_pre_tool_use(message));
    }
    Ok(None)
}

pub fn codex_lifecycle_input_error(message: &str) -> Value {
    json!({
        "decision": "block",
        "message": message,
        "reason": message,
        "hookSpecificOutput": {
            "hookEventName": "CodexLifecycleContext",
            "permissionDecision": "deny",
            "permissionDecisionReason": message,
        },
    })
}

pub fn iter_candidate_paths(payload: &Value) -> Vec<String> {
    let mut candidates = Vec::new();
    for key in [
        "file_path",
        "changed_path",
        "path",
        "config_path",
        "target_path",
    ] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            let normalized = text.replace('\\', "/");
            if !normalized.is_empty() {
                candidates.push(normalized);
            }
        }
    }
    if let Some(items) = payload.get("changed_files").and_then(Value::as_array) {
        for item in items {
            if let Some(text) = item.as_str() {
                let normalized = text.replace('\\', "/");
                if !normalized.is_empty() {
                    candidates.push(normalized);
                }
            }
        }
    }
    candidates
}

pub fn iter_payload_paths(payload: &Value) -> Vec<String> {
    let mut candidates = iter_candidate_paths(payload);
    if let Some(tool_input) = payload.get("tool_input") {
        candidates.extend(iter_candidate_paths(tool_input));
    }
    candidates
}

#[allow(dead_code)]
pub fn relative_candidate_path(path: &str, repo_root: &Path) -> String {
    router_rs::hook_common::path_guard::relative_candidate_path(path, Some(repo_root))
}

pub fn normalize_repo_relative_path(path: &str) -> String {
    router_rs::hook_common::path_guard::normalize_repo_relative_path(path)
}

pub fn classify_protected_generated_path(
    repo_root: &Path,
    path: &str,
    ctx: &router_rs::hook_common::path_guard::PathGuardContext,
) -> Option<&'static str> {
    router_rs::hook_common::path_guard::classify_protected_path(
        path,
        Some(repo_root),
        ctx.runtime_root.as_deref(),
        ctx.active_skill_dir.as_deref(),
    )
}

pub fn pre_tool_use_message(path: &str) -> String {
    format!(
        "[codex-pre-tool-use] blocked direct edits to generated Codex agent surface {path}; rerun `{}` instead."
        ,
        HOST_ENTRYPOINT_SYNC_HINT
    )
}

pub fn bash_generated_write_target(
    payload: &Value,
    ctx: &router_rs::hook_common::path_guard::PathGuardContext,
) -> Option<String> {
    let tool_name = payload.get("tool_name").and_then(Value::as_str)?;
    if tool_name != "Bash" {
        return None;
    }
    let command = payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|tool_input| tool_input.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)?;
    for segment in split_bash_segments(command) {
        let looks_mutating = bash_command_looks_mutating(&segment);
        for hint in protected_generated_paths() {
            if bash_segment_mentions_generated_path(&segment, hint)
                && (looks_mutating || bash_segment_redirects_to_hint(&segment, hint))
                && router_rs::hook_common::path_guard::protected_path_block_reason_with_context(
                    hint, ctx,
                )
                .is_some()
            {
                return Some(hint.to_string());
            }
        }
    }
    None
}

pub fn split_bash_segments(command: &str) -> Vec<String> {
    let chars = command.chars().collect::<Vec<_>>();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut idx = 0usize;

    while idx < chars.len() {
        let current = chars[idx];
        let next = chars.get(idx + 1).copied();
        let prev = if idx > 0 { Some(chars[idx - 1]) } else { None };
        let mut separator_len = 0usize;

        if current == ';' {
            separator_len = 1;
        } else if next == Some(current) && matches!(current, '&' | '|') {
            separator_len = 2;
        } else if current == '|' && prev != Some('>') {
            separator_len = 1;
        }

        if separator_len > 0 {
            let segment = chars[start..idx].iter().collect::<String>();
            let trimmed = segment.trim();
            if !trimmed.is_empty() {
                segments.push(trimmed.to_string());
            }
            idx += separator_len;
            start = idx;
            continue;
        }

        idx += 1;
    }

    let tail = chars[start..].iter().collect::<String>();
    let trimmed = tail.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }

    if segments.is_empty() {
        vec![command.trim().to_string()]
    } else {
        segments
    }
}

static MUTATING_COMMAND_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"^\s*(mv|cp|install|touch|rm|unlink|truncate)\b",
        r"^\s*ln\b[^\n]*\s-[^\n]*[fs][^\n]*\b",
        r"^\s*git\s+(checkout\s+--|restore\b)",
        r"\bsed\s+-i\b",
        r"\bperl\s+-pi\b",
        r"\bpython3?\s+-c\b",
        r"\bnode\s+-e\b",
        r"\bruby\s+-e\b",
        r"\btee\b",
        r"\bdd\b",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

pub fn bash_command_looks_mutating(command: &str) -> bool {
    MUTATING_COMMAND_PATTERNS.iter().any(|re| re.is_match(command))
}

pub fn bash_segment_mentions_generated_path(segment: &str, hint: &str) -> bool {
    segment
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '\'' | '"' | ';' | '&' | '|'))
        .map(|token| token.trim_start_matches('>').trim_start_matches("of="))
        .any(|token| normalize_repo_relative_path(token) == hint)
}

pub fn bash_segment_redirects_to_hint(segment: &str, hint: &str) -> bool {
    thread_local! {
        static HINT_RE_CACHE: std::cell::RefCell<std::collections::HashMap<String, [Regex; 3]>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    HINT_RE_CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        let regexes = map.entry(hint.to_string()).or_insert_with(|| {
            let escaped = regex::escape(hint);
            let p1 = format!(r#"(>>?|>\|)\s*['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            let p2 = format!(r#"\btee\b(?:\s+-a)?\s+['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            let p3 = format!(r#"\bdd\b[^\n;&|]*\bof=['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            [
                Regex::new(&p1).unwrap(),
                Regex::new(&p2).unwrap(),
                Regex::new(&p3).unwrap(),
            ]
        });
        regexes.iter().any(|re| re.is_match(segment))
    })
}
