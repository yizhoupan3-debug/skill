use crate::hook_common::{
    has_delegation_override, has_override, has_review_override, is_parallel_delegation_prompt,
    is_review_prompt, normalize_subagent_type, normalize_tool_name, saw_reject_reason,
    strip_quoted_or_codeblock_or_url,
};
use crate::review_gate_engine::{
    fork_context_from_values, independent_context_fork, review_gate_armed,
};
use crate::runtime_envelope_ids::MAX_CONCURRENT_SUBAGENTS_LIMIT;
use chrono::{DateTime, Utc};
use fs2::FileExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod repo_root;
mod stdin;

pub use repo_root::resolve_cursor_hook_repo_root;
pub(crate) use stdin::read_cursor_hook_stdin_json;

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    /// 并行单测下替代进程级 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE`，避免 env 竞态。
    static TEST_CURSOR_REVIEW_GATE_DISABLE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// 与运行时「subagent 并发上限契约」对齐（`runtime_envelope_ids::MAX_CONCURRENT_SUBAGENTS_LIMIT`）；可用 `ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS` 调低或设为 `0` 关闭计数限流。
const DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS: u32 = MAX_CONCURRENT_SUBAGENTS_LIMIT as u32;
const DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS: i64 = 2 * 60 * 60;

/// Shell 钩子 pending 队列长度上限，防止极长会话把 ledger 胀得过大。
const MAX_PENDING_SHELL_RECORDS: usize = 64;
/// `started_at` 与 pending `queued_ms` 对齐允许的时钟/调度 slack（毫秒）。
const SHELL_TERMINAL_TIME_MATCH_SLACK_MS: u64 = 10_000;

#[cfg(test)]
pub(crate) fn set_test_review_gate_disable_override(v: Option<bool>) {
    TEST_CURSOR_REVIEW_GATE_DISABLE.with(|c| c.set(v));
}

#[cfg(test)]
thread_local! {
    static FORCE_CURSOR_HOOK_STATE_LOCK_FAILURE_FOR_TEST: Cell<bool> =
        const { Cell::new(false) };
}

/// 仅限单测：`acquire_state_lock` 直接失败，校验「hook-state 锁不可用」降级路径。
#[cfg(test)]
pub(crate) fn set_force_cursor_hook_state_lock_failure(v: bool) {
    FORCE_CURSOR_HOOK_STATE_LOCK_FAILURE_FOR_TEST.with(|c| c.set(v));
}

#[cfg(test)]
fn should_force_hook_state_lock_failure_for_test() -> bool {
    FORCE_CURSOR_HOOK_STATE_LOCK_FAILURE_FOR_TEST.with(|c| c.get())
}

// --- split fragments (same module scope via include!) ---
include!("frag_01_continuity_intent.rs");
include!("frag_02_gate_event.rs");
include!("frag_03_paths_terminal_merge_lock_persist.rs");
include!("frag_04_review_gate_runtime.rs");
include!("frag_05_handlers_core.rs");
include!("frag_06_session_terminal_kill.rs");
include!("dispatch.rs");

#[cfg(test)]
mod tests {
    use super::*;
    include!("tests.rs");
}
