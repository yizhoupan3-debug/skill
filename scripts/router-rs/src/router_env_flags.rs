//! `ROUTER_RS_*` 连续性/续跑类开关：保留真正改变行为边界的少量闸门。
//!
//! 当前仍由环境变量驱动的开关：
//! - `ROUTER_RS_OPERATOR_INJECT`
//! - `ROUTER_RS_HARNESS_OPERATOR_NUDGES`
//! - `ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT`
//! - `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK`
//! - `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED`
//! - `ROUTER_RS_DEPTH_SCORE_MODE`
//!
//! 已退役的文案/投影分叉开关在代码层固定为关闭，不再暴露环境变量入口。

use std::env;

const ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT_ENV: &str = "ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT";
const ROUTER_RS_DEPTH_SCORE_MODE_ENV: &str = "ROUTER_RS_DEPTH_SCORE_MODE";
const ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED_ENV: &str =
    "ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED";

/// `/autopilot` **pre-goal** 仍保持显式 opt-in。
pub fn router_rs_cursor_autopilot_pre_goal_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED_ENV)
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
