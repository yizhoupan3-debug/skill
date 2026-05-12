/// 与 `.cursor/hook-state` 锁无关：只读合并 continuity 续跑，避免门控降级或应急短路时 goal/RFV 静默消失。
fn merge_continuity_followups(
    repo_root: &Path,
    output: &mut Value,
    frame: &crate::task_state::CursorContinuityFrame,
) {
    let autopilot = build_autopilot_drive_followup_using_frame(repo_root, frame);
    let rfv = build_rfv_loop_followup_using_frame(repo_root, frame);
    match (autopilot, rfv) {
        (Some(ap_msg), Some(rfv_msg)) if !ap_msg.is_empty() && !rfv_msg.is_empty() => {
            // Goal + RFV 同时活跃：只保留 **一条** `AUTOPILOT_DRIVE` 段落，把 RFV 压缩成尾注，
            // 避免再插第二段 `RFV_LOOP_CONTINUE` 头行（token 与 scan 噪声双高）。
            let stripped = rfv_msg.lines().next().map(str::trim).unwrap_or("");
            let note = stripped
                .strip_prefix("RFV_LOOP_CONTINUE:")
                .map(str::trim)
                .unwrap_or(stripped);
            let merged = if note.is_empty() {
                format!("{ap_msg}\nAlso: RFV active")
            } else {
                format!("{ap_msg}\nAlso: RFV active ({note})")
            };
            crate::autopilot_goal::merge_hook_nudge_paragraph(
                output,
                &merged,
                "AUTOPILOT_DRIVE",
                false,
            );
        }
        (Some(msg), _) if !msg.is_empty() => {
            crate::autopilot_goal::merge_hook_nudge_paragraph(
                output,
                &msg,
                "AUTOPILOT_DRIVE",
                false,
            );
        }
        (_, Some(msg)) if !msg.is_empty() => {
            crate::autopilot_goal::merge_hook_nudge_paragraph(
                output,
                &msg,
                "RFV_LOOP_CONTINUE",
                false,
            );
        }
        _ => {}
    }
}

/// Stop 收尾：在**无**硬 `followup_message` 时每轮稳定注入一条软提示，避免仅依赖规则时「有时有续跑段落、有时什么也没有」。
///
/// 可用 `ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE=0|false|off|no` 关闭（默认开启）。
const SESSION_CLOSE_STYLE_LINE_PREFIX: &str = "SESSION_CLOSE_STYLE";

fn session_close_style_stop_nudge_enabled_by_env() -> bool {
    match std::env::var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE") {
        Err(_) => true,
        Ok(raw) => {
            let t = raw.trim().to_ascii_lowercase();
            !matches!(t.as_str(), "" | "0" | "false" | "off" | "no")
        }
    }
}

fn merge_session_close_style_nudge_when_soft_terminal(output: &mut Value) {
    if output.get("followup_message").is_some() {
        return;
    }
    if !crate::router_env_flags::router_rs_operator_inject_globally_enabled() {
        return;
    }
    if !session_close_style_stop_nudge_enabled_by_env() {
        return;
    }
    let msg = concat!(
        "SESSION_CLOSE_STYLE: 收尾简短、像口头交代就行：这轮做了什么、效果如何、还有没有没擦干净的地方要不要接着弄；",
        "别默认摊开路径清单、长 diff 或整段命令，除非对方点名要。"
    );
    crate::autopilot_goal::merge_hook_nudge_paragraph(
        output,
        msg,
        SESSION_CLOSE_STYLE_LINE_PREFIX,
        false,
    );
}

fn finalize_stop_hook_outputs(
    repo_root: &Path,
    output: &mut Value,
    frame: &crate::task_state::CursorContinuityFrame,
) {
    merge_continuity_followups(repo_root, output, frame);
    merge_session_close_style_nudge_when_soft_terminal(output);
}

fn build_autopilot_drive_followup_using_frame(
    repo_root: &Path,
    frame: &crate::task_state::CursorContinuityFrame,
) -> Option<String> {
    if let (Some(task_id), Some(goal)) = (
        frame.pointer_view.task_id.as_deref(),
        frame.pointer_view.goal_state.as_ref(),
    ) {
        return crate::autopilot_goal::build_autopilot_drive_followup_message_from_state(
            repo_root, task_id, goal,
        );
    }
    crate::autopilot_goal::build_autopilot_drive_followup_message(repo_root)
}

fn build_rfv_loop_followup_using_frame(
    repo_root: &Path,
    frame: &crate::task_state::CursorContinuityFrame,
) -> Option<String> {
    let active = crate::autopilot_goal::read_active_task_id(repo_root)?;
    if frame.pointer_view.task_id.as_deref() == Some(active.as_str()) {
        if let Some(ref s) = frame.pointer_view.rfv_loop_state {
            return crate::rfv_loop::build_rfv_loop_followup_message_from_state(
                repo_root, &active, s,
            );
        }
    }
    crate::rfv_loop::build_rfv_loop_followup_message(repo_root)
}

fn merge_continuity_followups_before_submit(
    _repo_root: &Path,
    _output: &mut Value,
    _frame: &crate::task_state::CursorContinuityFrame,
) {
    // beforeSubmit 侧的 Goal/RFV 续跑注入已退役；续跑只保留在必要事件的单一路径。
}

fn completion_claimed_in_text(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    // Keep English + Chinese phrases aligned with closeout enforcement completion detection.
    // Chinese uses multi-character phrases to avoid substring hits inside unrelated words (e.g. 「完成度」).
    const EN: &[&str] = &["done", "finished", "completed", "passed", "succeeded"];
    const ZH_PHRASES: &[&str] = &[
        "已完成",
        "已经完成",
        "全部完成",
        "完成了",
        "验证通过",
        "测试通过",
        "审核通过",
        "已通过",
        "搞定",
    ];
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    let lower = sanitized.to_ascii_lowercase();
    if EN.iter().any(|kw| lower.contains(&kw.to_ascii_lowercase())) {
        return true;
    }
    ZH_PHRASES.iter().any(|p| sanitized.contains(p))
}

fn closeout_followup_for_completion_claim(
    repo_root: &Path,
    task_id: &str,
) -> Result<Option<String>, String> {
    if !crate::framework_runtime::closeout_programmatic_enforcement_enabled() {
        return Ok(None);
    }
    let record_path = crate::framework_runtime::closeout_record_path_for_task(repo_root, task_id);
    if !record_path.is_file() {
        return Ok(Some(format!(
            "CLOSEOUT_FOLLOWUP task_id={task_id} reason=missing_record path={}\n\
请在完成态宣称前写入 closeout record 并通过评估：\n\
- 记录路径：{}\n\
- 评估命令：router-rs closeout evaluate --repo-root \"{}\" --task-id \"{}\" --record-path \"{}\"",
            record_path.display(),
            record_path.display(),
            repo_root.display(),
            task_id,
            record_path.display()
        )));
    }
    let eval = crate::framework_runtime::evaluate_closeout_record_file_for_task(
        repo_root,
        task_id,
        &record_path,
    )?;
    let allowed = eval
        .get("closeout_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if allowed {
        return Ok(None);
    }
    let violations = eval.get("violations").cloned().unwrap_or(Value::Null);
    let missing = eval.get("missing_evidence").cloned().unwrap_or(Value::Null);
    Ok(Some(format!(
        "CLOSEOUT_FOLLOWUP task_id={task_id} reason=evaluation_failed path={}\n\
closeout_enforcement blocked completion: closeout_allowed=false\n\
violations={}\nmissing_evidence={}\n\
请修复 violations，或降级 completion/status，再重新评估。",
        record_path.display(),
        violations,
        missing
    )))
}

/// Strict closeout：**助手回复文本**中出现完成宣称且存在 `active_task` 时的硬 Stop 文案（与 `dispatch`/`handle_stop` 共用，避免分叉）。
///
/// `Err(evaluator)` 与 `Ok(Some(..))` 均返回 `Some`；未宣称完成、`Ok(None)` 或无 task 时返回 `None`。
#[allow(dead_code)] // false positive: callers live in other `include!` fragments of the same `cursor_hooks` module.
fn stop_hard_closeout_followup_for_assistant_response(
    repo_root: &Path,
    response_text: &str,
) -> Option<String> {
    if !completion_claimed_in_text(response_text) {
        return None;
    }
    let tid = crate::autopilot_goal::read_active_task_id(repo_root)?;
    match closeout_followup_for_completion_claim(repo_root, &tid) {
        Ok(Some(msg)) => Some(msg),
        Ok(None) => None,
        Err(err) => Some(format!(
            "CLOSEOUT_FOLLOWUP task_id={tid} reason=evaluator_error error={err}"
        )),
    }
}

pub const STATE_VERSION: u32 = 3;

fn subagent_types() -> &'static [&'static str] {
    &[
        "generalpurpose",
        "explore",
        "shell",
        "browser-use",
        "browseruse",
        "cursor-guide",
        "cursorguide",
        "ci-investigator",
        "ciinvestigator",
        "best-of-n-runner",
        "bestofnrunner",
        "explorer",
    ]
}

fn subagent_tool_names() -> &'static [&'static str] {
    &[
        "task",
        "functions.task",
        "functions.subagent",
        "functions.spawn_agent",
        "subagent",
        "spawn_agent",
    ]
}

/// MCP / 宿主可能使用 `…subagent…` 等未列入清单的工具名。
fn tool_name_matches_subagent_lane(normalized: &str) -> bool {
    if subagent_tool_names().contains(&normalized) {
        return true;
    }
    normalized.contains("subagent") || normalized.contains("spawn_agent") || normalized == "task"
}

fn framework_entrypoint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(^|\s)/(autopilot|team|gitx|update)\b").expect("invalid regex")
    })
}

fn autopilot_entrypoint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(^|\s)/autopilot\b").expect("invalid regex"))
}

fn goal_contract_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(goal|done when|validation commands|checkpoint plan|non-goals)\b|(目标|完成条件|验证命令|检查点|非目标)",
        )
        .expect("invalid regex")
    })
}

fn goal_progress_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(checkpoint|milestone|progress|next step)\b|(检查点|里程碑|进度|下一步)")
            .expect("invalid regex")
    })
}

fn goal_verify_or_block_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(verified|verification|test passed|blocker)\b|(已验证|验证通过|测试通过|阻塞)",
        )
        .expect("invalid regex")
    })
}

/// Task/subagent 调用里明示 `fork_context: true` 时视为与主会话共享上下文，不满足 autopilot 要求的「独立上下文」预检。
/// 部分宿主以字符串 `"true"` / `"false"` 下发，需与 JSON bool 同等解析。
fn fork_context_from_tool(event: &Value, tool_input: &Value) -> Option<bool> {
    fork_context_from_values(tool_input, Some(event))
}

fn counts_as_independent_context_fork(fork: Option<bool>) -> bool {
    independent_context_fork(fork)
}

fn fork_context_explicit_false(fork: Option<bool>) -> bool {
    independent_context_fork(fork)
}

fn has_goal_contract_signal(text: &str) -> bool {
    // A "goal contract" should be hard to satisfy by accident. Historically we matched any one
    // keyword (Goal / Done when / Validation / ...), which made it too easy for a one-liner to
    // count as a full contract and prematurely satisfy the goal gate.
    //
    // New rule (Cursor): require a structured, non-empty contract with:
    // - Goal
    // - Non-goals
    // - Done when (with >=2 acceptance items)
    // - Validation commands (non-empty)
    //
    // Support both English and Chinese headings.
    if goal_contract_re().is_match(text) {
        // Keep legacy keyword match as a weak signal, but only after passing the strong check
        // below. This preserves backwards compatibility for callers that only gate on this bool.
    }
    has_structured_goal_contract(text)
}

fn has_structured_goal_contract(text: &str) -> bool {
    let goal_ok =
        nonempty_inline_heading_any(text, "Goal") || nonempty_inline_heading_any(text, "目标");
    let non_goals_ok = nonempty_inline_heading_any(text, "Non-goals")
        || nonempty_inline_heading_any(text, "非目标");
    let validation_ok = nonempty_inline_heading_any(text, "Validation commands")
        || nonempty_inline_heading_any(text, "验证命令");
    let done_when_items = count_done_when_items(text);
    goal_ok && non_goals_ok && validation_ok && done_when_items >= 2
}

fn nonempty_inline_heading_any(text: &str, heading: &str) -> bool {
    let pattern = format!(r"(?im)^\s*{}\s*[:：]\s*(\S.+)$", regex::escape(heading));
    let Ok(re) = Regex::new(&pattern) else {
        return false;
    };
    re.captures(text)
        .and_then(|cap| cap.get(1))
        .map(|m| !m.as_str().trim().is_empty())
        .unwrap_or(false)
}

fn count_done_when_items(text: &str) -> usize {
    // Prefer bullet/numbered items under "Done when:" / "完成条件:".
    // Fallback: treat an inline list after the heading as multiple items if it contains clear
    // separators.
    const HEADINGS: [&str; 2] = ["Done when", "完成条件"];
    let numbered_line_re = Regex::new(r"(?m)^\d+\.\s+\S").ok();
    let re_done = Regex::new(&format!(
        r"(?im)^\s*{}\s*[:：]\s*(.*)$",
        regex::escape(HEADINGS[0])
    ));
    let re_zh = Regex::new(&format!(
        r"(?im)^\s*{}\s*[:：]\s*(.*)$",
        regex::escape(HEADINGS[1])
    ));
    let heading_pairs = [(HEADINGS[0], re_done.ok()), (HEADINGS[1], re_zh.ok())];
    for (h, maybe_re) in heading_pairs {
        let Some(re) = maybe_re else {
            continue;
        };
        let Some(cap) = re.captures(text) else {
            continue;
        };
        let inline = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        if !inline.is_empty() {
            // Inline: split on common separators; require at least 2 non-empty parts.
            let parts = inline
                .split(&[';', '；', ',', '，', '|', '、'][..])
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .count();
            if parts >= 2 {
                return parts;
            }
        }

        // Block-style: count bullet/numbered lines after the heading until the next heading-ish
        // line or a blank-only tail. This is intentionally conservative.
        let mut in_section = false;
        let mut count = 0usize;
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() {
                if in_section {
                    // Allow blank lines inside section; do not terminate immediately.
                    continue;
                }
                continue;
            }
            if !in_section {
                let lowered = line.to_ascii_lowercase();
                let target = h.to_ascii_lowercase();
                if lowered.starts_with(&target) && (lowered.contains(':') || line.contains('：')) {
                    in_section = true;
                }
                continue;
            }

            // Stop if we hit another contract heading.
            if goal_contract_re().is_match(line)
                && !line
                    .to_ascii_lowercase()
                    .starts_with(&h.to_ascii_lowercase())
            {
                break;
            }

            let is_bullet = line.starts_with("- ")
                || line.starts_with("* ")
                || line.starts_with("• ")
                || numbered_line_re.as_ref().is_some_and(|r| r.is_match(line));
            if is_bullet {
                count += 1;
            }
        }
        if count > 0 {
            return count;
        }
    }
    0
}

fn has_goal_progress_signal(text: &str) -> bool {
    goal_progress_re().is_match(text)
}

fn has_goal_verify_or_block_signal(text: &str) -> bool {
    goal_verify_or_block_re().is_match(text)
}

/// Task/subagent 工具载荷上的类型字段（与 Codex `codex_subagent_type_evidence` 对齐）：部分宿主用 `type` 代替 `subagent_type`。
fn cursor_subagent_type_pair(tool_input: &Value, event: &Value) -> (String, String) {
    let sub_raw = tool_input
        .get("subagent_type")
        .or_else(|| tool_input.get("subagentType"))
        .or_else(|| tool_input.get("type"))
        .or_else(|| tool_input.get("lane"))
        .or_else(|| tool_input.get("lane_type"))
        .or_else(|| tool_input.get("laneType"))
        .or_else(|| event.get("subagent_type"))
        .or_else(|| event.get("subagentType"))
        .or_else(|| event.get("type"))
        .and_then(Value::as_str);
    let agent_raw = tool_input
        .get("agent_type")
        .or_else(|| tool_input.get("agentType"))
        .or_else(|| event.get("agent_type"))
        .or_else(|| event.get("agentType"))
        .and_then(Value::as_str);
    (
        normalize_subagent_type(sub_raw),
        normalize_subagent_type(agent_raw),
    )
}

fn typed_subagent_in_allowlist(sub_type: &str, agent_type: &str) -> bool {
    (!sub_type.is_empty() && subagent_types().contains(&sub_type))
        || (!agent_type.is_empty() && subagent_types().contains(&agent_type))
}

/// `/autopilot` pre-goal：白名单 **或** 任一非空 lane/agent 字段即视为已起 sidecar（宿主常发自定义 lane 名）。
fn pre_goal_subagent_kind_ok(sub_type: &str, agent_type: &str) -> bool {
    typed_subagent_in_allowlist(sub_type, agent_type)
        || !sub_type.is_empty()
        || !agent_type.is_empty()
}

fn review_subagent_kind_ok_loose_when_cursor_gate_disabled(
    sub_type: &str,
    agent_type: &str,
) -> bool {
    fn lane_ok(lane: &str) -> bool {
        matches!(
            lane,
            "explore"
                | "explorer"
                | "general-purpose"
                | "generalpurpose"
                | "ci-investigator"
                | "ciinvestigator"
                | "cursor-guide"
                | "cursorguide"
                | "best-of-n-runner"
                | "bestofnrunner"
        )
    }
    (!sub_type.is_empty() && lane_ok(sub_type)) || (!agent_type.is_empty() && lane_ok(agent_type))
}

fn review_subagent_kind_ok(sub_type: &str, agent_type: &str) -> bool {
    if cursor_review_gate_disabled_by_env() {
        return review_subagent_kind_ok_loose_when_cursor_gate_disabled(sub_type, agent_type);
    }
    // 默认可清除 `REVIEW_GATE` 的深度审稿 lane：**不**含 `explore` / CI / guide（只做辅查，不算一轮独立深度 reviewer）。
    (!sub_type.is_empty() && crate::hook_common::is_deep_review_gate_lane_normalized(sub_type))
        || (!agent_type.is_empty()
            && crate::hook_common::is_deep_review_gate_lane_normalized(agent_type))
}

fn first_nonempty_tool_or_event_str(event: &Value, tool_input: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = tool_input.get(*key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Some(value) = event.get(*key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    String::new()
}

fn review_subagent_cycle_key(
    event: &Value,
    tool_input: &Value,
    sub_type: &str,
    agent_type: &str,
) -> Option<String> {
    let id = first_nonempty_tool_or_event_str(
        event,
        tool_input,
        &[
            "subagent_id",
            "subagentId",
            "agent_id",
            "agentId",
            "task_id",
            "taskId",
            "run_id",
            "runId",
            "id",
        ],
    );
    if !id.is_empty() {
        return Some(format!("id:{id}"));
    }
    let lane = if !sub_type.is_empty() {
        sub_type
    } else {
        agent_type
    };
    if lane.is_empty() {
        None
    } else {
        Some(format!("lane:{lane}"))
    }
}
