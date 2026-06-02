//! `ROUTER_RS_*` 连续性/续跑类开关：保留真正改变行为边界的少量闸门。
//!
//! **清单真源**：宿主可见语义与默认值以仓库根 [`docs/harness_architecture.md`](../../docs/harness_architecture.md) **§5 开关面**表格为准；下列为本模块 **提供读取 helper** 或在注释中高频交叉引用的子集。其余变量（连续性 PostTool、Codex checkpoint、Cursor review/subagent cap、CLI/host_integration/runtime_storage/maint 等）在对应源文件中直读 `std::env::var`，仍以 harness §5 表为准。
//!
//! Helper 映射：
//! - `ROUTER_RS_OPERATOR_INJECT`
//! - `ROUTER_RS_HARNESS_OPERATOR_NUDGES`（未在本文件展开；见 harness §5）
//! - `ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT`
//! - `ROUTER_RS_DEPTH_SCORE_MODE`
//! - `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED`
//! - `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP` → [`router_rs_cursor_hook_state_legacy_full_sweep_enabled`]
//! - `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` → [`router_rs_cursor_pre_goal_strict_disk_enabled`]
//! - `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` → [`router_rs_cursor_review_fork_context_missing_infer_false_enabled`]
//! - `ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` → [`router_rs_codex_review_fork_context_missing_infer_false_enabled`]
//! - `ROUTER_RS_TASK_LEDGER_FLOCK` → [`router_rs_task_ledger_flock_enabled`]（跨进程账本 flock，默认启用）
//! - `ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS` → [`router_rs_cursor_hook_outbound_context_max_bytes`]（出站 UTF-8 **字节**上限）
//! - `ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS` → [`router_rs_cursor_sessionstart_context_max_bytes`]
//! - `ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE`：Stop 软收尾提示（`SESSION_CLOSE_STYLE`）；`0`/`false`/`off`/`no` 关闭（见 `frag_01_continuity_intent.rs`）
//! - `ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY`（未在本文件展开 helper；见 `codex_hooks/mod.rs`）
//! - `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE`（未在本文件展开 helper；见 `codex_hooks/mod.rs`）
//!
//! **散落直读（仅索引）**：`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE`、`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT`、`ROUTER_RS_CLOSEOUT_ENFORCEMENT`、`ROUTER_RS_CURSOR_*`（review gate disable、**`ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES`** → [`router_rs_cursor_review_gate_stop_max_nudges_cap`]、pre-goal max nudges、open subagent cap/stale、session namespace、workspace root、terminal kill）、`ROUTER_RS_CODEX_*`（含 review gate disable、stable session key、Stop hook active bypass、SessionStart context max）、`ROUTER_RS_CLAUDE_*`、`ROUTER_RS_CLIPBOARD_PATH`、`ROUTER_RS_STORAGE_ROOT`、`ROUTER_RS_BIN`、`ROUTER_RS_GENERATOR_TIMEOUT_SECONDS`、`ROUTER_RS_SHARED_TARGET`、`ROUTER_RS_UPDATE_*` — 见 harness §5 与各模块 `std::env::var`。
//!
//! 已退役的文案/投影分叉开关在代码层固定为关闭，不再暴露环境变量入口。

use std::env;

const ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT_ENV: &str = "ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT";
const ROUTER_RS_DEPTH_SCORE_MODE_ENV: &str = "ROUTER_RS_DEPTH_SCORE_MODE";
const ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE_ENV: &str = "ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE";
const ROUTER_RS_DEPTH_COMPLIANCE_HINT_ENV: &str = "ROUTER_RS_DEPTH_COMPLIANCE_HINT";
const ROUTER_RS_TASK_STATE_AGGREGATE_AUTO_ENV: &str = "ROUTER_RS_TASK_STATE_AGGREGATE_AUTO";
const ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED_ENV: &str =
    "ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED";
const ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP_ENV: &str =
    "ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP";
const ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK_ENV: &str = "ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK";
const ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";
const ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";
const ROUTER_RS_TASK_LEDGER_FLOCK_ENV: &str = "ROUTER_RS_TASK_LEDGER_FLOCK";
const ROUTER_RS_HOOK_TIMING_ENV: &str = "ROUTER_RS_HOOK_TIMING";
const ROUTER_RS_CURSOR_CARGO_CHECK_SYNC_ENV: &str = "ROUTER_RS_CURSOR_CARGO_CHECK_SYNC";
const ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC_ENV: &str = "ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC";
const ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS_ENV: &str =
    "ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS";
const ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV: &str =
    "ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX";
const ROUTER_RS_CURSOR_HOOK_SILENT_ENV: &str = "ROUTER_RS_CURSOR_HOOK_SILENT";
const ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE_ENV: &str = "ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE";
const ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE_ENV: &str =
    "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
const ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX_ENV: &str =
    "ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX";

/// My implement **pre-goal** nudge（legacy env 名 `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED`）仍保持显式 opt-in。
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

/// Cursor：是否**禁止**仅凭磁盘 `GOAL_STATE` hydration 将 `pre_goal_review_satisfied` 置真。
///
/// 默认 **开启**（ADR-005：盘上仅有 GOAL 不足以满足 pre-goal）。**仅**当
/// `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK=0|false|off|no` 时恢复历史宽松语义。
pub fn router_rs_cursor_pre_goal_strict_disk_enabled() -> bool {
    router_rs_env_enabled_default_true(ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK_ENV)
}

const ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS_ENV: &str =
    "ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS";
const ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN_ENV: &str = "ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN";

/// 恢复已从默认 `hooks.json` 移除的 5 个事件的完整 handler dispatch（shell 账本、rustfmt、`afterAgentResponse` 等）。
pub fn router_rs_cursor_hook_legacy_subtracted_events_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS_ENV)
}

/// `ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN=1`：hook-state 持久化失败时 beforeSubmit 仍 `continue: true`（应急）。
pub fn router_rs_cursor_hook_state_fail_open_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN_ENV)
}

/// Spawn-first pairing reviewer one-line nudge in hook outbound context (all hosts). Default **on**; `0|false|off|no` disables nudge only (gate thresholds unchanged).
pub fn router_rs_review_spawn_first_nudge_enabled() -> bool {
    router_rs_env_enabled_default_true(ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE_ENV)
}

/// Cursor beforeSubmit: subagent/Task model inherit one-liner (independent of REVIEW_GATE / my-light). Default **on**; `0|false|off|no` disables.
pub fn router_rs_cursor_subagent_model_inherit_nudge_enabled() -> bool {
    router_rs_env_enabled_default_true(ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE_ENV)
}

const ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";

/// Claude：缺失 `fork_context` 时是否推断 independent fork（默认 **关闭**，不读取 Cursor env）。
pub fn router_rs_claude_review_fork_context_missing_infer_false_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV)
}

/// Cursor：当 subagent 事件**未**携带可解析的 `fork_context` 时，是否将可数深度 lane 视为 `fork_context=false`。
///
/// 默认 **开启**（`unset` 或非 `0`/`false`/`off`/`no`）。显式 `fork_context: true` 仍阻断独立上下文证据。
/// 关闭后恢复 harness §5.0「缺字段≠false」语义。
pub fn router_rs_cursor_review_fork_context_missing_infer_false_enabled() -> bool {
    router_rs_env_enabled_default_true(ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV)
}

/// Codex CLI：可数深度 lane 且 `fork_context` 缺失时是否推断为 independent fork（`false`）。
///
/// 默认 **开启**（与 Cursor 同语义）。显式 `fork_context: true` 仍阻断。关闭后缺字段不计 PostTool 深度证据。
pub fn router_rs_codex_review_fork_context_missing_infer_false_enabled() -> bool {
    router_rs_env_enabled_default_true(ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV)
}

/// `ROUTER_RS_TASK_LEDGER_FLOCK`：是否对「任务账本」写入使用 `artifacts/current` 旁路 sentinel 文件的 `flock`。
///
/// 默认 **启用**（unset 或非 `0`/`false`/`off`/`no`）；网络盘若不靠谱可显式设为关闭（并行写入风险自担）。
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

/// `ROUTER_RS_CURSOR_CARGO_CHECK_SYNC=1`: run blocking `cargo check` on Rust writes in postToolUse (up to 25s).
pub fn router_rs_cursor_cargo_check_sync_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_CARGO_CHECK_SYNC_ENV)
}

/// `ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC=1`: fsync hook-state parent directory after each save (slower).
pub fn router_rs_cursor_hook_state_dir_sync_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC_ENV)
}

/// `ROUTER_RS_CURSOR_HOOK_STATE_FILE_SYNC=1`: fsync hook-state file after each save (slower). Default false.
pub fn router_rs_cursor_hook_state_file_sync_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FILE_SYNC")
}

/// `ROUTER_RS_CURSOR_HOOK_STATE_LOCK_RETRIES`: how many times to retry hook-state flock (50ms interval). Default 100.
pub fn router_rs_cursor_hook_state_lock_retries() -> u32 {
    env::var("ROUTER_RS_CURSOR_HOOK_STATE_LOCK_RETRIES")
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
        .unwrap_or(100)
}


/// Age-based stale sweep for `.cursor/hook-state/` owned files (default **7** days).
///
/// `0` / `false` / `off` / `no` disables; `LEGACY_FULL_SWEEP` remains opt-in full wipe.
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

/// Max entries in `review_subagent_pending_cycle_keys` (default **32**).
pub fn router_rs_cursor_review_pending_cycle_max() -> usize {
    parse_router_rs_usize_clamped(ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV, 32, 1, 256)
}

/// Max distinct tool names in `SESSION_CALL_TRACKER.json` `per_tool` (default **128**).
pub fn router_rs_session_call_tracker_tool_keys_max() -> usize {
    parse_router_rs_usize_clamped(
        ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX_ENV,
        128,
        16,
        4096,
    )
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

/// `ROUTER_RS_CURSOR_HOOK_SILENT=1`：剥 advisory `additional_context`；保留 `router-rs ` 硬短码 followup。
pub fn router_rs_cursor_hook_silent_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CURSOR_HOOK_SILENT_ENV)
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

/// PostTool → `EVIDENCE_INDEX` append. **Default off** (2026-05 solo subtraction); `=1`/`true`/`yes`/`on` enables.
pub fn router_rs_continuity_post_tool_evidence_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE_ENV)
}

/// `深度信号:` line in continuity digest. Default off unless strict depth mode or `ROUTER_RS_DEPTH_COMPLIANCE_HINT=1`.
pub fn router_rs_depth_compliance_hint_enabled() -> bool {
    router_rs_depth_score_mode_strict()
        || router_rs_env_enabled_default_false(ROUTER_RS_DEPTH_COMPLIANCE_HINT_ENV)
}


/// Auto-refresh `TASK_STATE.json` after ledger mutations. Default off; CLI `task-state-aggregate-sync` always runs.
pub fn router_rs_task_state_aggregate_auto_enabled() -> bool {
    router_rs_env_enabled_default_false(ROUTER_RS_TASK_STATE_AGGREGATE_AUTO_ENV)
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

const ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV: &str =
    "ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES";

/// Cursor `Stop`：在 `REVIEW_GATE` 仍未满足时，**连续多少轮**仍输出完整 `need=`/`hint=` 行到 `followup_message`；超过后降级为短 `followup_message` + `additional_context` 承载完整行，并跳过与 `AUTOPILOT_DRIVE`/`RFV` 的 Stop 合并以免双叠。
///
/// - **未设置**（非 test）：默认 **8**。
/// - `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES=0` / `false` / `off` / `no`：**关闭**降频（每轮 Stop 仍输出完整硬行，严格）。
/// - 正整数：自定义「完整硬行」次数上限。
///
/// **单测**：未设置变量时返回 **`None`（严格、不降级）**，避免并行用例依赖默认 cap。
pub fn router_rs_cursor_review_gate_stop_max_nudges_cap() -> Option<u32> {
    #[cfg(test)]
    {
        let Ok(raw) = env::var(ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV) else {
            return None;
        };
        let t = raw.trim().to_ascii_lowercase();
        if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
            return None;
        }
        t.parse::<u32>().ok().filter(|v| *v >= 1)
    }
    #[cfg(not(test))]
    {
        match env::var(ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV) {
            Err(_) => Some(8),
            Ok(raw) => {
                let t = raw.trim().to_ascii_lowercase();
                if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
                    return None;
                }
                if let Some(n) = t.parse::<u32>().ok().filter(|v| *v >= 1) {
                    return Some(n);
                }
                eprintln!(
                    "[router-rs] invalid {ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV}={raw:?}; using default cap 8"
                );
                Some(8)
            }
        }
    }
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
    fn continuity_subtraction_defaults_off_until_explicit_enable() {
        let _g = lock_env();
        let keys = [
            "ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE",
            "ROUTER_RS_DEPTH_COMPLIANCE_HINT",
            "ROUTER_RS_TASK_STATE_AGGREGATE_AUTO",
        ];
        let prev: Vec<_> = keys
            .iter()
            .map(|k| (*k, env::var_os(k)))
            .collect();
        for key in keys {
            env::remove_var(key);
        }
        assert!(!super::router_rs_continuity_post_tool_evidence_enabled());
        assert!(!super::router_rs_depth_compliance_hint_enabled());
        assert!(!super::router_rs_task_state_aggregate_auto_enabled());
        env::set_var("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE", "1");
        assert!(super::router_rs_continuity_post_tool_evidence_enabled());
        for (key, val) in prev {
            match val {
                Some(v) => env::set_var(key, v),
                None => env::remove_var(key),
            }
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

    #[test]
    fn review_gate_stop_max_nudges_unset_in_tests_means_strict_none() {
        let _g = lock_env();
        let key = ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV;
        let prev = env::var_os(key);
        env::remove_var(key);
        assert!(super::router_rs_cursor_review_gate_stop_max_nudges_cap().is_none());
        env::set_var(key, "3");
        assert_eq!(
            super::router_rs_cursor_review_gate_stop_max_nudges_cap(),
            Some(3)
        );
        env::set_var(key, "0");
        assert!(super::router_rs_cursor_review_gate_stop_max_nudges_cap().is_none());
        match prev {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
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
        assert_eq!(super::router_rs_rfv_max_rounds_cap(), 10000); // clamped to max
        match prev {
            Some(v) => env::set_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP", v),
            None => env::remove_var("ROUTER_RS_RFV_MAX_ROUNDS_CAP"),
        }
    }
}

/// `ROUTER_RS_RFV_MAX_ROUNDS_CAP`: RFV 循环最大轮次硬上限。
///
/// 默认 **1000**；可运行时调整，上限 10000 以防止极端值。
pub fn router_rs_rfv_max_rounds_cap() -> u64 {
    const MAX_CAP: u64 = 10000;
    const DEFAULT: u64 = 1000;
    env::var("ROUTER_RS_RFV_MAX_ROUNDS_CAP")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .map(|n| n.min(MAX_CAP))
        .unwrap_or(DEFAULT)
}
