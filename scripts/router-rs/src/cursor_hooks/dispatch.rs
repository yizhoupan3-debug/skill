/// 在 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 应急关闭时，仍调用各事件的真实 handler，
/// 但需要把可能附加给用户的督促 (`user-visible nags`) 从这几类事件输出里剥离。
/// 与正常模式相比，纯粹是「输出清洁」差异，handler 行为本身不变。
fn dispatch_disabled_should_strip_nags(lowered: &str) -> bool {
    matches!(
        lowered,
        "posttooluse" | "subagentstart" | "subagentstop" | "precompact"
    )
}

pub(crate) fn dispatch_cursor_hook_event(
    repo_root: &Path,
    event_name: &str,
    payload: &Value,
) -> Value {
    let lowered = event_name.trim().to_lowercase();
    let lowered = lowered.as_str();
    let disabled = cursor_review_gate_disabled_by_env();

    // Emergency short-circuit: in disabled mode beforesubmit / userpromptsubmit skip the
    // review-gate-aware handler entirely so the host always sees `continue: true`. Other events
    // share the same handler dispatch with the normal mode; differences live in nag scrubbing.
    if disabled && matches!(lowered, "beforesubmitprompt" | "userpromptsubmit") {
        return json!({ "continue": true });
    }

    let mut out = match lowered {
        "sessionstart" => handle_session_start(repo_root, payload),
        "beforesubmitprompt" | "userpromptsubmit" => handle_before_submit(repo_root, payload),
        "subagentstart" => handle_subagent_start(repo_root, payload),
        "subagentstop" => handle_subagent_stop(repo_root, payload),
        "posttooluse" => handle_post_tool_use(repo_root, payload),
        "beforeshellexecution" => handle_before_shell_execution(repo_root, payload),
        "aftershellexecution" => handle_after_shell_execution(repo_root, payload),
        "afteragentresponse" => handle_after_agent_response(repo_root, payload),
        "stop" => handle_stop(repo_root, payload),
        "afterfileedit" => handle_after_file_edit(repo_root, payload),
        "precompact" => handle_pre_compact(repo_root, payload),
        "sessionend" => handle_session_end(repo_root, payload),
        _ => json!({}),
    };

    if disabled && dispatch_disabled_should_strip_nags(lowered) {
        strip_cursor_hook_user_visible_nags(&mut out);
    }

    out
}
