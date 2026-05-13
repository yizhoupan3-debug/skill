/// 仅 **review** 路径的硬门控（独立上下文 subagent 证据链）；**不包含** `delegation_required`。
fn review_hard_armed(state: &ReviewGateState) -> bool {
    review_gate_armed(state.review_required, state.review_override)
}

/// Stop：`review` 场景下独立 subagent 证据是否满足（phase≥3：独立 start 后 stop 记账）。
fn review_subagent_evidence_satisfied(state: &ReviewGateState) -> bool {
    state.phase >= 3
}

fn review_stop_followup_needed(state: &ReviewGateState) -> bool {
    review_hard_armed(state) && !review_subagent_evidence_satisfied(state)
}

/// Stop / 观测 fixture 共用的 `need=` 段（前缀仍须含 `REVIEW_GATE` 供 `router_rs_observation` 分类）。
pub(crate) const REVIEW_GATE_FOLLOWUP_NEED_SEGMENT: &str =
    "need=deep_reviewer_cycle general-purpose|best-of-n fork_context=false";

/// Short, stable tail for `REVIEW_GATE incomplete` lines (after `need=`). Does not change the first
/// `router-rs` token (`REVIEW_GATE`) used by observation classification.
pub(crate) const REVIEW_GATE_FOLLOWUP_HINT_SEGMENT: &str =
    "hint=fork_context_json_false_not_omitted";

fn review_stop_followup_line(state: &ReviewGateState) -> String {
    format!(
        "router-rs REVIEW_GATE incomplete phase={} {} {}",
        state.phase, REVIEW_GATE_FOLLOWUP_NEED_SEGMENT, REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    )
}

/// `merge_hook_nudge_paragraph` 去重前缀：首行须与 `REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX` 常量一致以便每轮刷新同一段落。
pub(crate) const REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX: &str = "router-rs REVIEW_GATE detail";

/// 超过「完整硬行」上限后写入 `followup_message` 的短行（仍以 `router-rs REVIEW_GATE` 开头供观测分类）。
pub(crate) fn review_stop_followup_soft_line(
    state: &ReviewGateState,
    full_line_cap: u32,
) -> String {
    format!(
        "router-rs REVIEW_GATE incomplete mode=soft_nag full_line_cap={full_line_cap} phase={} stop_nudge_count={} see=.cursor/hook-state rg_clear|ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE=1|ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES=0(strict)|detail=additional_context",
        state.phase, state.review_followup_count
    )
}

/// 完整 `need=`/`hint=` 行：降级到 `additional_context` 时与 `REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX` 首行合并。
pub(crate) fn review_stop_followup_detail_paragraph(state: &ReviewGateState) -> String {
    format!(
        "{}\n{}",
        REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX,
        review_stop_followup_line(state)
    )
}

fn is_overridden(state: &ReviewGateState) -> bool {
    state.review_override || state.delegation_override
}

fn goal_is_satisfied(state: &ReviewGateState) -> bool {
    if !state.goal_required {
        return true;
    }
    // 全局 override（例如不要用子代理）仍可跳过整套 gate。
    if is_overridden(state) {
        return true;
    }
    state.goal_contract_seen && state.goal_progress_seen && state.goal_verify_or_block_seen
}

fn bump_phase(state: &mut ReviewGateState, target: u32) {
    state.phase = state.phase.max(target);
}

fn autopilot_pre_goal_followup_message() -> String {
    "Autopilot (/autopilot)：先写清 Goal 契约与验证口径；需要时再并行分工与证据索引（建议，非硬门槛）。确为小任务请**单独一行**拒因 token（如 small_task），不要自拟仿宿主 `router-rs …` 续跑行。"
        .to_string()
}

/// 连续 pre-goal 提示上限：beforeSubmit 每轮在仍缺 pre-goal 时累加计数，达到后自动 `pre_goal_review_satisfied=true`，避免卡死。
/// - **未设置**环境变量：默认 **8**（第八轮仍卡则放行）。
/// - `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES=0` / `false` / `off` / `no`：**关闭**自动放行（严格）。
/// - 正整数：自定义上限。
fn cursor_autopilot_pre_goal_max_nudges_cap() -> Option<u32> {
    #[cfg(test)]
    {
        // 单测未显式设变量时关闭自动放行，避免并行用例间状态与计数依赖。
        let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES") else {
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
        match std::env::var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES") {
            Err(_) => Some(8),
            Ok(raw) => {
                let t = raw.trim().to_ascii_lowercase();
                if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
                    return None;
                }
                t.parse::<u32>().ok().filter(|v| *v >= 1).or(Some(8))
            }
        }
    }
}

fn maybe_autopilot_pre_goal_nag_cap_release(state: &mut ReviewGateState) -> Option<&'static str> {
    if !crate::router_env_flags::router_rs_cursor_autopilot_pre_goal_enabled() {
        return None;
    }
    if !state.goal_required
        || state.pre_goal_review_satisfied
        || is_overridden(state)
        || state.reject_reason_seen
    {
        return None;
    }
    let cap = cursor_autopilot_pre_goal_max_nudges_cap()?;
    state.pre_goal_nag_count = state.pre_goal_nag_count.saturating_add(1);
    if state.pre_goal_nag_count < cap {
        return None;
    }
    state.pre_goal_review_satisfied = true;
    state.pre_goal_nag_count = 0;
    clear_review_gate_escalation_counters(state);
    Some("router-rs：pre-goal 提示已达上限，已自动放行以便继续执行（需要严格不自动放行请设 `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES=0`）。仍可在用户消息单独一行写 `small_task` 主动清门。")
}

/// 本地逃生舱：**仅当** `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 为 `1` / `true` / `yes` / `on`（大小写不敏感）时关闭门控；
/// unset、空串或其它任意值 **保持门控启用**（对齐 `router_rs_env_enabled_default_false`，避免任意非空误触）。
fn cursor_review_gate_disabled_by_env() -> bool {
    #[cfg(test)]
    {
        if let Some(v) = TEST_CURSOR_REVIEW_GATE_DISABLE.with(|c| c.get()) {
            return v;
        }
    }
    crate::router_env_flags::router_rs_env_enabled_default_false(
        "ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE",
    )
}

/// `subagentStart` 只能拒绝/提示，不能主动关闭既有 subagent；这里用活跃数避免继续堆积。
fn cursor_max_open_subagents() -> Option<u32> {
    let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS") else {
        return Some(DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS);
    };
    let t = raw.trim().to_ascii_lowercase();
    if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
        return None;
    }
    t.parse::<u32>()
        .ok()
        .filter(|v| *v > 0)
        .map(|v| v.min(MAX_CONCURRENT_SUBAGENTS_LIMIT as u32))
        .or(Some(DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS))
}

fn cursor_open_subagent_stale_after_secs() -> Option<i64> {
    let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS") else {
        return Some(DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS);
    };
    let t = raw.trim().to_ascii_lowercase();
    if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
        return None;
    }
    t.parse::<i64>()
        .ok()
        .filter(|v| *v > 0)
        .or(Some(DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS))
}

fn reset_stale_active_subagents(state: &mut ReviewGateState) -> bool {
    if state.active_subagent_count == 0 {
        return false;
    }
    let Some(stale_after_secs) = cursor_open_subagent_stale_after_secs() else {
        return false;
    };
    let Some(started_at) = state.active_subagent_last_started_at.as_deref() else {
        return false;
    };
    let Ok(started_at) = chrono::DateTime::parse_from_rfc3339(started_at) else {
        return false;
    };
    let age = Utc::now().signed_duration_since(started_at.with_timezone(&Utc));
    if age.num_seconds() <= stale_after_secs {
        return false;
    }
    state.active_subagent_count = 0;
    state.active_subagent_last_started_at = None;
    true
}

fn subagent_limit_denial(active: u32, limit: u32) -> Value {
    json!({
        "permission": "deny",
        "user_message": format!(
            "router-rs：当前会话已有 {active} 个 subagent 仍标记为打开（上限 {limit}，等于 `max_concurrent_subagents_limit` 契约）。请先等已有 subagent 结束/关闭，或确认它们已 stale 后清理会话状态；如需临时关闭限流，设置 ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS=0。"
        )
    })
}

pub(crate) fn apply_cursor_hook_output_policy(output: &mut Value) {
    crate::router_rs_observation::attach_router_rs_observation(
        output,
        crate::router_rs_observation::HookObservationHost::Cursor,
    );
    let max_out = crate::router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes();
    if let Some(Value::String(s)) = output.get_mut("additional_context") {
        let next = truncate_cursor_hook_outbound_context(s.as_str(), max_out);
        *s = next;
    }

    let absurd_followup_threshold =
        crate::router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes()
            .saturating_mul(4)
            .max(32 * 1024);
    if let Some(Value::String(s)) = output.get_mut("followup_message") {
        if s.len() > absurd_followup_threshold {
            *s = truncate_cursor_hook_outbound_context(s.as_str(), max_out);
        }
    }
}

/// Cursor outbound truncation: UTF-8 byte cap; prefix retained; **fixed suffix** so operators can
/// tell budget clipping from gate logic. (Variable names may say `_CHARS`; semantics are bytes.)
pub(crate) const CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX: &str = "...[~trunc]";

/// Cursor 出站 `additional_context` / 极端 `followup_message`：**UTF-8 字节预算**，前缀优先，末尾固定
/// [`CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX`]（与 Codex `truncate_codex_additional_context_bytes` 的 `...` 相比更可观测）。
fn truncate_cursor_hook_outbound_context(combined: &str, max_bytes: usize) -> String {
    if combined.len() <= max_bytes {
        return combined.to_string();
    }
    let suf = CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX;
    let suf_len = suf.len();
    if max_bytes <= suf_len {
        let mut cut = max_bytes.min(combined.len());
        while cut > 0 && !combined.is_char_boundary(cut) {
            cut -= 1;
        }
        return combined[..cut].to_string();
    }
    let budget = max_bytes.saturating_sub(suf_len);
    let mut cut = budget.min(combined.len());
    while cut > 0 && !combined.is_char_boundary(cut) {
        cut -= 1;
    }
    if let Some(pos) = combined[..cut].rfind('\n') {
        if pos > 0 {
            cut = pos;
        }
    }
    while cut > 0 && !combined.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{}", &combined[..cut], suf)
}

/// 应急关闭门控时仍执行 PostToolUse/Subagent 状态更新，但不对模型注入门控类提示（与 SILENT 剥离字段一致）。
fn strip_cursor_hook_user_visible_nags(output: &mut Value) {
    if let Some(obj) = output.as_object_mut() {
        obj.remove("followup_message");
        obj.remove("additional_context");
        crate::router_rs_observation::strip_router_rs_observation(output);
    }
}

/// 清门或 subagent 满足 review 后归零，避免 `followup_count` 长期累积导致 **escalation** 粘住。
fn clear_review_gate_escalation_counters(state: &mut ReviewGateState) {
    state.followup_count = 0;
    state.review_followup_count = 0;
    state.pre_goal_nag_count = 0;
}

/// `GOAL_STATE` 列表字段是否含至少一条非空字符串（避免 `[""]` 这种伪非空数组）。
fn goal_state_list_any_nonempty_string(goal: &Value, key: &str) -> bool {
    match goal.get(key) {
        Some(Value::Array(a)) => a
            .iter()
            .any(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)),
        Some(Value::String(s)) => !s.trim().is_empty(),
        _ => false,
    }
}

/// 用 `GOAL_STATE.json` + `EVIDENCE_INDEX.json` 补全 goal 门控（只置 true，不收回），避免助手未写
/// 「Goal / Checkpoint / verified」等关键词时 Stop 报 `AG_FOLLOWUP` 四项全缺。
///
/// `arm_if_goal_file`：**Stop** 等收口路径传 `true`，在磁盘已有 GOAL 但 hook-state 未写 `goal_required` 时仍回补；
/// **beforeSubmit** 传 `false`，避免普通消息因残留 GOAL 文件被误标为 autopilot。
///
/// **`pre_goal_review_satisfied`（磁盘旁路）**：Stop 路径始终可由 hydration 置真。beforeSubmit 路径
/// 在 `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` 开启时**不**因仅存在磁盘 GOAL 而置真，以免遗留
/// `GOAL_STATE` 误放行 pre-goal；其余 goal 字段的 hydrate（contract/progress/verify 等）仍执行。
fn hydrate_goal_gate_from_disk(
    repo_root: &Path,
    state: &mut ReviewGateState,
    arm_if_goal_file: bool,
    frame: &crate::task_state::CursorContinuityFrame,
) {
    if !state.goal_required && !arm_if_goal_file {
        return;
    }
    let Some((goal, task_id)) = frame.hydration_goal.as_ref() else {
        return;
    };
    if arm_if_goal_file {
        state.goal_required = true;
    }
    if arm_if_goal_file || !crate::router_env_flags::router_rs_cursor_pre_goal_strict_disk_enabled()
    {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
    }
    let gtext = goal
        .get("goal")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let has_goal_text = !gtext.is_empty();
    let validation_nonempty = goal_state_list_any_nonempty_string(goal, "validation_commands");
    let non_goals_nonempty = goal_state_list_any_nonempty_string(goal, "non_goals");
    // Contract should be "deep enough" even when hydrated from disk: require non-empty goal,
    // non-goals, validation commands, and done_when (with >=2 items).
    let done_when_items = goal
        .get("done_when")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .count()
        })
        .unwrap_or(0);
    if has_goal_text && non_goals_nonempty && validation_nonempty && done_when_items >= 2 {
        state.goal_contract_seen = true;
    }
    let checkpointed = goal
        .get("checkpoints")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let (evidence_rows, evidence_ok) =
        crate::autopilot_goal::task_evidence_artifacts_summary_for_task(
            repo_root,
            task_id.as_str(),
        );
    let st_raw = goal.get("status").and_then(Value::as_str).unwrap_or("");
    let st_lc = st_raw.trim().to_ascii_lowercase();
    // `running` 为真源默认；`in_progress` 偶见于外部模板；缺省 status 且已有 goal 文本则按进行中回补。
    let active_like =
        matches!(st_lc.as_str(), "running" | "in_progress") || (has_goal_text && st_lc.is_empty());
    let disk_contract_signal = (done_when_items >= 2) && validation_nonempty && non_goals_nonempty;
    // 进行中状态或磁盘契约字段：进展/验收由 GOAL_STATE 承载，Stop 不强求聊天关键词。
    if checkpointed || evidence_rows || (has_goal_text && (disk_contract_signal || active_like)) {
        state.goal_progress_seen = true;
    }
    let blocker = goal
        .get("blocker")
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    // Tightened (review P0-C): plain "running goal with disk contract" is no longer enough to
    // claim `verification_or_blocker`. Explicit signals required:
    //   1) terminal status (blocked / completed / paused), OR
    //   2) explicit blocker text, OR
    //   3) at least one successful EVIDENCE_INDEX row, OR
    //   4) a checkpoint already recorded (model wrote progress at least once).
    // Without one of these, the gate stays open so the model is asked to either run a verifier
    // command, post a blocker, or record a checkpoint before being treated as having "verified".
    if matches!(st_lc.as_str(), "blocked" | "completed" | "paused")
        || blocker
        || evidence_ok
        || checkpointed
    {
        state.goal_verify_or_block_seen = true;
    }
}

fn goal_missing_parts(state: &ReviewGateState) -> String {
    let mut missing = Vec::new();
    if !state.goal_contract_seen {
        missing.push("goal_contract");
    }
    if !state.goal_progress_seen {
        missing.push("checkpoint_progress");
    }
    if !state.goal_verify_or_block_seen {
        missing.push("verification_or_blocker");
    }
    missing.join(", ")
}

/// Stop 上的 goal 门控短码：固定带 `router-rs AG_FOLLOWUP` 前缀，避免与陈旧/错误的自拟续跑标签混淆；附一行可执行脱困提示（仍保持单行优先）。
fn goal_stop_followup_line(state: &ReviewGateState) -> String {
    let parts = goal_missing_parts(state);
    let mut line = format!("router-rs AG_FOLLOWUP missing_parts={parts}");
    if state.goal_followup_count >= 3 {
        line.push_str(" | 已连续多轮 Stop 未满足门控；若确为小任务请直接单独一行 small_task");
    }
    line
}

fn state_lock_degraded_followup() -> &'static str {
    "router-rs：hook-state 锁不可用，本闸门控降级。收口前须见独立 subagent lane，或在**用户消息**中单独一行写拒因。"
}

fn lock_failure_followup_for_before_submit(event: &Value) -> (bool, String) {
    let text = prompt_text(event);
    let signal_text = hook_event_signal_text(event, &text, "");
    let review = is_review_prompt(&text);
    let autopilot_entrypoint = is_autopilot_goal_entry_prompt(&text, &signal_text);
    let review_arms = review && !autopilot_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let overridden = has_override(&text);

    let strong_constraint = (review_arms || delegation || autopilot_entrypoint) && !overridden;
    if strong_constraint {
        return (
            false,
            "router-rs：hook-state 锁不可用，本条为严格 review/委托/autopilot，**已拦截提交**。请修锁/权限后重试，或起 subagent / 写明拒因。"
                .to_string(),
        );
    }

    (
        true,
        "router-rs：hook-state 锁不可用，门控**降级**；非严格提示仍可继续。".to_string(),
    )
}

fn lock_failure_followup_for_stop(event: &Value) -> String {
    let text = prompt_text(event);
    let response_text = agent_response_text(event);
    let signal_text = hook_event_signal_text(event, &text, &response_text);
    let review = is_review_prompt(&text);
    let autopilot_entrypoint = is_autopilot_goal_entry_prompt(&text, &signal_text);
    let review_arms = review && !autopilot_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let overridden = has_override(&text) || saw_reject_reason(&signal_text, &text);

    let strong_constraint = (review_arms || delegation || autopilot_entrypoint) && !overridden;
    if strong_constraint {
        return "router-rs：hook-state 锁不可用，本轮须严格 review/委托/autopilot 证据。合并前请修复锁/权限并重试，或 subagent/拒因。".to_string();
    }
    state_lock_degraded_followup().to_string()
}
