//! `ROUTER_RS_*` 连续性/续跑类开关：保留真正改变行为边界的少量闸门。
//!
//! **清单真源**：宿主可见语义与默认值以仓库根 [`docs/harness_architecture/03-hook-and-switches.md`](../../docs/harness_architecture/03-hook-and-switches.md) **开关面**表格为准。
//! Review-gate 相关 reader 真源在 [`core-policy/env_flags.rs`](../../core-policy/src/env_flags.rs)；本模块为 router-rs 薄包装与 Cursor/连续性专用 env。
//!
//! Helper 映射（core-policy）：
//! - `ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE` → [`router_rs_review_spawn_first_nudge_enabled`]
//! - `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE` → [`router_rs_cursor_subagent_model_inherit_nudge_enabled`]
//! - `ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`（+ legacy 宿主别名）→ [`router_rs_review_fork_context_missing_infer_false_enabled`]
//! - `ROUTER_RS_REVIEW_GATE_DISABLE`（+ legacy `ROUTER_RS_*_REVIEW_GATE_DISABLE`）→ [`router_rs_review_gate_disabled_for_host`]
//! - `ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES`（legacy `ROUTER_RS_CURSOR_*`）→ [`router_rs_review_gate_stop_max_nudges_cap`]
//! - `ROUTER_RS_REVIEW_PENDING_CYCLE_MAX`（legacy `ROUTER_RS_CURSOR_*`）→ [`router_rs_review_pending_cycle_max`]
//!
//! 本模块直接读取：
//! - `ROUTER_RS_OPERATOR_INJECT`、`ROUTER_RS_CURSOR_*` hook-state / pre-goal / outbound 字节上限等
//! - `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE`、`ROUTER_RS_TASK_LEDGER_FLOCK`、`ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD` 等
//!
//! 已退役的文案/投影分叉开关在代码层固定为关闭，不再暴露环境变量入口。

use std::env;

const ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE_ENV: &str = "ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE";
const ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED_ENV: &str =
    "ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED";
const ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP_ENV: &str =
    "ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP";
const ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK_ENV: &str = "ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK";
const ROUTER_RS_TASK_LEDGER_FLOCK_ENV: &str = "ROUTER_RS_TASK_LEDGER_FLOCK";
const ROUTER_RS_HOOK_TIMING_ENV: &str = "ROUTER_RS_HOOK_TIMING";
const ROUTER_RS_CURSOR_CARGO_CHECK_SYNC_ENV: &str = "ROUTER_RS_CURSOR_CARGO_CHECK_SYNC";
const ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC_ENV: &str = "ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC";
const ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS_ENV: &str =
    "ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS";
const ROUTER_RS_CURSOR_HOOK_SILENT_ENV: &str = "ROUTER_RS_CURSOR_HOOK_SILENT";
const ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX_ENV: &str =
    "ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX";
const ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD_ENV: &str = "ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD";
const ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE_ENV: &str =
    "ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE";
const ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS_ENV: &str =
    "ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS";
const ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN_ENV: &str = "ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN";

pub use core_policy::env_flags::{
    router_rs_cursor_subagent_model_inherit_nudge_enabled,
    router_rs_review_fork_context_missing_infer_false_enabled,
    router_rs_review_gate_disabled_for_host, router_rs_review_pending_cycle_max,
    router_rs_review_spawn_first_nudge_enabled,
};

/// My implement **pre-goal** nudge（legacy env 名 `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED`）仍保持显式 opt-in。
pub fn router_rs_cursor_autopilot_pre_goal_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED_ENV)
}

/// Cursor `SessionEnd`：是否对 `.cursor/hook-state/` 做**历史全目录前缀清扫**。
pub fn router_rs_cursor_hook_state_legacy_full_sweep_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP_ENV)
}

/// Cursor：是否**禁止**仅凭磁盘 `GOAL_STATE` hydration 将 `pre_goal_review_satisfied` 置真。
pub fn router_rs_cursor_pre_goal_strict_disk_enabled() -> bool {
    router_rs_env_enabled_default_true(ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK_ENV)
}

/// 恢复已从默认 `hooks.json` 移除的 5 个事件的完整 handler dispatch。
pub fn router_rs_cursor_hook_legacy_subtracted_events_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS_ENV)
}

/// `ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN=1`：hook-state 持久化失败时 beforeSubmit 仍 `continue: true`（应急）。
pub fn router_rs_cursor_hook_state_fail_open_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN_ENV)
}

/// Cursor: missing `fork_context` on countable reviewer lane may infer independent fork.
/// Canonical `ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` explicit opt-in still wins;
/// legacy `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` **unset = on** (Cursor 历史默认)。
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
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_CARGO_CHECK_SYNC_ENV)
}

pub fn router_rs_cursor_hook_state_dir_sync_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC_ENV)
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
    match env::var(ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS_ENV) {
        Err(_) => 7,
        Ok(raw) => {
            let t = raw.trim().to_ascii_lowercase();
            if matches!(t.as_str(), "0" | "false" | "off" | "no") {
                return 0;
            }
            match raw.trim().parse::<u64>() {
                Ok(n) => n,
                Err(_) => {
                    eprintln!(
                        "[router-rs] invalid {ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS_ENV}={raw:?}; using default 7"
                    );
                    7
                }
            }
        }
    }
}

/// Legacy name; canonical env `ROUTER_RS_REVIEW_PENDING_CYCLE_MAX`.
pub fn router_rs_cursor_review_pending_cycle_max() -> usize {
    router_rs_review_pending_cycle_max()
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

pub fn router_rs_cursor_hook_silent_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_HOOK_SILENT_ENV)
}

pub fn router_rs_operator_inject_globally_enabled() -> bool {
    router_rs_env_enabled_default_true("ROUTER_RS_OPERATOR_INJECT")
}

pub fn router_rs_continuity_post_tool_evidence_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE_ENV)
}

pub fn router_rs_cursor_hook_outbound_context_max_bytes() -> usize {
    parse_router_rs_usize_clamped(
        "ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS",
        8192,
        1024,
        65536,
    )
}

pub fn router_rs_cursor_sessionstart_context_max_bytes() -> usize {
    parse_router_rs_usize_clamped(
        "ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS",
        1200,
        256,
        8192,
    )
}

/// Legacy name; canonical env `ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES`.
///
/// router-rs 单测：两 env 均未设置时返回 `None`（严格、不降级）；core-policy 依赖构建无 `cfg(test)`，故在此保留测试语义。
pub fn router_rs_cursor_review_gate_stop_max_nudges_cap() -> Option<u32> {
    #[cfg(test)]
    {
        let raw = env::var("ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES")
            .ok()
            .or_else(|| env::var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES").ok());
        if raw.is_none() {
            return None;
        }
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
    use std::sync::{Mutex, OnceLock};

    static ENV_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env test lock")
    }

    #[test]
    fn unset_means_enabled_for_default_true() {
        let _g = lock_env();
        let key = "ROUTER_RS_UNITTEST_ENV_ENABLED_DEFAULT_TRUE_UNSET";
        env::remove_var(key);
        assert!(router_rs_env_enabled_default_true(key));
    }

    #[test]
    fn zero_false_off_no_disable_default_true() {
        let _g = lock_env();
        let key = "ROUTER_RS_UNITTEST_ENV_ENABLED_DEFAULT_TRUE_TOKENS";
        for v in ["0", "false", "off", "no", "FALSE", " Off "] {
            env::set_var(key, v);
            assert!(
                !router_rs_env_enabled_default_true(key),
                "expected disabled for {v:?}"
            );
        }
        env::remove_var(key);
    }

    #[test]
    fn other_values_enable_default_true() {
        let _g = lock_env();
        let key = "ROUTER_RS_UNITTEST_ENV_ENABLED_DEFAULT_TRUE_OTHER";
        env::set_var(key, "1");
        assert!(router_rs_env_enabled_default_true(key));
        env::set_var(key, "");
        assert!(router_rs_env_enabled_default_true(key));
        env::remove_var(key);
    }

    #[test]
    fn autopilot_pre_goal_enabled_opt_in_only() {
        let _g = lock_env();
        let prev = env::var_os("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED");
        env::remove_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED");
        assert!(!super::router_rs_cursor_autopilot_pre_goal_enabled());
        env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", "true");
        assert!(super::router_rs_cursor_autopilot_pre_goal_enabled());
        match prev {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", v),
            None => env::remove_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED"),
        }
    }

    #[test]
    fn pre_goal_strict_disk_default_true() {
        let _g = lock_env();
        let prev = env::var_os("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
        env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
        assert!(super::router_rs_cursor_pre_goal_strict_disk_enabled());
        env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", "0");
        assert!(!super::router_rs_cursor_pre_goal_strict_disk_enabled());
        match prev {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", v),
            None => env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK"),
        }
    }

    #[test]
    fn continuity_posttool_defaults_off_until_explicit_enable() {
        let _g = lock_env();
        let key = "ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE";
        let prev = env::var_os(key);
        env::remove_var(key);
        assert!(!super::router_rs_continuity_post_tool_evidence_enabled());
        env::set_var(key, "1");
        assert!(super::router_rs_continuity_post_tool_evidence_enabled());
        match prev {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }

    #[test]
    fn review_gate_stop_max_nudges_unset_in_tests_means_strict_none() {
        let _g = lock_env();
        for key in [
            "ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES",
            "ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES",
        ] {
            env::remove_var(key);
        }
        assert!(super::router_rs_cursor_review_gate_stop_max_nudges_cap().is_none());
        env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES", "3");
        assert_eq!(
            super::router_rs_cursor_review_gate_stop_max_nudges_cap(),
            Some(3)
        );
        env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES", "0");
        assert!(super::router_rs_cursor_review_gate_stop_max_nudges_cap().is_none());
        for key in [
            "ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES",
            "ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES",
        ] {
            env::remove_var(key);
        }
    }

    #[test]
    fn rfv_max_rounds_cap_defaults_and_clamped() {
        let _g = lock_env();
        let prev = env::var_os("ROUTER_RS_RFV_MAX_ROUNDS_CAP");
        env::remove_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP");
        assert_eq!(super::router_rs_rfv_max_rounds_cap(), 1000);
        env::set_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP", "500");
        assert_eq!(super::router_rs_rfv_max_rounds_cap(), 500);
        env::set_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP", "20000");
        assert_eq!(super::router_rs_rfv_max_rounds_cap(), 10000);
        match prev {
            Some(v) => env::set_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP", v),
            None => env::remove_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP"),
        }
    }
}
