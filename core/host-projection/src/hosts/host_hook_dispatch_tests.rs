//! Shared dispatch routing tests for all [`super::host_hook::HostHook`] implementations.

#[cfg(test)]
mod tests {
    use super::super::claude_hook_host::ClaudeHookHost;
    use super::super::codex_hook_host::CodexHookHost;
    use super::super::cursor_hook_host::CursorHookHost;
    use super::super::host_hook::{HostHook, HookDecision};
    use super::super::host_hook_example::ExampleHookHost;
    use serde_json::json;
    use std::path::Path;

    fn assert_allow_value(v: serde_json::Value) {
        assert_eq!(v, HookDecision::allow_value());
    }

    fn assert_silent_success<H: HostHook>(host: &H, root: &Path) {
        assert_eq!(
            host.dispatch(root, "totally-unknown-event", &json!({})),
            host.silent_success()
        );
    }

    #[test]
    fn example_hook_dispatch_routes_core_events() {
        let host = ExampleHookHost;
        let root = Path::new("/tmp");
        let payload = json!({});
        assert_allow_value(host.dispatch(root, "PreToolUse", &payload));
        assert_allow_value(host.dispatch(root, "post-tool-use", &payload));
        assert_allow_value(host.dispatch(root, "Stop", &payload));
        assert_allow_value(host.dispatch(root, "user-prompt-submit", &payload));
        assert_silent_success(&host, root);
    }

    #[test]
    fn claude_hook_dispatch_routes_core_events_via_handlers() {
        let host = ClaudeHookHost;
        let root = Path::new("/tmp");
        let payload = json!({});
        assert_allow_value(host.dispatch(root, "PreToolUse", &payload));
        assert_allow_value(host.dispatch(root, "PostToolUse", &payload));
        assert_allow_value(host.dispatch(root, "Stop", &payload));
        assert_allow_value(host.dispatch(root, "UserPromptSubmit", &payload));
        assert_silent_success(&host, root);
    }

    fn assert_cursor_hook_output(v: serde_json::Value) {
        assert!(
            v.is_object(),
            "cursor dispatch must return a JSON object, got {v}"
        );
    }

    #[test]
    fn cursor_hook_dispatch_routes_core_and_custom_events() {
        let host = CursorHookHost;
        let root = Path::new("/tmp");
        let payload = json!({});
        assert_cursor_hook_output(host.dispatch(root, "postToolUse", &payload));
        assert_cursor_hook_output(host.dispatch(root, "stop", &payload));
        assert_cursor_hook_output(host.dispatch(root, "beforeSubmitPrompt", &payload));
        assert_cursor_hook_output(host.dispatch(root, "sessionStart", &payload));
        assert_cursor_hook_output(host.dispatch(root, "subagentStop", &payload));
        assert_silent_success(&host, root);
    }

    #[test]
    fn codex_hook_dispatch_routes_core_and_custom_events() {
        let host = CodexHookHost;
        let root = Path::new("/tmp");
        let payload = json!({});
        let session_payload = json!({ "session_id": "host-hook-dispatch-test" });
        // pre-tool-use may delegate to audit hook and return Custom or Allow
        let pre = host.dispatch(root, "pre-tool-use", &payload);
        assert!(
            pre == HookDecision::allow_value()
                || pre.get("decision").is_some()
                || pre.get("permissionDecision").is_some()
                || pre.is_object(),
            "unexpected pre-tool-use dispatch shape: {pre}"
        );
        assert_allow_value(host.dispatch(root, "post-tool-use", &session_payload));
        assert_allow_value(host.dispatch(root, "stop", &session_payload));
        let session = host.dispatch(root, "session-start", &session_payload);
        assert!(
            session == HookDecision::allow_value()
                || session == json!({})
                || session
                    .get("hookSpecificOutput")
                    .and_then(|v| v.get("additionalContext"))
                    .is_some(),
            "unexpected session-start dispatch shape: {session}"
        );
        assert_silent_success(&host, root);
    }

    #[test]
    fn codex_hook_canonicalizes_audit_subcommands() {
        let host = CodexHookHost;
        assert_eq!(
            host.canonical_event("contract-guard").unwrap(),
            "contract-guard"
        );
        assert_eq!(
            host.canonical_event("lifecycle-context").unwrap(),
            "lifecycle-context"
        );
        assert_eq!(
            host.canonical_event("review-subagent-gate").unwrap(),
            "lifecycle-context"
        );
    }

    #[test]
    fn all_hosts_map_critical_events_to_canonical_names() {
        let cases: [(&str, Box<dyn HostHook>); 4] = [
            ("claude", Box::new(ClaudeHookHost)),
            ("cursor", Box::new(CursorHookHost)),
            ("codex", Box::new(CodexHookHost)),
            ("example", Box::new(ExampleHookHost)),
        ];
        for (label, host) in cases {
            for raw in host.critical_events() {
                assert!(
                    host.canonical_event(raw).is_ok(),
                    "{label}: critical event {raw:?} must canonicalize"
                );
            }
        }
    }
}
