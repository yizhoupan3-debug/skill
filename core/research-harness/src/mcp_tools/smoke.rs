//! Smoke test runner tool.
//!
//! # Functions
//! - `tool_research_smoke` — run experiments defined by templates and params

use core_errors::FrameworkError;
use serde_json::Value;

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
    let repo_root = crate::mcp_tools::resolve_repo_root();
    crate::smoke::run_smoke_tests(&repo_root, arguments)
}
