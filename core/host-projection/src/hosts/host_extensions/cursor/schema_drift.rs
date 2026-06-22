//! Cursor-specific schema drift: hooks snapshot, audit, workspace template comparison.
//!
//! Extracted from `runtime-exit-gate/src/schema_drift.rs` per ADR-010 §4
//! host isolation — cursor-specific logic belongs in L0 host_extensions.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use super::{CURSOR_HOOKS_REGISTERED_EVENTS, CURSOR_HOOKS_SUBTRACTED_EVENTS};

// ── Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CursorHooksDriftSnapshot {
    pub registered_events: Vec<String>,
    pub forbidden_still_registered: Vec<String>,
    pub missing_required: Vec<String>,
    pub matches_workspace_template: bool,
    #[serde(default)]
    pub hook_command_issues: Vec<String>,
    #[serde(default)]
    pub gate_timeout_issues: Vec<String>,
    #[serde(default)]
    pub hooks_template_parity_issues: Vec<String>,
}

// ── Helpers ──

const GATE_TIMEOUT_SECS: &[(&str, u64)] = &[
    ("beforeSubmitPrompt", 20),
    ("stop", 20),
    ("postToolUse", 20),
    ("subagentStart", 20),
    ("subagentStop", 20),
    ("sessionStart", 5),
    ("sessionEnd", 15),
];

fn read_hooks_doc(path: &Path) -> Result<Value, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))
}

fn read_hooks_event_keys(path: &Path) -> Result<Vec<String>, String> {
    let doc = read_hooks_doc(path)?;
    let mut keys: Vec<String> = doc
        .get("hooks")
        .and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    keys.sort();
    Ok(keys)
}

fn first_hook_entry<'a>(
    hooks: &'a serde_json::Map<String, Value>,
    event: &str,
) -> Option<&'a Value> {
    hooks.get(event)?.as_array()?.first()
}

fn audit_hooks_doc(label: &str, doc: &Value) -> (Vec<String>, Vec<String>) {
    let mut command_issues = Vec::new();
    let mut timeout_issues = Vec::new();
    let Some(hooks) = doc.get("hooks").and_then(Value::as_object) else {
        return (
            vec![format!("{label}: missing hooks object")],
            timeout_issues,
        );
    };
    for (ev, want) in GATE_TIMEOUT_SECS {
        let Some(entry) = first_hook_entry(hooks, ev) else {
            continue;
        };
        let cmd = entry.get("command").and_then(Value::as_str).unwrap_or("");
        if !cmd.contains("cursor-router-rs-hook.sh") {
            command_issues.push(format!(
                "{label}: {ev} must invoke cursor-router-rs-hook.sh"
            ));
        }
        let timeout = entry.get("timeout").and_then(Value::as_u64);
        if timeout != Some(*want) {
            timeout_issues.push(format!(
                "{label}: {ev} timeout must be {want}s (got {timeout:?})"
            ));
        }
    }
    (command_issues, timeout_issues)
}

fn compare_hooks_template_parity(hooks_doc: &Value, template_doc: &Value) -> Vec<String> {
    let mut issues = Vec::new();
    let (Some(hooks), Some(template)) = (
        hooks_doc.get("hooks").and_then(Value::as_object),
        template_doc.get("hooks").and_then(Value::as_object),
    ) else {
        issues.push("hooks vs template: missing hooks object".to_string());
        return issues;
    };

    let mut h_keys: Vec<_> = hooks.keys().cloned().collect();
    h_keys.sort();
    let mut t_keys: Vec<_> = template.keys().cloned().collect();
    t_keys.sort();
    if h_keys != t_keys {
        issues.push(format!(
            "event key mismatch: hooks={h_keys:?} template={t_keys:?}"
        ));
    }

    for ev in h_keys.iter().filter(|k| template.contains_key(*k)) {
        let (Some(he), Some(te)) = (first_hook_entry(hooks, ev), first_hook_entry(template, ev))
        else {
            issues.push(format!("{ev}: missing hook entry in hooks or template"));
            continue;
        };
        let hc = he.get("command").and_then(Value::as_str).unwrap_or("");
        let tc = te.get("command").and_then(Value::as_str).unwrap_or("");
        if hc != tc {
            issues.push(format!(
                "command mismatch on {ev}: hooks={hc:?} template={tc:?}"
            ));
        }
        let ht = he.get("timeout");
        let tt = te.get("timeout");
        if ht != tt {
            issues.push(format!(
                "timeout mismatch on {ev}: hooks={ht:?} template={tt:?}"
            ));
        }
    }
    issues
}

// ── Public API ──

/// Snapshot cursor hooks configuration from disk.
pub fn snapshot_cursor_hooks(repo_root: &Path) -> Result<CursorHooksDriftSnapshot, String> {
    let hooks_path = repo_root.join(".cursor/hooks.json");
    let template_path = repo_root.join("configs/framework/cursor-hooks.workspace-template.json");
    let registered = read_hooks_event_keys(&hooks_path)?;
    let template_keys = read_hooks_event_keys(&template_path)?;
    let hooks_doc = read_hooks_doc(&hooks_path)?;
    let (mut hook_command_issues, mut gate_timeout_issues) =
        audit_hooks_doc(".cursor/hooks.json", &hooks_doc);
    let mut hooks_template_parity_issues = Vec::new();
    if template_path.is_file() {
        if let Ok(template_doc) = read_hooks_doc(&template_path) {
            let (cmd_i, to_i) = audit_hooks_doc("workspace-template", &template_doc);
            hook_command_issues.extend(cmd_i);
            gate_timeout_issues.extend(to_i);
            hooks_template_parity_issues =
                compare_hooks_template_parity(&hooks_doc, &template_doc);
        }
    } else {
        hooks_template_parity_issues
            .push("missing configs/framework/cursor-hooks.workspace-template.json".to_string());
    }
    let missing_required: Vec<String> = CURSOR_HOOKS_REGISTERED_EVENTS
        .iter()
        .filter(|ev| !registered.contains(&ev.to_string()))
        .map(|s| (*s).to_string())
        .collect();
    let forbidden_still_registered: Vec<String> = CURSOR_HOOKS_SUBTRACTED_EVENTS
        .iter()
        .filter(|ev| registered.contains(&ev.to_string()))
        .map(|s| (*s).to_string())
        .collect();
    let keys_match = registered == template_keys;
    Ok(CursorHooksDriftSnapshot {
        registered_events: registered.clone(),
        forbidden_still_registered,
        missing_required,
        matches_workspace_template: keys_match && hooks_template_parity_issues.is_empty(),
        hook_command_issues,
        gate_timeout_issues,
        hooks_template_parity_issues,
    })
}

/// Check whether a cursor hooks drift snapshot is "ok" (no issues).
pub fn cursor_hooks_snapshot_ok(s: &CursorHooksDriftSnapshot) -> bool {
    s.forbidden_still_registered.is_empty()
        && s.missing_required.is_empty()
        && s.hook_command_issues.is_empty()
        && s.gate_timeout_issues.is_empty()
        && s.hooks_template_parity_issues.is_empty()
}
