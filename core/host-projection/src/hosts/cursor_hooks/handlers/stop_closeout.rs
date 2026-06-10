// Stop 收尾：SESSION_CLOSE_STYLE 软提示与 strict closeout 完成宣称检测。
// 从原 `handlers.rs` 单体拆出的首片（P4 deferred 续切）；`handle_stop` 编排仍在
// `handlers_parts/handlers_stop.inc.rs`。

/// Stop 收尾：在**无**硬 `followup_message` 时每轮稳定注入一条软提示，避免仅依赖规则时「有时有续跑段落、有时什么也没有」。
///
/// Canonical `ROUTER_RS_SESSION_CLOSE_STYLE_NUDGE`; legacy `ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE` still honored.
/// `=0|false|off|no` 关闭（默认开启）。
const SESSION_CLOSE_STYLE_LINE_PREFIX: &str = "SESSION_CLOSE_STYLE";

fn session_close_style_stop_nudge_enabled_by_env() -> bool {
    let canonical_key = "ROUTER_RS_SESSION_CLOSE_STYLE_NUDGE";
    let legacy_key = "ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE";
    let raw = std::env::var(canonical_key)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var(legacy_key).ok().filter(|s| !s.trim().is_empty()));
    match raw {
        None => true,
        Some(raw) => {
            let t = raw.trim().to_ascii_lowercase();
            !matches!(t.as_str(), "" | "0" | "false" | "off" | "no")
        }
    }
}

fn merge_session_close_style_nudge_when_soft_terminal(output: &mut Value) {
    if output.get("followup_message").is_some() {
        return;
    }
    if !hooks::router_rs_operator_inject_globally_enabled() {
        return;
    }
    if !session_close_style_stop_nudge_enabled_by_env() {
        return;
    }
    let msg = concat!(
        "SESSION_CLOSE_STYLE: 收尾简短、像口头交代就行：这轮做了什么、效果如何、还有没有没擦干净的地方要不要接着弄；",
        "别默认摊开路径清单、长 diff 或整段命令，除非对方点名要。"
    );
    core_state::state_manager::merge_hook_nudge_paragraph(
        output,
        msg,
        SESSION_CLOSE_STYLE_LINE_PREFIX,
        false,
    );
}

fn finalize_stop_hook_outputs(
    _repo_root: &Path,
    output: &mut Value,
    _frame: &core_state::task_state::CursorContinuityFrame,
) {
    merge_session_close_style_nudge_when_soft_terminal(output);
}

/// Assistant 回复文本侧的完成宣称检测：先剥离引文 / 代码块 / URL，再交由 `hook_common`
/// 的单一 token 表扫描，与 `closeout_enforcement::summary_claims_completion` 共用一份关键词
/// 集合，避免漂移。中文使用多字短语，避开「完成度 / 讨论完成任务拆分」等子串误报。
fn completion_claimed_in_text(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    let unescaped = text.replace("\\", "");
    let sanitized = strip_quoted_or_codeblock_or_url(&unescaped);
    core_policy::hook_common::contains_completion_claim_token(&sanitized)
}

fn closeout_followup_for_completion_claim(
    repo_root: &Path,
    task_id: &str,
) -> Result<Option<String>, String> {
    if !hooks::closeout_programmatic_enforcement_enabled() {
        return Ok(None);
    }
    let record_path = hooks::closeout_record_path_for_task(repo_root, task_id)?;
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
    let eval = hooks::evaluate_closeout_record_file_for_task(
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

/// Strict closeout：**助手回复文本**中出现完成宣称且存在 continuation task（与 hydration 同指针语义）时的硬 Stop 文案（与 `dispatch`/`handle_stop` 共用，避免分叉）。
///
/// `Err(evaluator)` 与 `Ok(Some(..))` 均返回 `Some`；未宣称完成、`Ok(None)` 或无 task 时返回 `None`。
fn stop_hard_closeout_followup_for_assistant_response(
    repo_root: &Path,
    response_text: &str,
) -> Option<String> {
    if !completion_claimed_in_text(response_text) {
        return None;
    }
    let frame = core_state::task_state::resolve_cursor_continuity_frame(repo_root);
    // Pointer 机制已移除：先尝试 frame，再回退到 task_registry.json
    let tid = frame
        .hydration_goal
        .map(|(_, task_id)| task_id)
        .or(frame.pointer_view.task_id)
        .filter(|s| !s.is_empty())
        .or_else(|| hooks::first_task_id_from_registry(repo_root))?;
    match closeout_followup_for_completion_claim(repo_root, &tid) {
        Ok(Some(msg)) => Some(msg),
        Ok(None) => None,
        Err(err) => Some(format!(
            "CLOSEOUT_FOLLOWUP task_id={tid} reason=evaluator_error error={err}"
        )),
    }
}
