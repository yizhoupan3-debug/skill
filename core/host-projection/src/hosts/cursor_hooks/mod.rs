use core_policy::hook_common::{
    has_override, is_parallel_delegation_prompt, is_review_prompt, normalize_subagent_type,
    normalize_tool_name, saw_reject_reason, strip_quoted_or_codeblock_or_url,
};
use core_policy::review_gate_engine::{fork_context_from_values, review_gate_armed};
use core_policy::review_output_lint::{lint_review_output, LintSeverity};
use crate::hooks;
use crate::hooks::MAX_CONCURRENT_SUBAGENTS_LIMIT;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod repo_root;
mod stdin;

pub use repo_root::resolve_cursor_hook_repo_root;
pub use stdin::read_cursor_hook_stdin_json;

thread_local! {
    /// 并行单测下替代进程级 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE`，避免 env 竞态。
    static TEST_CURSOR_REVIEW_GATE_DISABLE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// 与运行时「subagent 并发上限契约」对齐（`runtime_envelope_ids::MAX_CONCURRENT_SUBAGENTS_LIMIT`）；可用 `ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS` 调低或设为 `0` 关闭计数限流。
const DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS: u32 = MAX_CONCURRENT_SUBAGENTS_LIMIT as u32;
const DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS: i64 = 15 * 60;

/// Shell 钩子 pending 队列长度上限，防止极长会话把 ledger 胀得过大。
const MAX_PENDING_SHELL_RECORDS: usize = 64;
/// `started_at` 与 pending `queued_ms` 对齐允许的时钟/调度 slack（毫秒）。
const SHELL_TERMINAL_TIME_MATCH_SLACK_MS: u64 = 10_000;

/// Set test override for cursor review gate disable (used by integration tests).
pub fn set_test_review_gate_disable_override(v: Option<bool>) {
    TEST_CURSOR_REVIEW_GATE_DISABLE.with(|c| c.set(v));
}

thread_local! {
    static FORCE_CURSOR_HOOK_STATE_LOCK_FAILURE_FOR_TEST: Cell<bool> =
        const { Cell::new(false) };
}

/// 仅限单测：`acquire_state_lock` 直接失败，校验「hook-state 锁不可用」降级路径。
pub fn set_force_cursor_hook_state_lock_failure(v: bool) {
    FORCE_CURSOR_HOOK_STATE_LOCK_FAILURE_FOR_TEST.with(|c| c.set(v));
}

fn should_force_hook_state_lock_failure_for_test() -> bool {
    FORCE_CURSOR_HOOK_STATE_LOCK_FAILURE_FOR_TEST.with(|c| c.get())
}

mod subtraction;
mod terminal_observation_cache;

pub use subtraction::{CURSOR_HOOKS_REGISTERED_EVENTS, CURSOR_HOOKS_SUBTRACTED_EVENTS};

// --- cursor hooks handlers ---
include!("handlers.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::Once;

    static TEST_DEPS_ONCE: Once = Once::new();

    /// Install tokenizer + review context probes. Called from guard constructors
    /// and from `ensure_kernel_bootstrap()` fallback in test builds.
    pub(crate) fn ensure_test_deps() {
        TEST_DEPS_ONCE.call_once(|| {
            crate::hooks::install_test_deps();
        });
    }

    /// Guard that ensures test deps are installed on first use.
    /// Drop this at the start of any test that uses review/arbitration functions.
    pub(crate) struct TestDepsGuard;
    impl TestDepsGuard {
        pub(crate) fn new() -> Self {
            ensure_test_deps();
            Self
        }
    }

    include!("tests.rs");
}
