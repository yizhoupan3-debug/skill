use serde_json::{Value, json};
use std::path::Path;

use fr_runtime::runtime_view;
use fr_runtime::constants::CURRENT_ARTIFACT_DIR;
use fr_runtime::types::FrameworkRuntimeView;

/// Thin wrappers to `runtime_view` kept in `mod.rs` for sibling-module access.
pub fn load_framework_runtime_view(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
) -> FrameworkRuntimeView {
    runtime_view::load_framework_runtime_view(repo_root, artifact_root_override, task_id_override)
}

pub fn classify_runtime_continuity(snapshot: &FrameworkRuntimeView) -> Value {
    runtime_view::classify_runtime_continuity(snapshot)
}

pub fn workspace_name_from_root(repo_root: &Path) -> String {
    runtime_view::workspace_name_from_root(repo_root)
}

/// 带可选 task_id 的版本（用于 Desktop MCP session_checkpoint tool）。
///
/// `repointer_focus`: when true, rewrite active/focus/supervisor (explicit user checkpoint).
/// `update_registry_only_if_known`: when true, never append a new registry row for unknown ids.
pub(crate) fn build_automatic_continuity_checkpoint_payload_with_task_id(
    repo_root: &Path,
    task_line: &str,
    summary_text: &str,
    task_id: Option<&str>,
    repointer_focus: bool,
    update_registry_only_if_known: bool,
) -> Value {
    let output_dir = repo_root.join("artifacts").join(CURRENT_ARTIFACT_DIR);
    let task = if task_line.trim().is_empty() {
        "session-checkpoint".to_string()
    } else {
        fr_runtime::util::truncate_utf8_chars(task_line.trim(), 200)
    };
    let summary = if summary_text.trim().is_empty() {
        "Automatic continuity checkpoint. No summary text was provided; refine in the next turn."
            .to_string()
    } else {
        fr_runtime::util::truncate_utf8_chars(summary_text.trim(), 8000)
    };
    let mut payload = json!({
        "output_dir": output_dir.to_string_lossy(),
        "repo_root": repo_root.to_string_lossy(),
        "task": task,
        "summary": summary,
        "phase": "execution",
        "status": "in_progress",
        "focus": repointer_focus,
        "update_registry_only_if_known": update_registry_only_if_known,
        "next_actions": [
            "Open artifacts/current/SESSION_SUMMARY.md on the next session.",
            "Optional: run `router-rs framework snapshot --repo-root <repo>` for a compact runtime read model.",
        ],
        "trace_metadata": {
            "checkpoint_kind": "automatic_stop_hook",
        }
    });
    if let Some(tid) = task_id.filter(|s| !s.is_empty())
        && let Some(obj) = payload.as_object_mut()
    {
        obj.insert("task_id".to_string(), serde_json::json!(tid));
    }
    payload
}

// ── Modules that remain in framework_runtime (deep coupling) ──
pub mod stdio_dispatch;
pub mod tool_handlers;
