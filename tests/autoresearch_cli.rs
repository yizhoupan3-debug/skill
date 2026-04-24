mod common;

use common::{project_root, run};
use serde_json::Value;
use std::process::{Command, Output};
use tempfile::tempdir;

#[test]
fn rust_autoresearch_full_loop() {
    let tmp = tempdir().unwrap();
    run_ctl_ok(&[
        "init",
        "--project",
        "rust-project",
        "--question",
        "Can Rust own autoresearch?",
        "--dir",
        tmp.path().to_str().unwrap(),
        "--mode",
        "quick",
    ]);
    let root = tmp.path().join("rust-project");
    let mut state = load_state(&root);
    assert_eq!(state["schema_version"], 4);
    assert_eq!(state["novelty_gate"]["status"], "pending");

    run_ctl_ok(&[
        "draft-claims",
        "--workspace",
        root.to_str().unwrap(),
        "--count",
        "4",
    ]);
    state = load_state(&root);
    assert_eq!(
        state["novelty_gate"]["draft_claims"]
            .as_sequence()
            .unwrap()
            .len(),
        4
    );
    assert!(
        std::fs::read_to_string(root.join("literature/NOVELTY_SEARCH_PLAN.md"))
            .unwrap()
            .contains("recommended first search target")
    );
    assert!(
        std::fs::read_to_string(root.join("literature/EXTERNAL_RESEARCH.md"))
            .unwrap()
            .contains("research-claim")
    );

    run_ctl_ok(&[
        "compare-claim",
        "--workspace",
        root.to_str().unwrap(),
        "--claim",
        "Rust controller reduces orchestration drift",
        "--axis",
        "workflow",
        "--closest-prior-work",
        "Python controller",
        "--overlap",
        "medium",
        "--difference",
        "Rust is the primary control plane.",
        "--confidence",
        "high",
        "--verdict",
        "defensible",
        "--claim-id",
        "C1",
    ]);
    run_ctl_ok(&[
        "set-novelty-gate",
        "--workspace",
        root.to_str().unwrap(),
        "--status",
        "passed",
        "--decision",
        "Proceed",
    ]);
    run_ctl_ok(&[
        "add-hypothesis",
        "--workspace",
        root.to_str().unwrap(),
        "--claim",
        "Rust controller can run the loop",
        "--prediction",
        "Run and reflection files are materialized",
        "--priority",
        "high",
        "--id",
        "rust-loop",
    ]);
    run_ctl_ok(&[
        "record-run",
        "--workspace",
        root.to_str().unwrap(),
        "--hypothesis-id",
        "rust-loop",
        "--outcome",
        "exploratory",
        "--summary",
        "Rust recorded the run.",
        "--metric-name",
        "files",
        "--metric-value",
        "2",
    ]);
    run_ctl_ok(&[
        "reflect",
        "--workspace",
        root.to_str().unwrap(),
        "--hypothesis-id",
        "rust-loop",
        "--direction",
        "DEEPEN",
        "--reason",
        "The loop is working.",
        "--next-step",
        "Add broader coverage.",
    ]);
    state = load_state(&root);
    assert_eq!(state["active_hypothesis"], "rust-loop");
    assert_eq!(state["hypotheses"][0]["status"], "active");
    assert!(root.join("experiments/rust-loop/run-001.md").is_file());
    assert!(root
        .join("experiments/rust-loop/run-001-reflection.md")
        .is_file());
    let resume = run_ctl_ok(&["resume", "--workspace", root.to_str().unwrap()]);
    assert!(stdout(&resume).contains("latest_direction: DEEPEN"));
    let ledger = std::fs::read_to_string(root.join("run-ledger.jsonl")).unwrap();
    assert!(ledger
        .lines()
        .any(|line| serde_json::from_str::<Value>(line).unwrap()["kind"] == "run.recorded"));
}

#[test]
fn autoresearch_records_external_research_from_arxiv() {
    let tmp = tempdir().unwrap();
    run_ctl_ok(&[
        "init",
        "--project",
        "external-research",
        "--question",
        "Can retrieval augmented generation improve citation grounded research?",
        "--dir",
        tmp.path().to_str().unwrap(),
    ]);
    let root = tmp.path().join("external-research");
    run_ctl_ok(&[
        "draft-claims",
        "--workspace",
        root.to_str().unwrap(),
        "--count",
        "2",
    ]);
    run_ctl_ok(&[
        "research-claim",
        "--workspace",
        root.to_str().unwrap(),
        "--claim-id",
        "C1",
        "--source",
        "arxiv",
        "--limit",
        "1",
    ]);
    let state = load_state(&root);
    assert_eq!(state["external_research"].as_sequence().unwrap().len(), 1);
    assert!(state["external_research"][0]["query"].as_str().is_some());
    assert!(!state["external_research"][0]["results"]
        .as_sequence()
        .unwrap()
        .is_empty());
    assert!(
        std::fs::read_to_string(root.join("literature/EXTERNAL_RESEARCH.md"))
            .unwrap()
            .contains("Managed External Research")
    );
}

#[test]
fn autoresearch_batch_research_and_gate_recommendation() {
    let tmp = tempdir().unwrap();
    run_ctl_ok(&[
        "init",
        "--project",
        "batch-research",
        "--question",
        "Can retrieval augmented generation improve citation grounded research?",
        "--dir",
        tmp.path().to_str().unwrap(),
    ]);
    let root = tmp.path().join("batch-research");
    run_ctl_ok(&[
        "draft-claims",
        "--workspace",
        root.to_str().unwrap(),
        "--count",
        "2",
    ]);
    run_ctl_ok(&[
        "research-all",
        "--workspace",
        root.to_str().unwrap(),
        "--source",
        "arxiv",
        "--limit",
        "1",
        "--max-claims",
        "2",
    ]);
    let state = load_state(&root);
    assert_eq!(state["external_research"].as_sequence().unwrap().len(), 2);
    let recommendation = run_ctl_ok(&["gate-from-research", "--workspace", root.to_str().unwrap()]);
    assert!(stdout(&recommendation).contains("recommended_status: pending"));
    assert!(stdout(&recommendation).contains("reviewed_claims:"));

    let first_claim = state["novelty_gate"]["draft_claims"][0]["claim"]
        .as_str()
        .unwrap()
        .to_string();
    let second_claim = state["novelty_gate"]["draft_claims"][1]["claim"]
        .as_str()
        .unwrap()
        .to_string();
    compare_claim(&root, &first_claim, "method", "C1");
    compare_claim(&root, &second_claim, "task", "C2");

    let applied = run_ctl_ok(&[
        "gate-from-research",
        "--workspace",
        root.to_str().unwrap(),
        "--apply",
    ]);
    assert!(stdout(&applied).contains("recommended_status: passed"));
    let state = load_state(&root);
    assert_eq!(state["novelty_gate"]["status"], "passed");
}

fn compare_claim(root: &std::path::Path, claim: &str, axis: &str, claim_id: &str) {
    run_ctl_ok(&[
        "compare-claim",
        "--workspace",
        root.to_str().unwrap(),
        "--claim",
        claim,
        "--axis",
        axis,
        "--closest-prior-work",
        "Nearest retrieved arXiv result",
        "--overlap",
        "medium",
        "--difference",
        "The scoped contribution is narrower.",
        "--confidence",
        "medium",
        "--verdict",
        "defensible",
        "--claim-id",
        claim_id,
    ]);
}

fn run_ctl_ok(args: &[&str]) -> Output {
    let output = run_ctl(args);
    common::assert_success(&output);
    output
}

fn run_ctl(args: &[&str]) -> Output {
    let mut command = Command::new("cargo");
    command
        .args(["run", "--quiet", "--manifest-path"])
        .arg(project_root().join("scripts/autoresearch-rs/Cargo.toml"))
        .arg("--")
        .args(args)
        .current_dir(project_root());
    run(command)
}

fn load_state(workspace: &std::path::Path) -> serde_yaml::Value {
    serde_yaml::from_str(&std::fs::read_to_string(workspace.join("research-state.yaml")).unwrap())
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}
