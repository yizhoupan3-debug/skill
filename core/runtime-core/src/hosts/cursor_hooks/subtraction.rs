//! Cursor hooks **减法闭集**：默认 `hooks.json` 不注册的 5 个事件在 dispatch 层显式 no-op。
//!
//! Handler 实现仍保留。恢复完整 dispatch 的路径：
//! - 将事件写回 [`.cursor/hooks.json`](../../../../.cursor/hooks.json)（非空 `command` 条目）→ 自动走 handler；
//! - 或 `ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS=1`（未注册时强制 handler，单测/对照）。

use crate::router_env_flags::router_rs_cursor_hook_legacy_subtracted_events_enabled;
use serde_json::{json, Value};
use std::path::Path;

/// 与 [`.cursor/hooks.json`](../../../../.cursor/hooks.json) / CI parity 一致的注册闭集。
pub const CURSOR_HOOKS_REGISTERED_EVENTS: &[&str] = &[
    "beforeSubmitPrompt",
    "stop",
    "sessionStart",
    "sessionEnd",
    "postToolUse",
    "subagentStart",
    "subagentStop",
];

/// 已从默认 `hooks.json` 移除、dispatch 默认 no-op 的事件（handler 保留）。
pub const CURSOR_HOOKS_SUBTRACTED_EVENTS: &[&str] = &[
    "afterAgentResponse",
    "beforeShellExecution",
    "afterShellExecution",
    "afterFileEdit",
    "preCompact",
];

pub fn cursor_hook_event_is_subtracted(lowered_event: &str) -> bool {
    matches!(
        lowered_event,
        "afteragentresponse"
            | "beforeshellexecution"
            | "aftershellexecution"
            | "afterfileedit"
            | "precompact"
    )
}

fn hook_entry_has_command(entries: &Value) -> bool {
    entries
        .as_array()
        .is_some_and(|arr| arr.iter().any(|entry| {
            entry
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.trim().is_empty())
        }))
}

/// 事件是否在仓库 `.cursor/hooks.json` 中有效注册（键存在且首条 hook 含非空 `command`）。
pub fn cursor_hooks_json_registers_event(repo_root: &Path, lowered_event: &str) -> bool {
    let path = repo_root.join(".cursor/hooks.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(doc) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };
    let Some(hooks) = doc.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    for (key, entries) in hooks {
        if key.trim().eq_ignore_ascii_case(lowered_event) {
            return hook_entry_has_command(entries);
        }
    }
    false
}

pub fn should_noop_subtracted_event(repo_root: &Path, lowered_event: &str) -> bool {
    if !cursor_hook_event_is_subtracted(lowered_event) {
        return false;
    }
    if router_rs_cursor_hook_legacy_subtracted_events_enabled() {
        return false;
    }
    if cursor_hooks_json_registers_event(repo_root, lowered_event) {
        return false;
    }
    true
}

/// 宿主安全的最小通过形态（无副作用：不写 hook-state / ledger / rustfmt）。
pub fn subtracted_event_noop_output(lowered_event: &str) -> Value {
    match lowered_event {
        "beforeshellexecution" => json!({
            "continue": true,
            "permission": "allow"
        }),
        _ => json!({}),
    }
}
