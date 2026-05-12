//! `ROUTER_RS_*` 连续性/续跑类开关：保留真正改变行为边界的少量闸门。
//!
//! 当前仍由环境变量驱动的开关：
//! - `ROUTER_RS_OPERATOR_INJECT`
//! - `ROUTER_RS_HARNESS_OPERATOR_NUDGES`
//! - `ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT`
//! - `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK`
//! - `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED`
//! - `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP` → [`router_rs_cursor_hook_state_legacy_full_sweep_enabled`]
//! - `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` → [`router_rs_cursor_pre_goal_strict_disk_enabled`]
//! - `ROUTER_RS_TASK_LEDGER_FLOCK` → [`router_rs_task_ledger_flock_enabled`]（跨进程账本 flock，默认启用）
//! - `ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS` → [`router_rs_cursor_hook_outbound_context_max_bytes`]
//! - `ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS` → [`router_rs_cursor_sessionstart_context_max_bytes`]
//! - `ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE`：Stop 软收尾提示（`SESSION_CLOSE_STYLE`）；`0`/`false`/`off`/`no` 关闭
//! - `ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY` → Codex：`UserPromptSubmit`/`PostToolUse`/`Stop` 无稳定会话键时 block（见 `codex_hooks.rs`）
//!
//! 已退役的文案/投影分叉开关在代码层固定为关闭，不再暴露环境变量入口。

use std::env;

const ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT_ENV: &str = "ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT";
const ROUTER_RS_DEPTH_SCORE_MODE_ENV: &str = "ROUTER_RS_DEPTH_SCORE_MODE";
const ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED_ENV: &str =
    "ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED";
const ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP_ENV: &str =
    "ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP";
const ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK_ENV: &str = "ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK";
const ROUTER_RS_TASK_LEDGER_FLOCK_ENV: &str = "ROUTER_RS_TASK_LEDGER_FLOCK";

/// `/autopilot` **pre-goal** 仍保持显式 opt-in。
pub fn router_rs_cursor_autopilot_pre_goal_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED_ENV)
}

/// Cursor `SessionEnd`：是否对 `.cursor/hook-state/` 做**历史全目录前缀清扫**（与今日旧行为一致）。
///
/// 默认 **关闭**（仅清当前 `session_key` 对应状态 + 全局清 tmp 孤儿，避免同仓库多会话互删门控文件）。
/// 仅当 `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP=1|true|yes|on` 时开启。
pub fn router_rs_cursor_hook_state_legacy_full_sweep_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP_ENV)
}

/// Cursor：beforeSubmit 路径上是否**禁止**仅凭磁盘 `GOAL_STATE` hydration 将 `pre_goal_review_satisfied` 置真。
///
/// 默认 **关闭**（与历史一致：盘上已有 GOAL 可跳过 pre-goal nag）。**仅**当
/// `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK=1|true|yes|on` 时开启，用于降低 checkout/遗留
/// `artifacts/current` 带入的旧 GOAL 误放行 pre-goal 的风险；Stop 路径（`arm_if_goal_file`）不受影响。
pub fn router_rs_cursor_pre_goal_strict_disk_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK_ENV)
}

/// `ROUTER_RS_TASK_LEDGER_FLOCK`：是否对「任务账本」写入使用 `artifacts/current` 旁路 sentinel 文件的 `flock`。
///
/// 默认 **启用**（unset 或非 `0`/`false`/`off`/`no`）；网络盘若不靠谱可显式设为关闭（并行写入风险自担）。
pub fn router_rs_task_ledger_flock_enabled() -> bool {
    router_rs_env_enabled_default_true(ROUTER_RS_TASK_LEDGER_FLOCK_ENV)
}

/// 与历史实现一致：空字符串经 trim 后不属于关闭词，仍视为启用。
pub fn router_rs_env_enabled_default_true(var_name: &str) -> bool {
    match env::var(var_name) {
        Ok(value) => {
            let token = value.trim().to_ascii_lowercase();
            !(token == "0" || token == "false" || token == "off" || token == "no")
        }
        Err(_) => true,
    }
}

/// 未设置视为关闭；仅 `1`/`true`/`yes`/`on` 时开启。
pub fn router_rs_env_enabled_default_false(var_name: &str) -> bool {
    match env::var(var_name) {
        Ok(value) => {
            let token = value.trim().to_ascii_lowercase();
            matches!(token.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

/// `ROUTER_RS_OPERATOR_INJECT`：聚合关断 advisory 注入；硬门控短码不受此开关影响。
pub fn router_rs_operator_inject_globally_enabled() -> bool {
    router_rs_env_enabled_default_true("ROUTER_RS_OPERATOR_INJECT")
}

/// `ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT`：仅影响 RFV advisory struct hint。
pub fn router_rs_rfv_external_struct_hint_enabled() -> bool {
    router_rs_operator_inject_globally_enabled()
        && router_rs_env_enabled_default_true(ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT_ENV)
}

/// `ROUTER_RS_DEPTH_SCORE_MODE=strict` 时启用 strict 第三分公式。
pub fn router_rs_depth_score_mode_strict() -> bool {
    match env::var(ROUTER_RS_DEPTH_SCORE_MODE_ENV) {
        Ok(value) => value.trim().eq_ignore_ascii_case("strict"),
        Err(_) => false,
    }
}

/// Cursor hook：出站 JSON 中 `additional_context` 总站 **UTF-8 字节** 上限。
///
/// 默认 **8192**；`ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS` 解析为十进制 usize，夹在 \[1024, 65536]。
pub fn router_rs_cursor_hook_outbound_context_max_bytes() -> usize {
    parse_router_rs_usize_clamped(
        "ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS",
        8192,
        1024,
        65536,
    )
}

/// Cursor `SessionStart`：`additional_context` 合成后的 **UTF-8 字节** 上限。
///
/// 默认 **1200**；夹在 \[256, 8192]。
pub fn router_rs_cursor_sessionstart_context_max_bytes() -> usize {
    parse_router_rs_usize_clamped(
        "ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS",
        1200,
        256,
        8192,
    )
}

fn parse_router_rs_usize_clamped(
    env_key: &'static str,
    default_val: usize,
    min_allowed: usize,
    max_allowed: usize,
) -> usize {
    env::var(env_key)
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .map(|n| n.clamp(min_allowed, max_allowed))
        .unwrap_or(default_val)
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
    fn pre_goal_strict_disk_opt_in_only() {
        let _g = lock_env();
        let prev = env::var_os("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
        env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
        assert!(!super::router_rs_cursor_pre_goal_strict_disk_enabled());
        env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", "true");
        assert!(super::router_rs_cursor_pre_goal_strict_disk_enabled());
        match prev {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", v),
            None => env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK"),
        }
    }

    #[test]
    fn depth_score_mode_strict_only_on_exact_token() {
        let _g = lock_env();
        let key = "ROUTER_RS_DEPTH_SCORE_MODE";
        let prev = env::var(key).ok();
        env::remove_var(key);
        assert!(!super::router_rs_depth_score_mode_strict());
        env::set_var(key, "strict");
        assert!(super::router_rs_depth_score_mode_strict());
        env::set_var(key, " STRICT ");
        assert!(super::router_rs_depth_score_mode_strict());
        env::set_var(key, "legacy");
        assert!(!super::router_rs_depth_score_mode_strict());
        match prev {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }
}
