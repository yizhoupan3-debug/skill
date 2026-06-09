//! E7 step 2: PreToolUse handler extracted from `claude_hooks` monolith.

use serde_json::{json, Value};
use std::path::Path;

use super::{
    deny_pre_tool_use, is_cross_host_or_retired_surface, is_framework_guarded_path,
    is_generated_entrypoint, is_host_private_path, is_settings_path, payload_relative_paths,
};

/// Claude PreToolUse decision: `None` → allow (silent); `Some` → deny or warn context.
pub fn evaluate_claude_pre_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    if router_rs::router_env_flags::router_rs_skip_pre_tool_use_guard() {
        return None;
    }
    let mut warn_contexts: Vec<String> = Vec::new();
    for path in payload_relative_paths(repo_root, payload) {
        if is_cross_host_or_retired_surface(&path) {
            return deny_pre_tool_use(format!(
                "Blocked direct mutation of cross-host or retired surface {path}; use the Rust host-entrypoint sync path instead."
            ));
        }
        if is_generated_entrypoint(&path) {
            return deny_pre_tool_use(format!(
                "Blocked direct mutation of generated host entrypoint {path}; use the Rust host-entrypoint sync path instead."
            ));
        }
        if is_framework_guarded_path(&path) {
            return deny_pre_tool_use(format!(
                "Blocked direct mutation of framework routing/runtime file {path}; use the Rust host-entrypoint sync or routing path instead."
            ));
        }
        if is_host_private_path(&path) {
            return deny_pre_tool_use(format!(
                "Blocked direct mutation of host-private agent state {path}; project policy must live in repo settings or Rust runtime code."
            ));
        }
        if is_settings_path(&path) {
            warn_contexts.push(format!(
                "Modifying {path} — ensure JSON validity before finishing (jq . or python -m json.tool)."
            ));
        } else if path == "AGENTS_CLAUDE.md" {
            warn_contexts.push(format!(
                "Modifying {path} — this is a cross-host strategy document; ensure consistency across all hosts."
            ));
        } else if path == "skills/SKILL_ROUTING_RUNTIME.json" || path == "skills/SKILL_MANIFEST.json" {
            warn_contexts.push(format!(
                "Modifying {path} — framework routing core data source; run `framework skills refresh --validate` after changes."
            ));
        }
    }
    if !warn_contexts.is_empty() {
        return Some(json!({
            "suppressOutput": true,
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "additionalContext": warn_contexts.join("\n"),
            },
        }));
    }
    None
}
