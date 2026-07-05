//! Smoke test runner tool.
//!
//! # Functions
//! - `tool_research_smoke` — run experiments defined by templates and params
//! - `resolve_repo_root` — walk up from CWD to find the project root

use core_errors::FrameworkError;
use serde_json::Value;
use std::path::PathBuf;

/// Resolve the framework project root by walking up from CWD.
/// Returns the first ancestor that contains `templates/` or `.git`,
/// falling back to an absolute CWD.
fn resolve_repo_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = Some(cwd.as_path());
        while let Some(d) = dir {
            if d.join("templates").exists() || d.join(".git").exists() {
                return d.to_path_buf();
            }
            dir = d.parent();
        }
    }
    // Ultimate fallback: absolute CWD
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Run smoke tests using template-based experiment definitions.
/// Backward compat guard rejects old `source`/`barrier_id` parameters.
pub(super) fn tool_research_smoke(arguments: &Value) -> Result<String, FrameworkError> {
    // Backward compat guard: old interface (source/barrier_id) is gone
    if arguments.get("source").is_some() || arguments.get("barrier_id").is_some() {
        return Err(FrameworkError::validation(
            "research_smoke 已升级为通用实验引擎。旧参数 source/barrier_id 不再支持。 \
             请使用 template (string) + params (array of {key: value, ...})。 \
             templates/ 目录下存放可执行实验模板。",
        ));
    }
    let repo_root = resolve_repo_root();
    crate::smoke::run_smoke_tests(&repo_root, arguments)
}
