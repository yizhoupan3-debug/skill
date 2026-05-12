pub(crate) fn dispatch_cursor_hook_event(
    repo_root: &Path,
    event_name: &str,
    payload: &Value,
) -> Value {
    let lowered = event_name.trim().to_lowercase();
    let lowered = lowered.as_str();
    if cursor_review_gate_disabled_by_env() {
        let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
        return match lowered {
            "sessionstart" => handle_session_start(repo_root, payload),
            "beforesubmitprompt" | "userpromptsubmit" => {
                let mut out = json!({ "continue": true });
                merge_continuity_followups_before_submit(repo_root, &mut out, &frame);
                out
            }
            "sessionend" => handle_session_end(repo_root, payload),
            "stop" => handle_stop(repo_root, payload),
            "posttooluse" => {
                let mut out = handle_post_tool_use(repo_root, payload);
                strip_cursor_hook_user_visible_nags(&mut out);
                out
            }
            "beforeshellexecution" => handle_before_shell_execution(repo_root, payload),
            "aftershellexecution" => handle_after_shell_execution(repo_root, payload),
            "subagentstart" => {
                let mut out = handle_subagent_start(repo_root, payload);
                strip_cursor_hook_user_visible_nags(&mut out);
                out
            }
            "subagentstop" => {
                let mut out = handle_subagent_stop(repo_root, payload);
                strip_cursor_hook_user_visible_nags(&mut out);
                out
            }
            "afterfileedit" => handle_after_file_edit(repo_root, payload),
            "precompact" => {
                let mut out = handle_pre_compact(repo_root, payload);
                strip_cursor_hook_user_visible_nags(&mut out);
                out
            }
            _ => json!({}),
        };
    }
    match lowered {
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
    }
}
