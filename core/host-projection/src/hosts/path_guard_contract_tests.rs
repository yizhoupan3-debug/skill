//! J4–J8 cross-host path-guard integration / contract tests.

#[cfg(test)]
mod tests {
    use super::super::codex_hook_host::CodexHookHost;
    use super::super::host_hook::HostHook;
    use router_rs::hosts::cursor_hooks::dispatch_cursor_hook_event;
    use router_rs::hook_common::path_guard::{
        classify_protected_path, pre_tool_protected_path_deny_reason, PathGuardContext,
    };
    use router_rs::mcp_pre_guard::evaluate_mcp_pre_guard_safe;
    use serde_json::json;
    use std::env;
    use std::path::PathBuf;

    fn framework_repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    struct LegacySubtractedEventsGuard {
        _lock: router_rs::test_env_sync::ProcessEnvLockGuard,
        prev: Option<std::ffi::OsString>,
    }

    impl LegacySubtractedEventsGuard {
        fn enable() -> Self {
            let _lock = router_rs::test_env_sync::process_env_lock();
            let key = "ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS";
            let prev = env::var_os(key);
            env::set_var(key, "1");
            Self { _lock, prev }
        }
    }

    impl Drop for LegacySubtractedEventsGuard {
        fn drop(&mut self) {
            let key = "ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS";
            match self.prev.take() {
                Some(v) => env::set_var(key, v),
                None => env::remove_var(key),
            }
        }
    }

    #[test]
    fn j4_j5_codex_and_shared_pre_tool_block_agents_md_write() {
        let repo = framework_repo();
        let payload = json!({"tool_input": {"file_path": "AGENTS.md"}});

        let host = CodexHookHost;
        let out = host.dispatch(&repo, "PreToolUse", &payload);
        assert_eq!(
            out.get("decision").and_then(|v| v.as_str()),
            Some("block"),
            "Codex PreToolUse must block AGENTS.md writes: {out}"
        );
        let reason = out
            .get("reason")
            .or_else(|| out.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            reason.contains("AGENTS.md") || reason.contains("protected"),
            "block reason must cite protected path: {out}"
        );

        let deny = pre_tool_protected_path_deny_reason(&repo, &payload);
        assert!(
            deny.as_deref().is_some_and(|r| r.contains("AGENTS.md")),
            "shared pre-tool guard must block AGENTS.md: {deny:?}"
        );
    }

    #[test]
    fn j5_cursor_file_edit_and_shell_block_agents_md() {
        let _legacy = LegacySubtractedEventsGuard::enable();
        let repo = framework_repo();
        let agents = repo.join("AGENTS.md");

        let edit_out = dispatch_cursor_hook_event(
            &repo,
            "afterFileEdit",
            &json!({
                "file_path": agents,
                "prompt": ""
            }),
        );
        assert!(
            edit_out
                .get("user_message")
                .and_then(|m| m.as_str())
                .is_some_and(|m| m.contains("AGENTS.md") || m.contains("protected")),
            "Cursor afterFileEdit must block AGENTS.md: {edit_out}"
        );

        let shell_out = dispatch_cursor_hook_event(
            &repo,
            "beforeShellExecution",
            &json!({
                "command": "printf x > ./AGENTS.md",
                "cwd": repo.to_string_lossy(),
                "prompt": ""
            }),
        );
        assert_eq!(
            shell_out.get("permission").and_then(|v| v.as_str()),
            Some("deny"),
            "Cursor beforeShellExecution must deny mutating AGENTS.md: {shell_out}"
        );
        assert!(
            shell_out
                .get("user_message")
                .and_then(|m| m.as_str())
                .is_some_and(|m| m.contains("AGENTS.md")),
            "shell deny must cite AGENTS.md: {shell_out}"
        );
    }

    #[test]
    fn j7_mcp_record_evidence_pre_guard_blocks_protected_path() {
        let repo = framework_repo();
        let verdict = evaluate_mcp_pre_guard_safe(
            "record_evidence",
            &json!({
                "path": "skills/SKILL_ROUTING_RUNTIME.json",
                "summary": "attempted write"
            }),
            &repo,
        );
        assert!(
            verdict.blocked,
            "record_evidence must block protected routing JSON: {verdict:?}"
        );
        assert!(
            verdict
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("SKILL_ROUTING_RUNTIME.json")),
            "block reason must cite protected path: {verdict:?}"
        );
    }

    #[test]
    fn j6_active_skill_dir_exempts_skill_authoring_paths() {
        let repo = framework_repo();
        let allowed = evaluate_mcp_pre_guard_safe(
            "record_evidence",
            &json!({
                "path": "skills/plan-mode/references/cursor-createplan-contract.md",
                "skill_path": "skills/plan-mode/SKILL.md",
                "summary": "skill edit"
            }),
            &repo,
        );
        assert!(
            !allowed.blocked,
            "active skill dir must exempt skill authoring paths: {allowed:?}"
        );

        let blocked = evaluate_mcp_pre_guard_safe(
            "record_evidence",
            &json!({
                "path": "AGENTS.md",
                "skill_path": "skills/plan-mode/SKILL.md",
                "summary": "kernel edit"
            }),
            &repo,
        );
        assert!(
            blocked.blocked,
            "skill dir must not exempt AGENTS.md: {blocked:?}"
        );
    }

    #[test]
    fn j8_in_tree_repo_subdir_still_protects_generated_paths() {
        let framework = framework_repo()
            .canonicalize()
            .unwrap_or_else(|_| framework_repo());
        let subdir = framework.join("core/router-rs");
        let ctx = PathGuardContext::new(&subdir).with_runtime_root(Some(framework.clone()));
        assert!(
            !ctx.is_dev_checkout(),
            "in-tree subdir must not trigger dev-checkout exemption"
        );
        assert_eq!(
            classify_protected_path("AGENTS.md", Some(&subdir), Some(&framework), None),
            Some("generated_host_entrypoint"),
            "in-tree subdir must still protect AGENTS.md"
        );
    }
}
