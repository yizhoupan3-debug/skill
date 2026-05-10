//! `ROUTER_RS_*` 连续性/续跑类开关：未设置视为启用；仅当值为 `0`/`false`/`off`/`no`（trim + ASCII 小写）时关闭。
//!
//! **`ROUTER_RS_HARNESS_OPERATOR_NUDGES`**：关闭 RFV / Autopilot 中由 `HARNESS_OPERATOR_NUDGES.json` 注入的 operator 文案（默认开启）。实现见 `harness_operator_nudges`。

use std::env;

const CURSOR_HOOK_CHAT_FOLLOWUP_ENV: &str = "ROUTER_RS_CURSOR_HOOK_CHAT_FOLLOWUP";

/// Cursor：`followup_message` 常以跟贴形式出现在主对话区。**默认**（未设置或 `0`/`false`/`off`/`no`）将续跑类提示写入 **`additional_context`**，减少对用户可见对话流的干扰。
/// `ROUTER_RS_CURSOR_HOOK_CHAT_FOLLOWUP=1`/`true`/`yes`/`on` 时改回写入 `followup_message`。
pub fn router_rs_cursor_hook_chat_followup_enabled() -> bool {
    match env::var(CURSOR_HOOK_CHAT_FOLLOWUP_ENV) {
        Ok(value) => {
            let token = value.trim().to_ascii_lowercase();
            matches!(token.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

/// `ROUTER_RS_GOAL_PROMPT_VERBOSE`：为 `1`/`true`/`yes`/`on`（trim + ASCII 小写）时，continuity digest 的 Goal 段落、**AUTOPILOT_DRIVE**、**RFV_LOOP_CONTINUE**、以及 `/autopilot` **pre-goal** 提示使用冗长版；**默认紧凑**。
pub fn router_rs_goal_prompt_verbose() -> bool {
    match env::var("ROUTER_RS_GOAL_PROMPT_VERBOSE") {
        Ok(value) => {
            let token = value.trim().to_ascii_lowercase();
            matches!(token.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

/// 与 `autopilot_drive_hook_enabled` 等历史实现一致：空字符串经 trim 后仍为 `""`，不属于关闭词 → 仍为启用。
pub fn router_rs_env_enabled_default_true(var_name: &str) -> bool {
    match env::var(var_name) {
        Ok(value) => {
            let token = value.trim().to_ascii_lowercase();
            !(token == "0" || token == "false" || token == "off" || token == "no")
        }
        Err(_) => true,
    }
}

/// 未设置视为**关闭**；仅 `1`/`true`/`yes`/`on`（trim + ASCII 小写）时开启。（用于 opt-in 类钩子，如论文强对抗审稿 beforeSubmit。）
pub fn router_rs_env_enabled_default_false(var_name: &str) -> bool {
    match env::var(var_name) {
        Ok(value) => {
            let token = value.trim().to_ascii_lowercase();
            matches!(token.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

/// `ROUTER_RS_OPERATOR_INJECT`：跨切片**聚合关断**（P1-E）。
///
/// 当此变量为 `0`/`false`/`off`/`no` 时，下列四类「面向模型的 operator 注入」**全部**视为关闭：
/// - `HARNESS_OPERATOR_NUDGES`（推理深度等 nudge 句）
/// - `AUTOPILOT_DRIVE_HOOK`（Cursor Stop/beforeSubmit 的 GOAL 续跑块）
/// - `RFV_LOOP_HOOK`（Cursor Stop/beforeSubmit 的 RFV 续跑块）
/// - **`ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK` 已启用时**：Cursor **`beforeSubmit`** 的 **`PAPER_ADVERSARIAL_HOOK`** 短文（论文强对抗审稿提示）
///
/// 细粒度变量仍可单独关掉某一类；本变量是「一键关全部续跑/nudge」的总闸。**默认开启**。
pub fn router_rs_operator_inject_globally_enabled() -> bool {
    router_rs_env_enabled_default_true("ROUTER_RS_OPERATOR_INJECT")
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
    fn unset_means_enabled() {
        let _g = lock_env();
        let key = "ROUTER_RS_UNITTEST_ENV_ENABLED_DEFAULT_TRUE_UNSET";
        env::remove_var(key);
        assert!(router_rs_env_enabled_default_true(key));
    }

    #[test]
    fn zero_false_off_no_disable() {
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
    fn other_values_enable() {
        let _g = lock_env();
        let key = "ROUTER_RS_UNITTEST_ENV_ENABLED_DEFAULT_TRUE_OTHER";
        env::set_var(key, "1");
        assert!(router_rs_env_enabled_default_true(key));
        env::set_var(key, "");
        assert!(router_rs_env_enabled_default_true(key));
        env::remove_var(key);
    }

    #[test]
    fn cursor_hook_chat_followup_only_explicit_opt_in() {
        let _g = lock_env();
        let key = super::CURSOR_HOOK_CHAT_FOLLOWUP_ENV;
        let prev = env::var(key).ok();
        env::remove_var(key);
        assert!(!super::router_rs_cursor_hook_chat_followup_enabled());
        for v in ["1", "true", "yes", "on", " TRUE "] {
            env::set_var(key, v);
            assert!(
                super::router_rs_cursor_hook_chat_followup_enabled(),
                "{v:?}"
            );
        }
        for v in ["0", "false", "", "maybe"] {
            env::set_var(key, v);
            assert!(
                !super::router_rs_cursor_hook_chat_followup_enabled(),
                "{v:?}"
            );
        }
        match prev {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }

    #[test]
    fn goal_prompt_verbose_only_explicit_tokens() {
        let _g = lock_env();
        let key = "ROUTER_RS_GOAL_PROMPT_VERBOSE";
        let prev = env::var(key).ok();
        env::remove_var(key);
        assert!(!super::router_rs_goal_prompt_verbose());
        for v in ["1", "true", "yes", "on", " TRUE ", "On"] {
            env::set_var(key, v);
            assert!(super::router_rs_goal_prompt_verbose(), "{v:?}");
        }
        for v in ["0", "false", "", "maybe"] {
            env::set_var(key, v);
            assert!(!super::router_rs_goal_prompt_verbose(), "{v:?}");
        }
        match prev {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }
}
