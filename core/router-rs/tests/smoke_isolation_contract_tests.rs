//! subagent context + filesystem isolation contracts.

use std::fs;
use std::path::Path;

use core_policy::hook_policy::{evaluate_hook_policy, HookPolicyEvaluateRequest};
use core_policy::review_gate_engine::{
    fork_context_from_values, independent_context_fork, review_independent_reviewer_evidence,
};
use serde_json::json;

/// `fork_context: false` means independent reviewer/subagent context across Cursor/Codex/Claude spellings.
#[test]
fn subagent_context_isolation_smoke() {
    assert!(independent_context_fork(Some(false)));
    assert!(!independent_context_fork(Some(true)));
    assert!(!independent_context_fork(None));

    let isolated_inputs = [
        json!({"subagent_type": "general-purpose", "fork_context": false}),
        json!({"subagent_type": "explore", "fork_context": false}),
        json!({"agentType": "explore", "fork_context": false}),
        json!({"agent_type": "general-purpose", "fork_context": false}),
        json!({"forkContext": false}),
    ];
    for tool_input in &isolated_inputs {
        let fork = fork_context_from_values(tool_input, None);
        assert_eq!(fork, Some(false), "tool_input={tool_input}");
        assert!(
            review_independent_reviewer_evidence(true, fork),
            "reviewer lane with fork_context=false must be independent: {tool_input}"
        );
        assert!(
            !review_independent_reviewer_evidence(false, fork),
            "non-reviewer lane must not count as independent evidence: {tool_input}"
        );
    }

    let shared_inputs = [
        json!({"subagent_type": "general-purpose", "fork_context": true}),
        json!({"agent_type": "general-purpose", "fork_context": true}),
        json!({"fork_context": true}),
    ];
    for tool_input in &shared_inputs {
        let fork = fork_context_from_values(tool_input, None);
        assert_eq!(fork, Some(true), "tool_input={tool_input}");
        assert!(
            !review_independent_reviewer_evidence(true, fork),
            "fork_context=true shares main context and must not satisfy isolation: {tool_input}"
        );
    }

    let event_root_fork = json!({"fork_context": false});
    let tool_input = json!({"subagent_type": "general-purpose"});
    let fork = fork_context_from_values(&tool_input, Some(&event_root_fork));
    assert_eq!(fork, Some(false));
    assert!(review_independent_reviewer_evidence(true, fork));
}

/// Shared `path_guard` + hook `protected-path` block traversal escapes and generated host writes.
#[test]
fn subagent_filesystem_isolation_smoke() {
    let root = std::env::temp_dir().join("subagent-fs-isolation-root");
    assert!(
        crate::path_guard::join_repo_relative_under_root(&root, "../etc/passwd").is_err(),
        "repo-relative join must reject parent-dir escape"
    );
    assert!(
        crate::path_guard::join_repo_relative_under_root(&root, "artifacts/../../outside")
            .is_err(),
        "repo-relative join must reject embedded traversal"
    );
    let traversal_write = Path::new("artifacts/current/../outside/GOAL_STATE.json");
    assert!(
        crate::path_guard::reject_unsafe_path(traversal_write).is_err(),
        "write helpers must reject .. segments"
    );
    assert!(
        crate::path_guard::safe_task_id_component("../../evil").is_none(),
        "lane/task ids must be single safe components"
    );
    assert_eq!(
        crate::path_guard::safe_task_id_component("roadmap-v5-exec"),
        Some("roadmap-v5-exec")
    );

    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("mkdir isolation root");
    let inside = root.join("src/lib.rs");
    fs::create_dir_all(inside.parent().expect("parent")).expect("mkdir src");
    fs::write(&inside, b"// smoke\n").expect("write inside");
    assert!(crate::path_guard::path_is_within_repo_root(&root, &inside));
    let outside = std::env::temp_dir().join("outside-subagent-fs-smoke.txt");
    assert!(!crate::path_guard::path_is_within_repo_root(&root, &outside));

    let protected = evaluate_hook_policy(HookPolicyEvaluateRequest {
        operation: "protected-path".to_string(),
        command: None,
        path: Some("AGENTS.md".to_string()),
        repo_root: None,
        runtime_root: None,
        tool_name: None,
        tool_args: None,
    })
    .expect("protected-path evaluate");
    assert!(
        protected.blocked && protected.protected,
        "generated host surfaces must be blocked for all hosts"
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_file(&outside);
}
