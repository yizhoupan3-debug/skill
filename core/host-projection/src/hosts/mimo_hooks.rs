use serde_json::Value;
use std::path::Path;

/// Registered hook events for mimo (minimal set, matching cursor pattern).
pub const MIMO_HOOKS_REGISTERED_EVENTS: &[&str] = &["PreToolUse", "Stop"];

/// Run mimo hook event dispatch.
pub fn run_mimo_hook(event: &str, repo_root: Option<&Path>) -> Result<Option<Value>, String> {
    // Minimal hook — allow all by default
    let _ = event;
    let _ = repo_root;
    Ok(None)
}
