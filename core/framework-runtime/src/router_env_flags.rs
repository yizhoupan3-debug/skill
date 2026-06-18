//! `ROUTER_RS_*` 连续性/续跑类开关：保留真正改变行为边界的少量闸门。
//!
//! **清单真源**：宿主可见语义与默认值以 [`env_flags.rs`](../../core-policy/src/env_flags.rs) **开关面**表格为准。
//! Review-gate 相关 reader 真源在 [`core-policy/env_flags.rs`](../../core-policy/src/env_flags.rs)；本模块为 router-rs 薄包装与连续性专用 env。
//!
//! Helper 映射（core-policy）：
//! - `ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE` → [`router_rs_review_spawn_first_nudge_enabled`]
//! - `ROUTER_RS_SUBAGENT_MODEL_INHERIT_NUDGE` → [`router_rs_cursor_subagent_model_inherit_nudge_enabled`]
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
const ROUTER_RS_TASK_LEDGER_FLOCK_ENV: &str = "ROUTER_RS_TASK_LEDGER_FLOCK";
const ROUTER_RS_HOOK_TIMING_ENV: &str = "ROUTER_RS_HOOK_TIMING";
const ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX_ENV: &str =
    "ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX";
const ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD_ENV: &str = "ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD";
const ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE_ENV: &str =
    "ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE";

pub use core_policy::env_flags::{
    router_rs_cursor_subagent_model_inherit_nudge_enabled,
    router_rs_review_fork_context_missing_infer_false_enabled,
    router_rs_review_gate_disabled_for_host, router_rs_review_pending_cycle_max,
    router_rs_review_spawn_first_nudge_enabled,
};

/// My implement **pre-goal** nudge.
/// Canonical `ROUTER_RS_PRE_GOAL_ENABLED`; .
pub fn router_rs_pre_goal_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_PRE_GOAL_ENABLED")
}

/// Hook-state: 是否对 `.cursor/hook-state/` 做**历史全目录前缀清扫**。
/// Legacy env: `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP`.
pub fn router_rs_cursor_hook_state_legacy_full_sweep_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP")
}

/// 是否**禁止**仅凭磁盘 `GOAL_STATE` hydration 将 `pre_goal_review_satisfied` 置真。
/// Canonical `ROUTER_RS_PRE_GOAL_STRICT_DISK`; legacy `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` still honored.
/// Canonical explicitly set (`0`/`false`/`off`/`no`) wins over legacy; unset falls through to legacy.
pub fn router_rs_cursor_pre_goal_strict_disk_enabled() -> bool {
    let canonical_key = "ROUTER_RS_PRE_GOAL_STRICT_DISK";
    let legacy_key = "ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK";
    if env::var(canonical_key).is_ok() {
        return router_rs_env_enabled_default_true(canonical_key);
    }
    router_rs_env_enabled_default_true(legacy_key)
}

/// 恢复已从默认 `hooks.json` 移除的 5 个事件的完整 handler dispatch。
pub fn router_rs_cursor_hook_legacy_subtracted_events_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS")
}

/// `ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN=1`（legacy）：hook-state 持久化失败时 beforeSubmit 仍 `continue: true`（应急）。
pub fn router_rs_cursor_hook_state_fail_open_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN")
}

/// Cross-host: missing `fork_context` on countable reviewer lane may infer independent fork.
/// Canonical `ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` explicit opt-in still wins;
/// legacy `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` **unset = on** (Cursor 历史默认).
/// Deprecated: prefer canonical `ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`.
pub fn router_rs_cursor_review_fork_context_missing_infer_false_enabled() -> bool {
    if router_rs_env_enabled_default_false("ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE") {
        return true;
    }
    router_rs_env_enabled_default_true("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE")
}

/// `ROUTER_RS_TASK_LEDGER_FLOCK`：是否对「任务账本」写入使用 flock sentinel。
pub fn router_rs_task_ledger_flock_enabled() -> bool {
    static FLOCK_WARN: std::sync::Once = std::sync::Once::new();
    let enabled = router_rs_env_enabled_default_true(ROUTER_RS_TASK_LEDGER_FLOCK_ENV);
    if !enabled {
        FLOCK_WARN.call_once(|| {
            eprintln!(
                "[router-rs] WARNING: ROUTER_RS_TASK_LEDGER_FLOCK is disabled;                  parallel writes to task ledger files may interleave"
            );
        });
    }
    enabled
}

/// `ROUTER_RS_HOOK_TIMING=1`: emit `hook_timing …` lines on stderr per hook invocation.
pub fn router_rs_hook_timing_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_HOOK_TIMING_ENV)
}

pub fn router_rs_cursor_cargo_check_sync_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC")
}

pub fn router_rs_cursor_hook_state_dir_sync_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC")
}

pub fn router_rs_cursor_hook_state_file_sync_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FILE_SYNC")
}

pub fn router_rs_cursor_hook_state_lock_retries() -> u32 {
    env::var("ROUTER_RS_CURSOR_HOOK_STATE_LOCK_RETRIES")
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
        .unwrap_or(100)
}

pub fn router_rs_cursor_hook_state_stale_sweep_days() -> u64 {
    let env_key = "ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS";
    match env::var(env_key) {
        Err(_) => 7,
        Ok(raw) => {
            let t = raw.trim().to_ascii_lowercase();
            if matches!(t.as_str(), "0" | "false" | "off" | "no") {
                return 0;
            }
            match raw.trim().parse::<u64>() {
                Ok(n) => n,
                Err(_) => {
                    eprintln!("[router-rs] invalid {env_key}={raw:?}; using default 7");
                    7
                }
            }
        }
    }
}

pub fn router_rs_session_call_tracker_tool_keys_max() -> usize {
    parse_router_rs_usize_clamped(
        ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX_ENV,
        128,
        16,
        4096,
    )
}

pub fn router_rs_env_enabled_default_true(var_name: &str) -> bool {
    core_policy::env_flags::env_enabled_default_true(var_name)
}

pub fn router_rs_env_enabled_default_false(var_name: &str) -> bool {
    core_policy::env_flags::env_enabled_default_false(var_name)
}

/// Canonical `ROUTER_RS_HOOK_SILENT`; legacy `ROUTER_RS_CURSOR_HOOK_SILENT` still honored.
pub fn router_rs_cursor_hook_silent_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_HOOK_SILENT")
        || router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_SILENT")
}

pub fn router_rs_operator_inject_globally_enabled() -> bool {
    router_rs_env_enabled_default_true("ROUTER_RS_OPERATOR_INJECT")
}

pub fn router_rs_continuity_post_tool_evidence_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE_ENV)
}

/// Canonical `ROUTER_RS_HOOK_OUTBOUND_CONTEXT_MAX_CHARS`; legacy `ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS` still honored.
pub fn router_rs_cursor_hook_outbound_context_max_bytes() -> usize {
    let key_canonical = "ROUTER_RS_HOOK_OUTBOUND_CONTEXT_MAX_CHARS";
    let key_legacy = "ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS";
    let raw = env::var(key_canonical)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| env::var(key_legacy).ok().filter(|s| !s.trim().is_empty()));
    match raw {
        None => 8192,
        Some(raw) => {
            let trimmed = raw.trim();
            match trimmed.parse::<usize>() {
                Ok(n) => n.clamp(1024, 65536),
                Err(_) => {
                    eprintln!(
                        "[router-rs] invalid {key_canonical} (or legacy {key_legacy})={raw:?}; using default 8192 (clamp 1024..65536)"
                    );
                    8192
                }
            }
        }
    }
}

/// Legacy name; canonical env `ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES`.
///
/// router-rs 单测：两 env 均未设置时返回 `None`（严格、不降级）；core-policy 依赖构建无 `cfg(test)`，故在此保留测试语义。
/// Deprecated: `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES` → prefer `ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES`.
pub fn router_rs_cursor_review_gate_stop_max_nudges_cap() -> Option<u32> {
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
                    eprintln!(
                        "[router-rs] invalid {env_key}={raw:?}; using default {default_val} (clamp {min_allowed}..{max_allowed})"
                    );
                    default_val
                }
            }
        }
    }
}

/// `ROUTER_RS_RFV_MAX_ROUNDS_CAP`: RFV 循环最大轮次硬上限。
pub fn router_rs_rfv_max_rounds_cap() -> u64 {
    const MAX_CAP: u64 = 10000;
    const DEFAULT: u64 = 1000;
    env::var("ROUTER_RS_RFV_MAX_ROUNDS_CAP")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .map(|n| n.min(MAX_CAP))
        .unwrap_or(DEFAULT)
}

pub fn router_rs_skip_pre_tool_use_guard() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD_ENV)
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
        unsafe { env::remove_var(key) };
        assert!(router_rs_env_enabled_default_true(key));
    }

    #[test]
    fn zero_false_off_no_disable_default_true() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_UNITTEST_ENV_ENABLED_DEFAULT_TRUE_TOKENS";
        for v in ["0", "false", "off", "no", "FALSE", " Off "] {
            unsafe { env::set_var(key, v) };
            assert!(
                !router_rs_env_enabled_default_true(key),
                "expected disabled for {v:?}"
            );
        }
        unsafe { env::remove_var(key) };
    }

    #[test]
    fn other_values_enable_default_true() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_UNITTEST_ENV_ENABLED_DEFAULT_TRUE_OTHER";
        unsafe { env::set_var(key, "1") };
        assert!(router_rs_env_enabled_default_true(key));
        unsafe { env::set_var(key, "") };
        assert!(router_rs_env_enabled_default_true(key));
        unsafe { env::remove_var(key) };
    }

    #[test]
    fn pre_goal_enabled_opt_in_only() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_PRE_GOAL_ENABLED";
        let prev = env::var_os(key);
        unsafe { env::remove_var(key) };
        assert!(!super::router_rs_pre_goal_enabled());
        unsafe { env::set_var(key, "true") };
        assert!(super::router_rs_pre_goal_enabled());
        match prev {
            Some(v) => unsafe { env::set_var(key, v) },
            None => unsafe { env::remove_var(key) },
        }
    }

    #[test]
    fn pre_goal_strict_disk_default_true() {
        let _g = process_env_lock();
        let key_canonical = "ROUTER_RS_PRE_GOAL_STRICT_DISK";
        let key_legacy = "ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK";
        let prev_canon = env::var_os(key_canonical);
        let prev_legacy = env::var_os(key_legacy);
        unsafe { env::remove_var(key_canonical) };
        unsafe { env::remove_var(key_legacy) };
        assert!(super::router_rs_cursor_pre_goal_strict_disk_enabled());
        // Canonical explicitly set to "0" disables
        unsafe { env::set_var(key_canonical, "0") };
        assert!(!super::router_rs_cursor_pre_goal_strict_disk_enabled());
        unsafe { env::remove_var(key_canonical) };
        // Legacy fallback works when canonical unset
        unsafe { env::set_var(key_legacy, "0") };
        assert!(!super::router_rs_cursor_pre_goal_strict_disk_enabled());
        match prev_canon {
            Some(v) => unsafe { env::set_var(key_canonical, v) },
            None => unsafe { env::remove_var(key_canonical) },
        }
        match prev_legacy {
            Some(v) => unsafe { env::set_var(key_legacy, v) },
            None => unsafe { env::remove_var(key_legacy) },
        }
    }

    #[test]
    fn continuity_posttool_defaults_off_until_explicit_enable() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE";
        let prev = env::var_os(key);
        unsafe { env::remove_var(key) };
        assert!(!super::router_rs_continuity_post_tool_evidence_enabled());
        unsafe { env::set_var(key, "1") };
        assert!(super::router_rs_continuity_post_tool_evidence_enabled());
        match prev {
            Some(v) => unsafe { env::set_var(key, v) },
            None => unsafe { env::remove_var(key) },
        }
    }

    #[test]
    fn review_gate_stop_max_nudges_unset_in_tests_means_strict_none() {
        let _g = process_env_lock();
        let key = "ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES";
        let prev = env::var_os(key);
        unsafe { env::remove_var(key) };
        assert!(super::router_rs_cursor_review_gate_stop_max_nudges_cap().is_none());
        unsafe { env::set_var(key, "3") };
        assert_eq!(
            super::router_rs_cursor_review_gate_stop_max_nudges_cap(),
            Some(3)
        );
        unsafe { env::set_var(key, "0") };
        assert!(super::router_rs_cursor_review_gate_stop_max_nudges_cap().is_none());
        unsafe { env::remove_var(key) };
        // Also verify legacy CURSOR_ name is still honored by core-policy
        unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES", "5") };
        assert_eq!(
            super::router_rs_cursor_review_gate_stop_max_nudges_cap(),
            Some(5)
        );
        unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES") };
        match prev {
            Some(v) => unsafe { env::set_var(key, v) },
            None => unsafe { env::remove_var(key) },
        }
    }

    #[test]
    fn rfv_max_rounds_cap_defaults_and_clamped() {
        let _g = process_env_lock();
        let prev = env::var_os("ROUTER_RS_RFV_MAX_ROUNDS_CAP");
        unsafe { env::remove_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP") };
        assert_eq!(super::router_rs_rfv_max_rounds_cap(), 1000);
        unsafe { env::set_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP", "500") };
        assert_eq!(super::router_rs_rfv_max_rounds_cap(), 500);
        unsafe { env::set_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP", "20000") };
        assert_eq!(super::router_rs_rfv_max_rounds_cap(), 10000);
        match prev {
            Some(v) => unsafe { env::set_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP", v) },
            None => unsafe { env::remove_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP") },
        }
    }
}
