//! Shared state management & evidence recording for all 4 hosts.
//!
//! Extracted from `hook_dispatch.rs` as part of the modularization.

use serde_json::Value;
use std::path::Path;

/// Generic disk state indicator for hook state files.
pub enum AgentDiskState<T> {
    Absent,
    Ok(T),
    Unreadable,
}

/// Shared touch state tracking (settings/framework file modifications).
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct TouchState {
    pub settings: bool,
    pub framework: bool,
    pub settings_validated: bool,
    pub framework_tested: bool,
}

/// Write HookReviewDiskCore to disk with version bump.
pub fn write_review_state_unlocked(path: &Path, state: &core_policy::HookReviewDiskCore) -> Result<(), String> {
    let mut to_write = state.clone();
    to_write.bump_version_for_save();
    let mut body = serde_json::to_string(&to_write).map_err(|e| e.to_string())?;
    body.push('\n');
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, &body).map_err(|e| e.to_string())
}

/// Read HookReviewDiskCore from disk with migration support.
pub fn read_review_gate_file(path: &Path) -> AgentDiskState<core_policy::HookReviewDiskCore> {
    if !path.is_file() {
        return AgentDiskState::Absent;
    }
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return AgentDiskState::Unreadable,
    };
    if raw.trim().is_empty() {
        return AgentDiskState::Unreadable;
    }
    let value: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return AgentDiskState::Unreadable,
    };
    AgentDiskState::Ok(core_policy::migrate_hook_review_disk_core(&value))
}

/// Read TouchState from disk.
pub fn load_touch_state_from_path(touch_path: &Path) -> AgentDiskState<TouchState> {
    if !touch_path.is_file() {
        return AgentDiskState::Absent;
    }
    match std::fs::read_to_string(touch_path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(state) => AgentDiskState::Ok(state),
            Err(_) => AgentDiskState::Unreadable,
        },
        Err(_) => AgentDiskState::Unreadable,
    }
}

/// Determine if review gate should sync on UserPromptSubmit.
pub fn should_sync_review_gate_on_user_prompt(repo_root: &Path, prompt: &str) -> bool {
    core_policy::hook_common::is_interactive_profile(Some(repo_root), prompt)
        || core_policy::hook_common::is_framework_goal_entry_prompt(prompt)
        || core_policy::hook_common::is_my_pre_execution_entry_prompt(prompt)
        || core_policy::hook_common::is_narrow_review_prompt(prompt)
        || core_policy::hook_common::is_review_prompt(prompt)
        || core_policy::hook_common::has_override(prompt)
}

/// Auto-record verification evidence from a Bash tool call.
pub fn auto_record_verification_evidence(repo_root: &Path, payload: &Value) {
    let tool_name = payload.get("tool_name").and_then(Value::as_str).unwrap_or_default();
    let Some(command) = crate::hosts::hook_dispatch::bash_command(payload) else { return; };
    let cmd_trimmed = command.trim();
    if !crate::hosts::hook_dispatch::is_verification_command(tool_name, cmd_trimmed) { return; }
    let exit_code = crate::hosts::hook_dispatch::payload_exit_code(payload);
    let output_summary = crate::hosts::hook_dispatch::extract_output_summary(payload, 500);
    let mut entry = serde_json::Map::new();
    entry.insert("kind".to_string(), serde_json::json!("auto_evidence"));
    entry.insert("source".to_string(), serde_json::json!("post_tool_use_auto"));
    entry.insert("tool_name".to_string(), serde_json::json!("Bash"));
    entry.insert("command_preview".to_string(), serde_json::json!(cmd_trimmed));
    entry.insert("recorded_at".to_string(), serde_json::json!(crate::hooks::current_local_timestamp()));
    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), serde_json::json!(ec));
        entry.insert("success".to_string(), serde_json::json!(ec == 0));
    }
    if let Some(ref text) = output_summary {
        entry.insert("output".to_string(), serde_json::json!(text));
    }
    let _ = crate::hooks::append_evidence_index(repo_root, None, entry);
}

/// Auto-record research activity from tool calls.
pub fn auto_record_research_activity(repo_root: &Path, payload: &Value) {
    let tool_name = payload.get("tool_name").or_else(|| payload.get("tool"))
        .and_then(Value::as_str).unwrap_or_default();
    let summary = match tool_name {
        "Bash" => crate::hosts::hook_dispatch::bash_command(payload).unwrap_or_default().to_string(),
        "WebFetch" | "web_fetch" => payload.get("tool_input")
            .and_then(|ti| ti.get("url")).and_then(Value::as_str)
            .unwrap_or_default().to_string(),
        other => other.to_string(),
    };
    if summary.is_empty() || summary == tool_name { return; }
    crate::hooks::maybe_record_research_activity(repo_root, tool_name, &summary);
}

/// Standard closeout check (delegates to hooks proxy).
pub fn closeout_check_shared(repo_root: &Path, text: &str) -> Option<String> {
    crate::hooks::closeout_stop_followup_for_completion_text(repo_root, text)
}

/// Standard dispatch bootstrap.
pub fn ensure_dispatch_bootstrap_shared() {
    crate::hooks::ensure_kernel_bootstrap();
}
