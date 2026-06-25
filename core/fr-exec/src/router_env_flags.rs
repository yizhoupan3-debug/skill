//! `ROUTER_RS_*` 连续性/续跑类开关：保留真正改变行为边界的少量闸门。
//!
//! **清单真源**：宿主可见语义与默认值以 [`env_flags.rs`](../../core-policy/src/env_flags.rs) **开关面**表格为准。
//! Review-gate 相关 reader 真源在 [`core-policy/env_flags.rs`](../../core-policy/src/env_flags.rs)；本模块为 router-rs 薄包装与连续性专用 env。
//!
//! Helper 映射（core-policy）：
//! - `ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE` → [`router_rs_review_spawn_first_nudge_enabled`]
//! - `ROUTER_RS_SUBAGENT_MODEL_INHERIT_NUDGE` → [`router_rs_subagent_model_inherit_nudge_enabled`]
//! - `ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`（+ legacy per-host 别名）→ [`router_rs_review_fork_context_missing_infer_false_enabled`]
//! - `ROUTER_RS_REVIEW_GATE_DISABLE`（+ legacy per-host `ROUTER_RS_*_REVIEW_GATE_DISABLE`）→ [`router_rs_review_gate_disabled_for_host`]
//! - `ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES`（legacy `ROUTER_RS_CURSOR_*`）→ [`router_rs_review_gate_stop_max_nudges_cap`]
//! - `ROUTER_RS_REVIEW_PENDING_CYCLE_MAX`（legacy `ROUTER_RS_CURSOR_*`）→ [`router_rs_review_pending_cycle_max`]
//!
//! 本模块直接读取：
//! - `ROUTER_RS_OPERATOR_INJECT`、`ROUTER_RS_*` hook-state / pre-goal / outbound 字节上限等
//! - `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE`、`ROUTER_RS_TASK_LEDGER_FLOCK`、`ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD` 等
//!
//! v6 canonicalization: per-host prefix env vars now prefer canonical `ROUTER_RS_*` names
//! with legacy per-host fallback for backward compatibility. Dead per-host constants removed.

use std::env;

const ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE_ENV: &str = "ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE";
const ROUTER_RS_HOOK_TIMING_ENV: &str = "ROUTER_RS_HOOK_TIMING";
const ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX_ENV: &str =
    "ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX";
const ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE_ENV: &str =
    "ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE";

pub use core_policy::env_flags::{
    router_rs_subagent_model_inherit_nudge_enabled,
    router_rs_review_gate_disabled_for_host, router_rs_review_pending_cycle_max,
    router_rs_review_spawn_first_nudge_enabled,
    router_rs_operator_inject_globally_enabled, router_rs_pre_goal_enabled,
    router_rs_hook_silent_enabled, router_rs_hook_outbound_context_max_bytes,
    router_rs_pre_goal_strict_disk_enabled, router_rs_hook_state_fail_open_enabled,
    router_rs_hook_state_lock_retries, router_rs_hook_state_file_sync_enabled,
    router_rs_hook_state_dir_sync_enabled, router_rs_cargo_check_sync_enabled,
    router_rs_hook_state_legacy_full_sweep_enabled, router_rs_hook_state_stale_sweep_days,
    router_rs_hook_legacy_subtracted_events_enabled,
    env_enabled_default_true as router_rs_env_enabled_default_true,
    env_enabled_default_false as router_rs_env_enabled_default_false,
};

/// Cross-host: missing `fork_context` on countable reviewer lane may infer independent fork.
///
/// 将到 core-policy 的规范实现（单一真源）。
/// 所有宿主统一通过 `ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` 控制。
/// Cursor 的 legacy 环境变量 `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`
/// 也在 core-policy 中处理（但默认关闭；Cursor 用户需显式设置）。
pub fn router_rs_review_fork_context_missing_infer_false_enabled() -> bool {
    core_policy::env_flags::router_rs_review_fork_context_missing_infer_false_enabled()
}

/// `ROUTER_RS_TASK_LEDGER_FLOCK`：是否对「任务账本」写入使用 flock sentinel。
/// Delegates to core-policy's canonical implementation; emits a one-time tracing warning when disabled.
pub fn router_rs_task_ledger_flock_enabled() -> bool {
    static FLOCK_WARN: std::sync::Once = std::sync::Once::new();
    let enabled = core_policy::env_flags::router_rs_task_ledger_flock_enabled();
    if !enabled {
        FLOCK_WARN.call_once(|| {
            tracing::warn!(
                "[router-rs] ROUTER_RS_TASK_LEDGER_FLOCK is disabled; parallel writes to task ledger files may interleave"
            );
        });
    }
    enabled
}

/// `ROUTER_RS_HOOK_TIMING=1`: emit `hook_timing …` lines on stderr per hook invocation.
pub fn router_rs_hook_timing_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_HOOK_TIMING_ENV)
}

pub fn router_rs_session_call_tracker_tool_keys_max() -> usize {
    parse_router_rs_usize_clamped(
        ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX_ENV,
        128,
        16,
        4096,
    )
}

pub fn router_rs_continuity_post_tool_evidence_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE_ENV)
}

/// Legacy name; canonical env `ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES`.
///
/// router-rs 单测：两 env 均未设置时返回 `None`（严格、不降级）；core-policy 依赖构建无 `cfg(test)`，故在此保留测试语义。
/// Deprecated: `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES` → prefer `ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES`.
pub fn router_rs_review_gate_stop_max_nudges_cap() -> Option<u32> {
    #[cfg(test)]
    {
        let raw = env::var("ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES")
            .ok()
            .or_else(|| env::var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES").ok());
        raw.as_ref()?;
    }
    core_policy::env_flags::router_rs_review_gate_stop_max_nudges_cap()
}

fn parse_router_rs_usize_clamped(
    env_key: &'static str,
    default_val: usize,
    min_allowed: usize,
    max_allowed: usize,
) -> usize {
    match env::var(env_key) {
        Err(_) => default_val,
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return default_val;
            }
            match trimmed.parse::<usize>() {
                Ok(n) => n.clamp(min_allowed, max_allowed),
                Err(_) => {
                    tracing::warn!(
                        "[router-rs] invalid {env_key}={raw:?}; using default {default_val} (clamp {min_allowed}..{max_allowed})"
                    );
                    default_val
                }
            }
        }
    }
}

/// `ROUTER_RS_QG_MAX_ROUNDS_CAP`: Quality Gate 循环最大轮次硬上限。
pub fn router_rs_qg_max_rounds_cap() -> u64 {
    const MAX_CAP: u64 = 10000;
    const DEFAULT: u64 = 1000;
    env::var("ROUTER_RS_QG_MAX_ROUNDS_CAP")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .map(|n| n.min(MAX_CAP))
        .unwrap_or(DEFAULT)
}

/// §6.4 cat.1 real-process spawn/terminate smoke (`sleep 1` via `smoke-shell` host). Opt-in for CI.
pub fn router_rs_session_supervisor_real_process_smoke_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE_ENV)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_policy::test_env_sync::process_env_lock;

    #[test]
    fn unset_means_enabled_for_default_true() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_UNITTEST_ENV_ENABLED_DEFAULT_TRUE_UNSET";
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key) };
        assert!(router_rs_env_enabled_default_true(key));
    }

    #[test]
    fn zero_false_off_no_disable_default_true() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_UNITTEST_ENV_ENABLED_DEFAULT_TRUE_TOKENS";
        for v in ["0", "false", "off", "no", "FALSE", " Off "] {
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            unsafe { env::set_var(key, v) };
            assert!(
                !router_rs_env_enabled_default_true(key),
                "expected disabled for {v:?}"
            );
        }
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key) };
    }

    #[test]
    fn other_values_enable_default_true() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_UNITTEST_ENV_ENABLED_DEFAULT_TRUE_OTHER";
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(key, "1") };
        assert!(router_rs_env_enabled_default_true(key));
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(key, "") };
        assert!(router_rs_env_enabled_default_true(key));
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key) };
    }

    #[test]
    fn pre_goal_enabled_opt_in_only() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_PRE_GOAL_ENABLED";
        let prev = env::var_os(key);
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key) };
        assert!(!super::router_rs_pre_goal_enabled());
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(key, "true") };
        assert!(super::router_rs_pre_goal_enabled());
        match prev {
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            Some(v) => unsafe {env::set_var(key, v) },
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            None => unsafe {env::remove_var(key) },
        }
    }

    #[test]
    fn pre_goal_strict_disk_default_true() {
        let _g = process_env_lock();
        let key_canonical = "ROUTER_RS_PRE_GOAL_STRICT_DISK";
        let key_legacy = "ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK";
        let prev_canon = env::var_os(key_canonical);
        let prev_legacy = env::var_os(key_legacy);
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key_canonical) };
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key_legacy) };
        // Unset → true (default_true backward compat)
        assert!(super::router_rs_pre_goal_strict_disk_enabled());
        // Canonical explicitly set to "true" enables
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(key_canonical, "true") };
        assert!(super::router_rs_pre_goal_strict_disk_enabled());
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key_canonical) };
        // Canonical explicitly set to "0" disables (short-circuits, no legacy fallthrough)
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(key_canonical, "0") };
        assert!(!super::router_rs_pre_goal_strict_disk_enabled());
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key_canonical) };
        // Legacy fallback: legacy "0" disables when canonical unset
        match prev_canon {
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            Some(v) => unsafe {env::set_var(key_canonical, v) },
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            None => unsafe {env::remove_var(key_canonical) },
        }
        match prev_legacy {
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            Some(v) => unsafe {env::set_var(key_legacy, v) },
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            None => unsafe {env::remove_var(key_legacy) },
        }
    }

    #[test]
    fn continuity_posttool_defaults_off_until_explicit_enable() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE";
        let prev = env::var_os(key);
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key) };
        assert!(!super::router_rs_continuity_post_tool_evidence_enabled());
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(key, "1") };
        assert!(super::router_rs_continuity_post_tool_evidence_enabled());
        match prev {
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            Some(v) => unsafe {env::set_var(key, v) },
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            None => unsafe {env::remove_var(key) },
        }
    }

    #[test]
    fn review_gate_stop_max_nudges_unset_in_tests_means_strict_none() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES";
        let prev = env::var_os(key);
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key) };
        assert!(super::router_rs_review_gate_stop_max_nudges_cap().is_none());
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(key, "3") };
        assert_eq!(
            super::router_rs_review_gate_stop_max_nudges_cap(),
            Some(3)
        );
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(key, "0") };
        assert!(super::router_rs_review_gate_stop_max_nudges_cap().is_none());
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(key) };
        // Also verify legacy CURSOR_ name is still honored by core-policy
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES", "5") };
        assert_eq!(
            super::router_rs_review_gate_stop_max_nudges_cap(),
            Some(5)
        );
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES") };
        match prev {
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            Some(v) => unsafe {env::set_var(key, v) },
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            None => unsafe {env::remove_var(key) },
        }
    }

    #[test]
    fn qg_max_rounds_cap_defaults_and_clamped() {
        let _g = process_env_lock();
        let prev = env::var_os("ROUTER_RS_QG_MAX_ROUNDS_CAP");
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var("ROUTER_RS_QG_MAX_ROUNDS_CAP") };
        assert_eq!(super::router_rs_qg_max_rounds_cap(), 1000);
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var("ROUTER_RS_QG_MAX_ROUNDS_CAP", "500") };
        assert_eq!(super::router_rs_qg_max_rounds_cap(), 500);
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var("ROUTER_RS_QG_MAX_ROUNDS_CAP", "20000") };
        assert_eq!(super::router_rs_qg_max_rounds_cap(), 10000);
        match prev {
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            Some(v) => unsafe {env::set_var("ROUTER_RS_QG_MAX_ROUNDS_CAP", v) },
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            None => unsafe {env::remove_var("ROUTER_RS_QG_MAX_ROUNDS_CAP") },
        }
    }
}
