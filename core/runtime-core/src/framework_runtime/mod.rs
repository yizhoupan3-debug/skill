use serde_json::Value;
use std::path::Path;

use fr_runtime::runtime_view;
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

// ── Modules that remain in framework_runtime (deep coupling) ──
pub mod stdio_dispatch;
pub mod tool_handlers;
