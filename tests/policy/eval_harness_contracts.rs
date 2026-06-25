use crate::common::{project_root, read_json, router_rs_json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use tempfile::tempdir;

#[test]
fn closeout_record_schema_is_published() {
    let path = project_root().join("configs/framework/CLOSEOUT_RECORD_SCHEMA.json");
    assert!(
        path.exists(),
        "expected closeout record schema at {}",
        path.display()
    );
    let schema = read_json(&path);
    assert_eq!(schema["schema_version"], "closeout-record-v1");
    let required = schema["required_fields"]
        .as_array()
        .expect("required_fields array");
    for expected in [
        "schema_version",
        "task_id",
        "verification_status",
        "summary",
    ] {
        assert!(
            required.iter().any(|v| v == expected),
            "closeout schema missing required field: {expected}"
        );
    }
    let rules = schema["enforcement_rules"]
        .as_array()
        .expect("enforcement_rules array");
    let schema_rules = rules
        .iter()
        .map(|rule| rule["id"].as_str().expect("rule id").to_string())
        .collect::<BTreeSet<_>>();
    let contract = router_rs_json(&["closeout", "contract"]);
    let contract_rules = contract["rules"]
        .as_array()
        .expect("contract rules")
        .iter()
        .map(|rule| rule.as_str().expect("contract rule id").to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        schema_rules, contract_rules,
        "CLOSEOUT_RECORD_SCHEMA.enforcement_rules must stay aligned with router-rs closeout contract"
    );
}

#[test]
fn closeout_evaluate_blocks_unverified_completion_via_cli() {
    let payload = serde_json::json!({
        "schema_version": "closeout-record-v1",
        "task_id": "policy-contract-1",
        "summary": "已完成 deck rebuild",
        "verification_status": "not_run",
    });
    let response = router_rs_json(&["closeout", "evaluate", "--input-json", &payload.to_string()]);
    assert_eq!(response["closeout_allowed"], false);
    let violations = response["violations"].as_array().expect("violations array");
    assert!(
        violations
            .iter()
            .any(|v| v["rule"] == "claimed_done_without_evidence")
    );
}

#[test]
fn closeout_evaluate_allows_clean_record_via_cli() {
    let payload = serde_json::json!({
        "schema_version": "closeout-record-v1",
        "task_id": "policy-contract-2",
        "summary": "Refactored builder; not yet executed",
        "verification_status": "partial",
        "changed_files": ["ppt/build_deck.py"],
        "risks": ["did not run python build_deck.py because PIL missing"]
    });
    let response = router_rs_json(&["closeout", "evaluate", "--input-json", &payload.to_string()]);
    assert_eq!(response["closeout_allowed"], true, "got {response:#?}");
    assert_eq!(response["claimed_completion"], false);
}

#[test]
fn closeout_evaluate_uses_task_evidence_context_via_cli() {
    let tmp = tempdir().unwrap();
    let repo = tmp.path();
    let task_id = "policy-context-closeout";
    let record_dir = repo.join("artifacts/closeout");
    fs::create_dir_all(&record_dir).unwrap();
    let record_path = record_dir.join(format!("{task_id}.json"));
    let payload = serde_json::json!({
        "schema_version": "closeout-record-v1",
        "task_id": task_id,
        "summary": "tests passed and task completed",
        "verification_status": "passed",
        "artifacts_checked": [{"path": "target/debug/app", "exists": true}]
    });
    fs::write(&record_path, serde_json::to_string(&payload).unwrap()).unwrap();
    let response = router_rs_json(&[
        "closeout",
        "evaluate",
        "--repo-root",
        repo.to_str().unwrap(),
        "--task-id",
        task_id,
        "--record-path",
        record_path.to_str().unwrap(),
    ]);
    assert_eq!(response["closeout_allowed"], false, "got {response:#?}");
    assert!(
        response["violations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["rule"] == "claimed_passed_without_evidence_index_rows")
    );
}

#[test]
fn closeout_contract_command_lists_rules() {
    let response = router_rs_json(&["closeout", "contract"]);
    assert_eq!(
        response["record_schema_version"], "closeout-record-v1",
        "got {response:#?}"
    );
    let rules = response["rules"].as_array().expect("rules array");
    assert!(
        rules
            .iter()
            .any(|v| v == "verification_passed_with_missing_artifact")
    );
}

#[test]
fn eval_route_cli_reports_metrics() {
    let cases_path = project_root().join("tests/routing_eval_cases.json");
    let cases_json = read_json(&cases_path);
    let expected_total = cases_json["cases"]
        .as_array()
        .expect("routing eval cases array")
        .len();
    let response = router_rs_json(&["eval", "route", "--cases", &cases_path.to_string_lossy()]);
    assert_eq!(
        response["total_cases"].as_u64().expect("total_cases") as usize,
        expected_total
    );
    // Routing regression gate: route_accuracy must be >= 0.95 across all eval cases.
    let route_accuracy = response["route_accuracy"]
        .as_f64()
        .expect("route_accuracy field");
    assert!(
        route_accuracy >= 0.95,
        "Routing regression detected: route_accuracy {:.4} < 0.95 threshold \
         ({} passed, {} failed out of {} total). \
         Fix the failing cases in tests/routing_eval_cases.json before merging.",
        route_accuracy,
        response["passed"].as_u64().unwrap_or(0),
        response["failed"].as_u64().unwrap_or(0),
        expected_total,
    );
    assert!(response["passed"].as_u64().unwrap() > 0);
    // overtrigger must be zero (false positives are worse than false negatives)
    let wrong_owner_rate = response["wrong_owner_rate"].as_f64().unwrap_or(0.0);
    assert!(
        wrong_owner_rate < 0.05,
        "Wrong-owner rate {:.4} exceeds 5% tolerance (threshold 0.05).",
        wrong_owner_rate,
    );
    // Per-case owner_correct: every eval case with expected_owner must route
    // to the expected skill. This catches manifest-only slugs that were
    // previously missing from the runtime index.
    let failures = response["failures"].as_array().expect("failures array");
    let owner_failures: Vec<&serde_json::Value> = failures
        .iter()
        .filter(|f| f["field"].as_str() == Some("selected_skill"))
        .collect();
    assert!(
        owner_failures.is_empty(),
        "Per-case owner mismatch detected ({} case(s)): {}",
        owner_failures.len(),
        owner_failures
            .iter()
            .map(|f| format!(
                "\n  case={}: expected={} got={}",
                f["case_id"].as_str().unwrap_or("?"),
                f["expected"].as_str().unwrap_or("?"),
                f["got"].as_str().unwrap_or("?"),
            ))
            .collect::<String>(),
    );
}

#[test]
fn eval_route_contract_cli_lists_metrics() {
    let response = router_rs_json(&["eval", "route-contract"]);
    assert_eq!(
        response["schema_version"], "routing-eval-report-v1",
        "got {response:#?}"
    );
    let metrics = response["metrics"].as_array().expect("metrics array");
    assert!(metrics.iter().any(|v| v == "route_accuracy"));
    assert!(metrics.iter().any(|v| v == "wrong_owner_rate"));
}

#[test]
fn harness_failure_taxonomy_config_matches_cli_contract() {
    let config = read_json(&project_root().join("configs/framework/HARNESS_FAILURE_TAXONOMY.json"));
    assert_eq!(config["schema_version"], "harness-failure-taxonomy-v1");
    let config_classes = config["classes"]
        .as_array()
        .expect("classes array")
        .iter()
        .map(|v| {
            (
                v["id"].as_str().expect("class id").to_string(),
                v["description"]
                    .as_str()
                    .expect("class description")
                    .to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let response = router_rs_json(&["eval", "harness-contract"]);
    let contract_classes = response["failure_taxonomy"]
        .as_array()
        .expect("failure taxonomy array")
        .iter()
        .map(|v| {
            (
                v["id"].as_str().expect("taxonomy id").to_string(),
                v["description"]
                    .as_str()
                    .expect("taxonomy description")
                    .to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(config_classes, contract_classes);
    for expected in [
        "route_miss",
        "verification_missing",
        "subagent_misuse",
        "trace_gap",
        "step_recovery_gap",
    ] {
        assert!(
            contract_classes.contains_key(expected),
            "missing {expected}"
        );
    }
}

#[test]
fn harness_behavioral_eval_cases_cover_required_tracks() {
    let config =
        read_json(&project_root().join("configs/framework/HARNESS_BEHAVIORAL_EVAL_CASES.json"));
    assert_eq!(config["schema_version"], "harness-behavioral-eval-cases-v1");
    let tracks = config["tracks"]
        .as_array()
        .expect("tracks array")
        .iter()
        .map(|v| v["id"].as_str().expect("track id").to_string())
        .collect::<BTreeSet<_>>();
    let cases = config["cases"].as_array().expect("cases array");
    let case_ids = cases
        .iter()
        .map(|v| v["id"].as_str().expect("case id").to_string())
        .collect::<BTreeSet<_>>();
    let taxonomy_ids = read_json(
        &project_root().join("configs/framework/HARNESS_FAILURE_TAXONOMY.json"),
    )["classes"]
        .as_array()
        .expect("taxonomy classes")
        .iter()
        .map(|v| v["id"].as_str().expect("failure class id").to_string())
        .collect::<BTreeSet<_>>();
    let response = router_rs_json(&["eval", "harness-contract"]);
    let contract_tracks = response["behavioral_eval_tracks"]
        .as_array()
        .expect("contract tracks")
        .iter()
        .map(|v| v.as_str().expect("track").to_string())
        .collect::<BTreeSet<_>>();
    assert!(contract_tracks.is_subset(&tracks));
    for expected in [
        "routing_accuracy",
        "token_efficiency",
        "long_task_continuity",
        "trajectory_health",
        "closeout_integrity",
        "skill_contract_quality",
        "subagent_lane_integrity",
        "review_gate_integrity",
        "contract_integrity",
    ] {
        assert!(tracks.contains(expected), "missing track {expected}");
    }
    for track in config["tracks"].as_array().expect("tracks array") {
        for case_id in track["case_ids"].as_array().expect("case_ids") {
            let case_id = case_id.as_str().expect("case id");
            assert!(
                case_ids.contains(case_id),
                "track {} references missing case {case_id}",
                track["id"].as_str().unwrap_or("<unknown>")
            );
        }
    }
    for case in cases {
        let failure_class = case["failure_class"].as_str().expect("failure_class");
        assert!(
            taxonomy_ids.contains(failure_class),
            "case {} uses unknown failure_class {failure_class}",
            case["id"].as_str().unwrap_or("<unknown>")
        );
        assert!(
            case["verify"]
                .as_str()
                .unwrap_or_default()
                .contains("cargo ")
                || case["verify"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("router-rs "),
            "case {} must name an executable verification command",
            case["id"].as_str().unwrap_or("<unknown>")
        );
    }
}

#[test]
fn harness_skill_contract_lint_cli_reports_protocol_shape() {
    let payload = serde_json::json!({
        "skills_root": project_root().join("skills").to_string_lossy(),
        "slugs": ["skill-framework-developer", "agent-swarm-orchestration", "research-discovery", "gh-fix-ci"]
    });
    let response = router_rs_json(&[
        "eval",
        "skill-contract-lint",
        "--input-json",
        &payload.to_string(),
    ]);
    assert_eq!(
        response["schema_version"],
        "router-rs-harness-skill-contract-lint-v1"
    );
    assert_eq!(
        response["skills_scanned"]
            .as_array()
            .expect("skills scanned")
            .len(),
        4
    );
    assert!(response["findings"].is_array());
    assert!(response["execution_items"].is_array());
    assert!(response["verification_results"].is_array());
    assert!(
        response["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .all(|finding| finding["severity"] != "major"),
        "default high-impact lint must not report major findings: {response:#?}"
    );
    assert_eq!(
        response["verification_results"][0]["status"], "pass",
        "default high-impact lint must be a gate, not shape-only: {response:#?}"
    );
}

#[test]
fn framework_step_ledger_append_projects_summary_into_task_state() {
    let tmp = tempdir().unwrap();
    let repo = tmp.path();
    let payload = serde_json::json!({
        "operation": "append",
        "repo_root": repo.to_string_lossy(),
        "task_id": "step-ledger-policy",
        "step_id": "plan-1",
        "phase": "implementation",
        "status": "pass",
        "input_text": "implement harness plan",
        "retry_count": 0,
        "side_effects": [],
        "evidence_ref": {"kind":"manual","label":"unit-test"},
        "next_resume_hint": "continue at verify"
    });
    let response = router_rs_json(&[
        "framework",
        "step-ledger",
        "--input-json",
        &payload.to_string(),
    ]);
    assert_eq!(
        response["schema_version"],
        "router-rs-step-ledger-response-v1"
    );
    let summary_payload = serde_json::json!({
        "operation": "summary",
        "repo_root": repo.to_string_lossy(),
        "task_id": "step-ledger-policy"
    });
    let summary = router_rs_json(&[
        "framework",
        "step-ledger",
        "--input-json",
        &summary_payload.to_string(),
    ]);
    assert_eq!(summary["entry_count"], 1);
    assert_eq!(summary["latest"]["step_id"], "plan-1");
    let task_state = read_json(
        &repo
            .join("artifacts/current/step-ledger-policy")
            .join("TASK_STATE.json"),
    );
    assert_eq!(task_state["step_ledger"]["entry_count"], 1);
    assert_eq!(
        task_state["step_ledger"]["latest"]["next_resume_hint"],
        "continue at verify"
    );
}
