//! MCP stdio hosts (Opencode): closeout + review_gate advisory contract slice.
//!
//! Complements the Cursor/Codex/Claude matrix in `mod.rs`; uses shared MCP harness helpers.

use serde_json::json;
use std::path::PathBuf;

use crate::mcp_stdio_harness;
use crate::mcp_stdio_test_support;

const MCP_HOSTS: &[&str] = &["opencode"];

fn fresh_mcp_repo(host: &str, label: &str) -> PathBuf {
    let path = mcp_stdio_test_support::unique_temp_repo(&format!("hook-contract-{host}-{label}"));
    mcp_stdio_test_support::seed_minimal_current_task_layout(&path);
    path
}

fn call_closeout_gate(repo: &PathBuf, host: &str, args: &serde_json::Value) -> String {
    mcp_stdio_harness::tool_closeout_gate(args, repo, host).expect("closeout_gate")
}

fn call_review_gate_prompt(repo: &PathBuf, host: &str) -> String {
    let response = mcp_stdio_harness::handle_mcp_request(
        r#"{"jsonrpc":"2.0","id":1,"method":"prompts/get","params":{"name":"review_gate"}}"#,
        repo,
        host,
        "test-session",
    )
    .expect("prompts/get review_gate");
    response["result"]["messages"][0]["content"]["text"]
        .as_str()
        .expect("review_gate prompt text")
        .to_string()
}

/// Default seeded layout lacks successful evidence → advisory unsatisfied (not hard-block error).
#[test]
fn mcp_closeout_gate_advisory_unsatisfied_by_default() {
    for host in MCP_HOSTS {
        let repo = fresh_mcp_repo(host, "closeout-default");
        let out = call_closeout_gate(&repo, host, &json!({}));
        assert!(
            out.contains("ADVISORY"),
            "{host} closeout_gate must be advisory when unsatisfied; got {out}"
        );
        assert!(
            out.contains("no hook REVIEW_GATE"),
            "{host} must document honor-system review_gate; got {out}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }
}

/// Review goal without reviewer evidence → WARN advisory on both MCP hosts.
#[test]
fn mcp_closeout_gate_review_goal_without_evidence_advisory() {
    for host in MCP_HOSTS {
        let repo = fresh_mcp_repo(host, "review-warn");
        let task_id = "test-task";
        let task_dir = repo.join("artifacts/current").join(task_id);
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","status":"running","goal":"深度 review 这个 PR"}"#,
        )
        .unwrap();

        let out = call_closeout_gate(&repo, host, &json!({"task_id": task_id}));
        assert!(
            out.contains("WARN: review_gate: GOAL suggests review work"),
            "{host} must WARN when review goal lacks evidence; got {out}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }
}

/// `review_gate` prompt surfaces registry reviewer lanes + fork_context=false on both hosts.
#[test]
fn mcp_review_gate_prompt_lists_reviewer_lanes() {
    for host in MCP_HOSTS {
        let repo = fresh_mcp_repo(host, "review-prompt");
        let text = call_review_gate_prompt(&repo, host);
        assert!(
            text.contains("reviewer_lanes"),
            "{host} review_gate prompt must mention reviewer_lanes; got {text}"
        );
        assert!(
            text.contains("fork_context=false"),
            "{host} review_gate prompt must mention fork_context=false; got {text}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }
}

/// On-disk review-lanes/*.md clears review WARN while keeping advisory closeout shape.
#[test]
fn mcp_closeout_gate_review_lanes_clear_review_warn() {
    for host in MCP_HOSTS {
        let repo = fresh_mcp_repo(host, "review-lanes");
        let task_id = "test-task";
        let task_dir = repo.join("artifacts/current").join(task_id);
        let review_lanes = task_dir.join("review-lanes");
        std::fs::create_dir_all(&review_lanes).unwrap();
        std::fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","status":"running","goal":"深度 review 这个 PR"}"#,
        )
        .unwrap();
        std::fs::write(review_lanes.join("lane-a.md"), "[P2] example — ok").unwrap();

        let out = call_closeout_gate(&repo, host, &json!({"task_id": task_id}));
        assert!(
            !out.contains("WARN: review_gate: GOAL suggests review work"),
            "{host} review-lanes should clear review WARN; got {out}"
        );
        assert!(
            out.contains("reviewer evidence attested"),
            "{host} must acknowledge review-lanes evidence; got {out}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }
}
