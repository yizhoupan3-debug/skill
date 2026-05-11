//! `ROUTER_RS_*` 连续性/续跑类开关：未设置视为启用；仅当值为 `0`/`false`/`off`/`no`（trim + ASCII 小写）时关闭。
//!
//! **`ROUTER_RS_HARNESS_OPERATOR_NUDGES`**：关闭 RFV / Autopilot 中由 `HARNESS_OPERATOR_NUDGES.json` 注入的 operator 文案（默认开启）。实现见 `harness_operator_nudges`。
//! **`ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT`**：当 RFV active 且 `prefer_structured_external_research` 等条件满足时，在续跑文末追加单行「并入 external_research」提示；设为 `0`/`false`/`off`/`no` 关闭（默认启用）。
//!
//! **`ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT`** / **`ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT`**：仅控制 Cursor **`beforeSubmit`** 是否合并 **AUTOPILOT_DRIVE** / **RFV_LOOP_CONTINUE** 续跑块；**默认关闭**（`1`/`true`/`yes`/`on` 显式开启）。**Stop** 等路径仍由 `ROUTER_RS_AUTOPILOT_DRIVE_HOOK` / `ROUTER_RS_RFV_LOOP_HOOK` 等既有开关决定。
//!
//! **`ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED`**：是否启用 `/autopilot` **pre-goal** 注入（beforeSubmit 侧）；**默认关闭**（显式 opt-in）。不影响磁盘 `GOAL_STATE` 收口门控；严格宏任务工作流可手动打开。
//!
//! **`ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE`**：Cursor **CreatePlan** 落盘的 **`.cursor/plans/*.plan.md`** 路径出现在 **`beforeSubmit` stdin JSON 全树或用户提示**中时（典型：官方 Plan 点 **Build** 带入计划引用），**视同**已走 **`/autopilot` 入口**以拉起 **goal 门控**（`goal_required` 等，与 `is_autopilot_entrypoint_prompt` 对齐）。**默认关闭**；`1`/`true`/`yes`/`on` 开启。不会替你执行 shell 或 `router-rs framework_autopilot_goal`；Goal 契约仍须按 `skills/autopilot/SKILL.md` 在宿主内发布。
//!
//! **`ROUTER_RS_DEPTH_SCORE_MODE`**：未设置或 `legacy`（trim，ASCII 大小写不敏感）→ 使用既有 `depth_score` 三档公式；设为 **`strict`** 时第三分在「checkpoint / 对抗轮」之外，还把 **非零 falsification_tests 计数** 与（任务 `external_research_strict` 为真时的）**strict 外研通过轮次**计入第三分条件。见 `docs/harness_architecture.md` §8。
//!
//! **`ROUTER_RS_REVIEW_GATE_SUPPRESS_ON_MANUSCRIPT_CONTEXT`**：opt-in（默认关）。为 `1`/`true`/`yes`/`on` 时，`hook_common::is_review_prompt` 在已命中 `REVIEW_ROUTING_SIGNALS` 正则且 **`has_paper_context`** 为真、且**无**「强代码/PR 锚点」时返回 **false**，以降低手稿话术误触 Cursor **`REVIEW_GATE`**；与 **`ROUTER_RS_OPERATOR_INJECT`** 无包含关系。

use std::env;

const ROUTER_RS_REVIEW_GATE_SUPPRESS_ON_MANUSCRIPT_CONTEXT_ENV: &str =
    "ROUTER_RS_REVIEW_GATE_SUPPRESS_ON_MANUSCRIPT_CONTEXT";

const ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT_ENV: &str = "ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT";
const ROUTER_RS_DEPTH_SCORE_MODE_ENV: &str = "ROUTER_RS_DEPTH_SCORE_MODE";

const CURSOR_HOOK_CHAT_FOLLOWUP_ENV: &str = "ROUTER_RS_CURSOR_HOOK_CHAT_FOLLOWUP";

const ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT_ENV: &str = "ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT";
const ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT_ENV: &str = "ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT";
const ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED_ENV: &str =
    "ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED";

const ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE_ENV: &str =
    "ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE";

/// Cursor **`beforeSubmit`**：合并 **AUTOPILOT_DRIVE** 续跑块；未设置或关闭 token → **不合并**（默认安静）。
pub fn router_rs_autopilot_drive_before_submit_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT_ENV)
}

/// Cursor **`beforeSubmit`**：合并 **RFV_LOOP_CONTINUE** 续跑块；未设置或关闭 token → **不合并**（默认安静）。
pub fn router_rs_rfv_loop_before_submit_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT_ENV)
}

/// `/autopilot` **pre-goal**（beforeSubmit 长段提示与计数放行）；未设置 → **关闭**；仅 `1`/`true`/`yes`/`on` 开启。
pub fn router_rs_cursor_autopilot_pre_goal_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED_ENV)
}

/// Cursor Plan **Build** 首条：出现官方 **`.cursor/plans/*.plan.md`** 引用时视同 **`/autopilot`** 拉起 goal 门控；未设置 → **关闭**。
pub fn router_rs_cursor_plan_build_autopilot_goal_gate_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE_ENV)
}

/// **`REVIEW_GATE`** 启发式：手稿语境下跳过「review 提示」判定（**默认关闭**；仅 `1`/`true`/`yes`/`on` 开启）。实现见 `hook_common::is_review_prompt`。
pub fn router_rs_review_gate_suppress_on_manuscript_context_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_REVIEW_GATE_SUPPRESS_ON_MANUSCRIPT_CONTEXT_ENV)
}

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
/// - `AUTOPILOT_DRIVE_HOOK`（Cursor **Stop** 等的 GOAL 续跑块；**beforeSubmit** 另见 `ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT`）
/// - `RFV_LOOP_HOOK`（Cursor **Stop** 等的 RFV 续跑块；**beforeSubmit** 另见 `ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT`）
/// - **`ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK` 已启用时**：Cursor **`beforeSubmit`** 的 **`PAPER_ADVERSARIAL_HOOK`** 短文（论文强对抗审稿提示）
///
/// 细粒度变量仍可单独关掉某一类；本变量是「一键关全部续跑/nudge」的总闸。**默认开启**。
pub fn router_rs_operator_inject_globally_enabled() -> bool {
    router_rs_env_enabled_default_true("ROUTER_RS_OPERATOR_INJECT")
}

/// P1：`ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT`。默认与其它 `ROUTER_RS_*` 软关语义一致：**未设置 = 启用**；
/// **`ROUTER_RS_OPERATOR_INJECT`** 总闸仍为 off 时，`build_rfv_loop_followup_message_from_state` 整体不产出续跑，
/// 本条不会单独生效。
pub fn router_rs_rfv_external_struct_hint_enabled() -> bool {
    router_rs_operator_inject_globally_enabled()
        && router_rs_env_enabled_default_true(ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT_ENV)
}

/// `ROUTER_RS_DEPTH_SCORE_MODE=strict`（trim，ASCII 大小写不敏感）时启用 `DepthCompliance.depth_score` 的 **strict** 第三分公式；未设置或其它取值 → **legacy**（与历史行为一致）。
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

    #[test]
    fn before_submit_continuity_envs_default_off() {
        let _g = lock_env();
        let prev_ap = env::var_os("ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT");
        let prev_rfv = env::var_os("ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT");
        env::remove_var("ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT");
        env::remove_var("ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT");
        assert!(!super::router_rs_autopilot_drive_before_submit_enabled());
        assert!(!super::router_rs_rfv_loop_before_submit_enabled());
        env::set_var("ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT", "1");
        env::set_var("ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT", "on");
        assert!(super::router_rs_autopilot_drive_before_submit_enabled());
        assert!(super::router_rs_rfv_loop_before_submit_enabled());
        match prev_ap {
            Some(v) => env::set_var("ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT", v),
            None => env::remove_var("ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT"),
        }
        match prev_rfv {
            Some(v) => env::set_var("ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT", v),
            None => env::remove_var("ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT"),
        }
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
    fn plan_build_autopilot_goal_gate_opt_in_only() {
        let _g = lock_env();
        let prev = env::var_os("ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE");
        env::remove_var("ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE");
        assert!(!super::router_rs_cursor_plan_build_autopilot_goal_gate_enabled());
        env::set_var("ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE", "on");
        assert!(super::router_rs_cursor_plan_build_autopilot_goal_gate_enabled());
        match prev {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE", v),
            None => env::remove_var("ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE"),
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
