//! Host hooks snapshot for schema drift: capture and compare hook configurations.
//!
//! All host-specific data (hook paths, events) is passed as parameters —
//! not hardcoded. L4 schema drift (runtime-core) stores the result as an opaque JSON blob
//! and compares for equality; it never inspects the struct directly.
//!
//! ## Usage
//!
//! ```ignore
//! let snap = snapshot_host_hooks(
//!     repo_root,
//!     hooks_path,                                    // from HostProvider::hooks_manifest_path()
//!     template_path,                                 // e.g. "configs/framework/{host_id}-hooks.workspace-template.json"
//!     &host_registered_hook_events(host_id),
//!     &[],                                           // from HostLifecycle::subtracted_hook_events()
//!     "{host_id}-router-rs-hook.sh",
//! )?;
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

// ---------------------------------------------------------------------------
// Snapshot type
// ---------------------------------------------------------------------------

/// Snapshot of a host's hook configuration for schema drift comparison.
/// L4 code stores this as an opaque JSON blob; equality comparison is
/// `Value`-level. Test code may inspect individual fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostHooksSnapshot {
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

/// Check whether a hooks snapshot is "ok" (no issues).
pub fn host_hooks_snapshot_ok(s: &HostHooksSnapshot) -> bool {
    s.forbidden_still_registered.is_empty()
        && s.missing_required.is_empty()
        && s.hook_command_issues.is_empty()
        && s.gate_timeout_issues.is_empty()
        && s.hooks_template_parity_issues.is_empty()
}

// ---------------------------------------------------------------------------
// Gate timeout expectations (shared across all hosts that use the same format)
// ---------------------------------------------------------------------------

const GATE_TIMEOUT_SECS: &[(&str, u64)] = &[
    // camelCase events
    ("beforeSubmitPrompt", 20),
    ("stop", 20),
    ("postToolUse", 20),
    ("subagentStart", 20),
    ("subagentStop", 20),
    ("sessionStart", 5),
    ("sessionEnd", 15),
    // kebab-case events
    ("pre-tool-use", 20),
    ("user-prompt-submit", 20),
    // PascalCase events
    ("SessionStart", 5),
    ("PreToolUse", 20),
    ("UserPromptSubmit", 20),
    ("PostToolUse", 20),
    ("Stop", 20),
    ("SubagentStart", 20),
    ("SubagentStop", 20),
    // dot-separated events
    ("tool.execute.before", 20),
    ("tool.execute.after", 20),
    ("session.idle", 5),
    ("session.created", 5),
    ("session.deleted", 5),
    ("permission.asked", 20),
    ("permission.replied", 20),
    ("file.edited", 20),
    ("shell.env", 20),
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_hooks_doc(path: &Path) -> Result<Value, String> {
    let raw =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
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

fn audit_hooks_doc(label: &str, doc: &Value, expected_cmd_fragment: &str) -> (Vec<String>, Vec<String>) {
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
        if !cmd.contains(expected_cmd_fragment) {
            command_issues.push(format!("{label}: {ev} must invoke {expected_cmd_fragment}"));
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

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Snapshot a host's hook configuration for schema drift comparison.
///
/// # Parameters
///
/// - `hooks_path` — repo-root-relative path to the host's hooks manifest (from HostProvider::hooks_manifest_path())
/// - `template_path` — repo-root-relative path to the workspace template (e.g. `configs/framework/{host_id}-hooks.workspace-template.json`)
/// - `expected_events` — events that the host must register (from host_registered_hook_events())
/// - `forbidden_events` — events that must NOT be registered (e.g. subtracted events)
pub fn snapshot_host_hooks(
    repo_root: &Path,
    hooks_path: &Path,
    template_path: &Path,
    expected_events: &[&str],
    forbidden_events: &[&str],
    expected_cmd_fragment: &str,
) -> Result<HostHooksSnapshot, String> {
    let abs_hooks = repo_root.join(hooks_path);
    let abs_template = repo_root.join(template_path);

    let registered = read_hooks_event_keys(&abs_hooks)?;
    let template_keys = read_hooks_event_keys(&abs_template)?;
    let hooks_doc = read_hooks_doc(&abs_hooks)?;
    let (mut hook_command_issues, mut gate_timeout_issues) =
        audit_hooks_doc(&hooks_path.display().to_string(), &hooks_doc, expected_cmd_fragment);
    let mut hooks_template_parity_issues = Vec::new();
    if abs_template.is_file() {
        if let Ok(template_doc) = read_hooks_doc(&abs_template) {
            let (cmd_i, to_i) = audit_hooks_doc(
                &template_path.display().to_string(),
                &template_doc,
                expected_cmd_fragment,
            );
            hook_command_issues.extend(cmd_i);
            gate_timeout_issues.extend(to_i);
            hooks_template_parity_issues =
                compare_hooks_template_parity(&hooks_doc, &template_doc);
        }
    } else {
        hooks_template_parity_issues.push(format!(
            "missing {}",
            template_path.display()
        ));
    }
    let missing_required: Vec<String> = expected_events
        .iter()
        .filter(|ev| !registered.contains(&ev.to_string()))
        .map(|s| (*s).to_string())
        .collect();
    let forbidden_still_registered: Vec<String> = forbidden_events
        .iter()
        .filter(|ev| registered.contains(&ev.to_string()))
        .map(|s| (*s).to_string())
        .collect();
    let keys_match = registered == template_keys;
    Ok(HostHooksSnapshot {
        registered_events: registered,
        forbidden_still_registered,
        missing_required,
        matches_workspace_template: keys_match && hooks_template_parity_issues.is_empty(),
        hook_command_issues,
        gate_timeout_issues,
        hooks_template_parity_issues,
    })
}

/// Build a JSON Value blob from a host hooks snapshot (for baseline storage).
pub fn snapshot_host_hooks_json(
    repo_root: &Path,
    hooks_path: &Path,
    template_path: &Path,
    expected_events: &[&str],
    forbidden_events: &[&str],
    expected_cmd_fragment: &str,
) -> Result<Value, String> {
    let snap = snapshot_host_hooks(repo_root, hooks_path, template_path, expected_events, forbidden_events, expected_cmd_fragment)?;
    serde_json::to_value(snap).map_err(|e| format!("serialize snapshot: {e}"))
}

/// Check a host hooks snapshot JSON blob for validity.
pub fn host_hooks_json_ok(hooks: &Value) -> bool {
    let Some(s) = hooks.as_object() else { return false };
    s.get("forbidden_still_registered")
        .and_then(Value::as_array)
        .map(|a| a.is_empty())
        .unwrap_or(false)
        && s.get("missing_required")
            .and_then(Value::as_array)
            .map(|a| a.is_empty())
            .unwrap_or(false)
        && s.get("hook_command_issues")
            .and_then(Value::as_array)
            .map(|a| a.is_empty())
            .unwrap_or(false)
        && s.get("gate_timeout_issues")
            .and_then(Value::as_array)
            .map(|a| a.is_empty())
            .unwrap_or(false)
        && s.get("hooks_template_parity_issues")
            .and_then(Value::as_array)
            .map(|a| a.is_empty())
            .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Unified host projection verification (registry-driven)
// ---------------------------------------------------------------------------

/// Unified verification of a host's projection installation.
///
/// All host-specific data (hooks path, events, launcher) is derived from the
/// HostProvider registry. No per-host hardcoded paths or match arms.
///
/// Checks:
/// 1. If host has hooks_manifest_path → verify hooks.json exists and has correct structure
/// 2. If host has registered_hook_events → verify all events are registered in hooks.json
/// 3. Launcher command pattern derived from host_id
pub fn verify_host_projection(repo_root: &Path, host_id: &str) -> Result<(), String> {
    let provider = crate::hosts::host_provider_for_id(host_id)
        .ok_or_else(|| format!("verify_host_projection: unknown host {host_id}"))?;

    let hooks_manifest_path = provider.hooks_manifest_path();

    // If host has hooks.json, verify it
    if let Some(hooks_rel) = hooks_manifest_path {
        let hooks_path = repo_root.join(hooks_rel);
        if !hooks_path.is_file() {
            return Err(format!(
                "verify_{host_id}: missing {hooks_rel}"
            ));
        }

        let text = std::fs::read_to_string(&hooks_path)
            .map_err(|e| format!("verify_{host_id}: read {hooks_rel}: {e}"))?;
        let payload: Value = serde_json::from_str(&text)
            .map_err(|e| format!("verify_{host_id}: parse {hooks_rel}: {e}"))?;
        let hooks = payload
            .get("hooks")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                format!("verify_{host_id}: {hooks_rel} must contain a hooks object")
            })?;

        let expected_events = provider.registered_hook_events();
        let launcher_needle = format!("{host_id}-router-rs-hook.sh");

        for event in expected_events {
            let entries = hooks
                .get(*event)
                .and_then(Value::as_array)
                .filter(|a| !a.is_empty())
                .ok_or_else(|| format!("verify_{host_id}: missing hook event {event}"))?;
            let cmds: Vec<&str> = entries
                .iter()
                .filter_map(|entry| entry.get("command").and_then(Value::as_str))
                .collect();
            if cmds.is_empty() {
                return Err(format!(
                    "verify_{host_id}: event {event} must contain command hooks"
                ));
            }
            if !cmds.iter().any(|c| c.contains(&launcher_needle)) {
                return Err(format!(
                    "verify_{host_id}: {event} must invoke `{launcher_needle}` (see {hooks_rel})"
                ));
            }
        }
    }

    Ok(())
}
