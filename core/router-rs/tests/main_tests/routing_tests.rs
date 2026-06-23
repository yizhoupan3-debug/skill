use super::common::*;
use super::*;

use serde_json::{json, Value};


#[test]
fn stdio_request_dispatches_route_policy_payload() {
    let response =
        handle_stdio_json_line(r#"{"id":1,"op":"route_policy","payload":{"mode":"verify"}}"#);
    assert!(response.ok);
    assert_eq!(response.id, json!(1));
    assert_eq!(
        response.payload.expect("payload")["policy_schema_version"],
        json!(ROUTE_POLICY_SCHEMA_VERSION)
    );
}


#[test]
fn stdio_request_dispatches_hook_policy_payload() {
    let response = handle_stdio_json_line(
        r#"{"id":1,"op":"hook_policy","payload":{"operation":"validation-categories","command":"python3 -m json.tool .codex/config.toml"}}"#,
    );
    assert!(response.ok, "{}", response.error.unwrap_or_default());
    let payload = response.payload.expect("payload");
    assert_eq!(payload["categories"], json!(["config", "json"]));
}


#[test]
fn stdio_request_dispatches_concurrency_defaults_payload() {
    let response = handle_stdio_json_line(r#"{"id":1,"op":"concurrency_defaults","payload":{}}"#);
    assert!(response.ok);
    let payload = response.payload.expect("payload");
    assert_eq!(
        payload["router_stdio"]["default_pool_size"],
        json!(DEFAULT_ROUTER_STDIO_POOL_SIZE)
    );
    assert_eq!(
        payload["router_stdio"]["max_pool_size"],
        json!(MAX_ROUTER_STDIO_POOL_SIZE)
    );
    assert_eq!(
        payload["max_background_jobs"],
        json!(DEFAULT_MAX_BACKGROUND_JOBS)
    );
    assert_eq!(
        payload["max_concurrent_subagents"],
        json!(DEFAULT_MAX_CONCURRENT_SUBAGENTS)
    );
}


#[test]
fn stdio_request_dispatches_execute_payload() {
    let payload =
        serde_json::to_string(&sample_execute_request()).expect("serialize execute payload");
    let response = handle_stdio_json_line(&format!(
        "{{\"id\":3,\"op\":\"execute\",\"payload\":{payload}}}"
    ));
    assert!(response.ok);
    assert_eq!(response.id, json!(3));
    let payload = response.payload.expect("payload");
    assert_eq!(
        payload["execution_schema_version"],
        json!(EXECUTION_SCHEMA_VERSION)
    );
    assert_eq!(payload["authority"], json!(EXECUTION_AUTHORITY));
    assert_eq!(payload["live_run"], json!(false));
}


#[test]
fn stdio_request_rejects_unknown_operations() {
    let response = handle_stdio_json_line(r#"{"id":"req-1","op":"not-supported","payload":{}}"#);
    assert!(!response.ok);
    assert_eq!(response.id, json!("req-1"));
    assert!(response
        .error
        .expect("error")
        .contains("unsupported stdio operation"));
}


#[test]
fn stdio_dispatch_domain_classification_covers_known_ops() {
    assert!(is_routing_stdio_op("route_report"));
    assert!(is_routing_stdio_op("pre_tool_use_guard"));
    assert!(is_runtime_stdio_op("runtime_storage"));
    assert!(is_trace_stdio_op("trace_stream_replay"));
    assert!(is_framework_stdio_op("framework_prompt_compression"));
    assert!(is_framework_stdio_op("framework_hook_evidence_append"));
    assert!(is_framework_stdio_op("framework_goal_drive"));
    assert!(is_framework_stdio_op("framework_quality_gate"));
    assert!(!is_routing_stdio_op("framework_prompt_compression"));
    assert!(!is_runtime_stdio_op("trace_record_event"));
    assert!(matches!(
        classify_stdio_op("route_report"),
        Some(StdioOpDomain::Routing)
    ));
    assert!(matches!(
        classify_stdio_op("runtime_storage"),
        Some(StdioOpDomain::Runtime)
    ));
    assert!(matches!(
        classify_stdio_op("trace_stream_replay"),
        Some(StdioOpDomain::Trace)
    ));
    assert!(matches!(
        classify_stdio_op("framework_prompt_compression"),
        Some(StdioOpDomain::Framework)
    ));
    assert!(matches!(
        classify_stdio_op("framework_goal_drive"),
        Some(StdioOpDomain::Framework)
    ));
    assert!(matches!(
        classify_stdio_op("framework_quality_gate"),
        Some(StdioOpDomain::Framework)
    ));
}


#[test]
fn stdio_request_routes_common_ops_to_expected_domains() {
    let routing = dispatch_stdio_json_request("concurrency_defaults", json!({}))
        .expect("routing op should resolve");
    assert!(routing.get("router_stdio").is_some());

    let runtime_error = dispatch_stdio_json_request("runtime_storage", json!({}))
        .expect_err("runtime op should parse runtime storage payload");
    assert!(runtime_error.contains("parse runtime storage input failed"));

    let trace_error = dispatch_stdio_json_request("trace_compact", json!({}))
        .expect_err("trace op should parse trace compact payload");
    assert!(trace_error.contains("parse trace compact input failed"));

    let framework_error = dispatch_stdio_json_request("framework_runtime_snapshot", json!({}))
        .expect_err("framework op should require repo_root");
    assert!(framework_error.contains("repo_root"));
}


#[test]
fn stdio_request_dispatches_route_snapshot_payload() {
    let response = handle_stdio_json_line(
        r#"{"id":2,"op":"route_snapshot","payload":{"engine":"rust","selected_skill":"router","overlay_skill":null,"layer":"L2","score":42.0,"reasons":["matched"],"matched_token_count":1}}"#,
    );
    assert!(response.ok);
    assert_eq!(response.id, json!(2));
    let payload = response.payload.expect("payload");
    assert_eq!(
        payload["snapshot_schema_version"],
        json!(ROUTE_SNAPSHOT_SCHEMA_VERSION)
    );
    assert_eq!(payload["route_snapshot"]["selected_skill"], json!("router"));
}


#[test]
fn stdio_route_supports_inline_skill_catalog_and_token_budget_bias() {
    let response = handle_stdio_json_line(
        r#"{"id":4,"op":"route","payload":{"query":"这是多阶段任务，但只要 bounded sidecar，保留主线程集成，降低 token 开销，不要完整 worker 编排","session_id":"inline-route","allow_overlay":true,"first_turn":true,"skills":[{"name":"agent-swarm-orchestration","description":"Decide whether work should stay local, use bounded sidecars, or fall back to a local supervisor queue.","routing_layer":"L0","routing_owner":"gate","routing_gate":"delegation","routing_priority":"P1","trigger_hints":["subagent","sidecar","delegation"]},{"name":"deepinterview","description":"Evidence-first clarification and convergence review.","routing_layer":"L1","routing_owner":"owner","routing_gate":"none","routing_priority":"P1","trigger_hints":["deepinterview","review"]}]}}"#,
    );
    assert!(response.ok, "{:?}", response.error);
    let payload = response.payload.expect("payload");
    assert_eq!(
        payload["selected_skill"],
        json!("agent-swarm-orchestration")
    );
    assert_eq!(payload["overlay_skill"], Value::Null);
    let reasons = payload["reasons"]
        .as_array()
        .expect("route reasons array")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(reasons
        .iter()
        .any(|reason| reason.contains("Token-budget boost applied")));
}


#[test]
fn stdio_route_cache_reuses_records_until_runtime_changes() {
    let runtime_path = temp_json_path("routing-runtime");
    let manifest_path = temp_json_path("routing-manifest");
    write_runtime_fixture(&runtime_path, "alpha");
    write_manifest_fixture(&manifest_path, "alpha", "P1");

    let first = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))
        .expect("first cache load");
    let second = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))
        .expect("second cache load");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first[0].slug, "alpha");

    sleep(Duration::from_millis(20));
    write_runtime_fixture(&runtime_path, "beta");

    let third = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))
        .expect("reload after runtime change");
    assert!(!Arc::ptr_eq(&second, &third));
    assert_eq!(third[0].slug, "beta");

    let _ = fs::remove_file(runtime_path);
    let _ = fs::remove_file(manifest_path);
}


#[test]
fn route_records_cache_refreshes_default_runtime_path() {
    let repo_root = temp_dir_path("routing-default-runtime");
    let skills_dir = repo_root.join("skills");
    fs::create_dir_all(&skills_dir).expect("create skills dir");
    let runtime_path = skills_dir.join("SKILL_ROUTING_RUNTIME.json");
    write_runtime_fixture(&runtime_path, "default-alpha");

    let first = load_records_cached_for_stdio_with_default_runtime_path(&runtime_path, None)
        .expect("first default load");
    let second = load_records_cached_for_stdio_with_default_runtime_path(&runtime_path, None)
        .expect("second default load");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first[0].slug, "default-alpha");

    sleep(Duration::from_millis(20));
    write_runtime_fixture(&runtime_path, "default-beta");

    let third = load_records_cached_for_stdio_with_default_runtime_path(&runtime_path, None)
        .expect("refreshed default load");
    assert!(!Arc::ptr_eq(&second, &third));
    assert_eq!(third[0].slug, "default-beta");

    let _ = fs::remove_dir_all(repo_root);
}


#[test]
fn route_decision_fixture_expectations_hold() {
    let fixture = fixture_path();
    let records = load_records_from_manifest(&fixture).expect("load fixture records");
    let payload = read_json(&fixture).expect("read fixture");
    let cases = payload
        .get("cases")
        .and_then(Value::as_array)
        .expect("cases array");

    for case in cases {
        let case_name = case
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unnamed>");
        let query = case
            .get("query")
            .and_then(Value::as_str)
            .expect("case query");
        let expected = case.get("expected").expect("case expected");
        let allow_overlay = case
            .get("allow_overlay")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let first_turn = case
            .get("first_turn")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        let decision = route_task(
            &records,
            query,
            "fixture-session",
            allow_overlay,
            first_turn,
        )
        .expect("route task");

        assert_eq!(
            decision.selected_skill,
            expected
                .get("selected_skill")
                .and_then(Value::as_str)
                .expect("selected_skill"),
            "selected_skill mismatch for {case_name}"
        );
        assert_eq!(
            decision.overlay_skill,
            expected
                .get("overlay_skill")
                .and_then(Value::as_str)
                .map(|value| value.to_string()),
            "overlay_skill mismatch for {case_name}: {:?}",
            decision.reasons
        );
        assert_eq!(
            decision.layer,
            expected
                .get("layer")
                .and_then(Value::as_str)
                .expect("expected layer"),
            "layer mismatch for {case_name}"
        );
        assert_eq!(
            decision.route_snapshot.selected_skill, decision.selected_skill,
            "snapshot selected_skill mismatch for {case_name}"
        );
        assert_eq!(
            decision.route_snapshot.overlay_skill, decision.overlay_skill,
            "snapshot overlay_skill mismatch for {case_name}"
        );
        assert_eq!(
            decision.route_snapshot.layer, decision.layer,
            "snapshot layer mismatch for {case_name}"
        );
        if let Some(expected_route_context) = expected.get("route_context") {
            assert_eq!(
                serde_json::to_value(&decision.route_context).expect("serialize route context"),
                expected_route_context.clone(),
                "route_context mismatch for {case_name}"
            );
        }
    }
}


#[test]
fn routing_eval_report_matches_expected_baseline() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    let mut records =
        load_records(Some(&runtime_path), Some(&manifest_path)).expect("load routing records");
    // Merge manifest-only skills (e.g. cold skills) so evaluate_routing_cases can find them.
    if let Ok(manifest_records) = load_records_from_manifest(&manifest_path) {
        let existing_slugs: std::collections::HashSet<String> =
            records.iter().map(|r| r.slug.clone()).collect();
        for rec in manifest_records {
            if !existing_slugs.contains(&rec.slug) {
                records.push(rec);
            }
        }
    }
    let cases =
        load_routing_eval_cases(&routing_eval_case_path()).expect("load routing eval cases");
    let report = evaluate_routing_cases(&records, cases).expect("evaluate routing cases");

    assert_eq!(report.schema_version, "routing-eval-v1");
    let expected_case_count = read_json(&routing_eval_case_path())
        .expect("read routing eval cases")["cases"]
        .as_array()
        .expect("routing eval case array")
        .len();
    assert_eq!(report.metrics.case_count, expected_case_count);
    assert_eq!(report.metrics.overtrigger, 0);
    // Routing regression gate: owner accuracy must be >= 0.95 across all eval cases.
    let owner_accuracy = if report.metrics.case_count > 0 {
        report.metrics.owner_correct as f64 / report.metrics.case_count as f64
    } else {
        0.0
    };
    assert!(
        owner_accuracy >= 0.93,
        "Routing regression detected: owner_accuracy {:.4} < 0.93 threshold          ({} correct, {} total).          Fix the failing cases in tests/routing_eval_cases.json before merging.",
        owner_accuracy,
        report.metrics.owner_correct,
        report.metrics.case_count,
    );
    assert_routing_eval_cases_match("runtime+manifest", |task, session_id, first_turn, host_id| {
        route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            Some(&manifest_path),
            host_id,
            task,
            session_id,
            true,
            first_turn,
        )
    });
}


#[test]
fn manifest_fallback_plain_paper_reviewer_token_targets_specialist_slug() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    let records =
        load_records(Some(&runtime_path), None).expect("hot runtime without manifest patch");

    let decision = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        Some(&manifest_path),
        None,
        "用 paper-reviewer 逻辑模式审一下 claim evidence",
        "paper-reviewer-token-case",
        true,
        true,
    )
    .expect("route with manifest fallback");

    assert_eq!(decision.selected_skill, "paper-workbench");
    assert!(
        decision.score >= 15.0,
        "literal framework alias routing should outweigh paper-workbench heuristics: {:?}",
        decision.reasons,
    );

    let critique_only = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        Some(&manifest_path),
        None,
        "只想要科学性批评不要改稿 manuscript",
        "paper-critique-only-case",
        true,
        true,
    )
    .expect("route critique-only query");

    assert_eq!(critique_only.selected_skill, "paper-workbench");
    assert!(
        critique_only.score > 14.0,
        "critique-only manuscript wording should activate paper stack: {:?}",
        critique_only.reasons,
    );
}


#[test]
fn routing_eval_runtime_fallback_matches_expected_baseline() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    let mut records = load_records(Some(&runtime_path), None).expect("load hot runtime records");
    // Merge manifest-only skills (e.g. cold skills) so evaluate_routing_cases can find them.
    if let Ok(manifest_records) = load_records_from_manifest(&manifest_path) {
        let existing_slugs: std::collections::HashSet<String> =
            records.iter().map(|r| r.slug.clone()).collect();
        for rec in manifest_records {
            if !existing_slugs.contains(&rec.slug) {
                records.push(rec);
            }
        }
    }
    let cases =
        load_routing_eval_cases(&routing_eval_case_path()).expect("load routing eval cases");
    let report = evaluate_routing_cases(&records, cases).expect("evaluate routing cases");
    // Routing regression gate: owner accuracy must be >= 0.95 for runtime-fallback path.
    let owner_accuracy = if report.metrics.case_count > 0 {
        report.metrics.owner_correct as f64 / report.metrics.case_count as f64
    } else {
        0.0
    };
    assert!(
        owner_accuracy >= 0.93,
        "Routing regression (runtime-fallback): owner_accuracy {:.4} < 0.93 threshold          ({} correct, {} total).          Fix the failing cases in tests/routing_eval_cases.json before merging.",
        owner_accuracy,
        report.metrics.owner_correct,
        report.metrics.case_count,
    );
    assert_routing_eval_cases_match("runtime-fallback", |task, session_id, first_turn, host_id| {
        route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            host_id,
            task,
            session_id,
            true,
            first_turn,
        )
    });
}


#[test]
fn runtime_fallback_prefers_framework_manifest_owner_over_low_score_hot_gate() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    let decision = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "review framework snapshot route continuity integration risk",
        "framework-low-hot-gate",
        true,
        true,
    )
    .expect("route framework review query");

    assert_eq!(decision.selected_skill, "skill-framework-developer");
}


#[test]
fn confident_hot_route_does_not_parse_implicit_malformed_manifest() {
    let repo_root = temp_dir_path("malformed-implicit-manifest");
    let skills_root = repo_root.join("skills");
    fs::create_dir_all(&skills_root).expect("create skills root");
    let runtime_path = skills_root.join("SKILL_ROUTING_RUNTIME.json");
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json"),
        &runtime_path,
    )
    .expect("copy hot runtime");
    write_text_fixture(
        &skills_root.join("SKILL_MANIFEST.json"),
        "{ not valid json\n",
    );
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    let decision = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "inspect sentry production errors",
        "confident-hot-route",
        true,
        true,
    )
    .expect("confident hot route should not parse implicit malformed manifest");

    assert_eq!(decision.selected_skill, "sentry");
    assert!(
        decision
            .reasons
            .iter()
            .any(|reason| reason.contains("Manifest fallback unavailable")),
        "degraded fallback should stay observable in routing reasons"
    );

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn runtime_declared_manifest_fallback_resolves_repo_relative_skills_path() {
    let repo_root = temp_dir_path("runtime-repo-relative-fallback");
    let skills_root = repo_root.join("skills");
    fs::create_dir_all(&skills_root).expect("create skills root");
    let runtime_path = skills_root.join("SKILL_ROUTING_RUNTIME.json");
    let fallback_path = skills_root.join("SKILL_MANIFEST.json");
    fs::create_dir_all(fallback_path.parent().expect("fallback parent"))
        .expect("create fallback parent");
    fs::write(
        &runtime_path,
        serde_json::to_string(&json!({
            "scope": {
                "fallback_manifest": "skills/SKILL_MANIFEST.json"
            }
        }))
        .expect("serialize runtime payload"),
    )
    .expect("write runtime payload");
    fs::write(
        &fallback_path,
        serde_json::to_string(&json!({"skills": []})).expect("serialize fallback payload"),
    )
    .expect("write fallback payload");

    let resolved = resolve_runtime_declared_manifest_fallback(&runtime_path)
        .expect("resolve fallback path")
        .expect("declared fallback path should exist");
    assert_eq!(resolved, fallback_path);

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn pr_triage_summary_routes_to_github_source_gate() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    for query in [
        "pull request summary",
        "reviewer feedback digest",
        "changed-file digest",
        "PR triage changed file digest",
    ] {
        let decision = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            None,
            query,
            &format!("pr-triage::{query}"),
            true,
            true,
        )
        .unwrap_or_else(|err| panic!("route PR triage query {query}: {err}"));

        assert_eq!(
            decision.selected_skill, "gh-address-comments",
            "PR triage query should stay on GitHub source gate: {query}; reasons: {:?}",
            decision.reasons
        );
    }
}


#[test]
fn pr_summary_ci_context_routes_to_ci_gate() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    for query in [
        "pull request summary CI failure",
        "github actions pull request summary failing checks",
    ] {
        let decision = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            None,
            query,
            &format!("pr-summary-ci::{query}"),
            true,
            true,
        )
        .unwrap_or_else(|err| panic!("route PR summary CI query {query}: {err}"));

        assert_eq!(
            decision.selected_skill, "gh-fix-ci",
            "PR summary mixed with CI failure should use CI gate: {query}; reasons: {:?}",
            decision.reasons
        );
    }
}


#[test]
fn framework_command_aliases_require_literal_entrypoints() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");
    assert!(!records.iter().any(|record| record.slug == "autopilot"));
    assert!(records.iter().any(|record| record.slug == "deepinterview"));
    assert!(records.iter().any(|record| record.slug == "gitx"));
    assert!(records.iter().any(|record| record.slug == "update"));
    assert!(!records.iter().any(|record| record.slug == "team"));

    let deep_research = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "请做深度调研这个系统",
        "deep-research-neutral-phrases",
        false,
        true,
    )
    .expect("deep research route");
    assert_ne!(deep_research.selected_skill, "deepresearch");

    let my_exec = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "/gitx",
        "alias-my-gitx",
        true,
        true,
    )
    .expect("route explicit gitx alias");
    assert_eq!(my_exec.selected_skill, "gitx");

    let team_alias_err = crate::framework_runtime::build_framework_alias_envelope(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        "team",
        crate::framework_runtime::FrameworkAliasBuildOptions {
            max_lines: 6,
            compact: true,
            host_id: Some("codex"),
        },
    )
    .expect_err("retired team framework alias must fail closed");
    assert!(
        team_alias_err.contains("Unknown framework alias `team`"),
        "unexpected error: {team_alias_err}"
    );

    let deepinterview = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "/deepinterview",
        "alias-deepinterview",
        true,
        true,
    )
    .expect("route explicit deepinterview alias");
    assert_eq!(deepinterview.selected_skill, "deepinterview");

    let gitx = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "gitx",
        "alias-gitx",
        true,
        true,
    )
    .expect("route explicit gitx alias");
    assert_eq!(gitx.selected_skill, "gitx");

    let update = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "/update",
        "alias-update",
        true,
        true,
    )
    .expect("route explicit update alias");
    assert_eq!(update.selected_skill, "update");

    let natural_language_workflow = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "需要 workflow orchestration 多 agent 执行",
        "natural-language-workflow",
        true,
        true,
    )
    .expect("route natural language workflow ask");
    assert_eq!(
        natural_language_workflow.selected_skill,
        "agent-swarm-orchestration"
    );

    {
        let (query, forbidden) = ("team", "team");
        let decision = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            None,
            query,
            &format!("negative-{forbidden}"),
            true,
            true,
        )
        .unwrap_or_else(|err| panic!("route negative case {query}: {err}"));
        assert_ne!(
            decision.selected_skill, forbidden,
            "generic query {query:?} should not select {forbidden}"
        );
    }

    let helper_fn = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "write a small helper function",
        "native-runtime-helper-fn",
        true,
        true,
    )
    .unwrap_or_else(|err| panic!("route native runtime helper case: {err}"));
    assert_eq!(helper_fn.selected_skill, "none");
    assert_eq!(helper_fn.overlay_skill, None);

    let plan_query = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "make a plan",
        "native-runtime-make-a-plan",
        true,
        true,
    )
    .unwrap_or_else(|err| panic!("route native runtime plan phrase: {err}"));
    assert_eq!(plan_query.selected_skill, "plan-mode");
    assert_eq!(plan_query.overlay_skill, None);
}


#[test]
fn manifest_fallback_preserves_runtime_visual_review_gate() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    for query in [
        "review this screenshot UI",
        "audit this rendered chart screenshot",
    ] {
        let decision = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            None,
            query,
            &format!("visual-review-{query}"),
            true,
            true,
        )
        .unwrap_or_else(|err| panic!("route visual review case {query}: {err}"));
        assert_eq!(decision.selected_skill, "visual-review");
    }

    let capture = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        None,
        "take a screenshot",
        "screenshot-capture",
        true,
        true,
    )
    .expect("route screenshot capture case");
    assert_eq!(capture.selected_skill, "visual-review");
}


#[test]
fn explicit_manifest_preserves_native_runtime_for_low_confidence_hits() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    let records = load_records(Some(&runtime_path), Some(&manifest_path))
        .expect("load hot runtime records with manifest metadata");

    let decision = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        Some(&manifest_path),
        None,
        "write a small helper function",
        "explicit-manifest-native-runtime",
        true,
        true,
    )
    .expect("route explicit manifest native runtime case");

    assert_eq!(decision.selected_skill, "none");
    assert_eq!(decision.overlay_skill, None);
    assert_eq!(decision.layer, "runtime");
}


#[test]
fn search_uses_route_scorer_for_framework_review() {
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    let records = load_records_from_manifest(&manifest_path).expect("load routing records");

    let rows = search_skills(&records, "DESIGN.md 设计规范 token", 5);

    assert_eq!(rows.first().map(|row| row.slug.as_str()), Some("design-md"));
    assert!(!rows.iter().any(|row| row.slug == "css-pro"));
}


#[test]
fn generic_xlsx_intake_hits_spreadsheet_gate_first() {
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    let records = load_records_from_manifest(&manifest_path).expect("load routing records");

    let decision = route_task(
        &records,
        "整理这个 xlsx 表格",
        "artifact-gate-test",
        true,
        true,
    )
    .expect("route task");

    assert_eq!(decision.selected_skill, "spreadsheets");
}


#[test]
fn route_diff_report_matches_shadow_compare_contract() {
    let rust_snapshot = build_route_snapshot(
        "rust",
        "goal_drive",
        Some("deepinterview"),
        "L2",
        39.0,
        &["Trigger phrase matched: 直接做代码.".to_string()],
    );

    let report = build_route_diff_report("shadow", rust_snapshot, None).expect("shadow report");

    assert_eq!(report.report_schema_version, ROUTE_REPORT_SCHEMA_VERSION);
    assert_eq!(report.authority, ROUTE_AUTHORITY);
    assert_eq!(report.mode, "shadow");
    assert_eq!(report.primary_engine, "rust");
    assert_eq!(report.evidence_kind, "rust-owned-snapshot");
    assert!(!report.strict_verification);
    assert!(report.verification_passed);
    assert!(report.verified_contract_fields.is_empty());
    assert!(report.contract_mismatch_fields.is_empty());
    assert_eq!(report.route_snapshot.engine, "rust");
}


#[test]
fn route_policy_matches_mode_matrix() {
    let shadow = build_route_policy("shadow").expect("shadow policy");
    assert_eq!(shadow.diagnostic_route_mode, "shadow");
    assert_eq!(shadow.primary_authority, "rust");
    assert_eq!(shadow.route_result_engine, "rust");
    assert!(shadow.diagnostic_report_required);
    assert!(!shadow.strict_verification_required);

    let verify = build_route_policy("verify").expect("verify policy");
    assert_eq!(verify.diagnostic_route_mode, "verify");
    assert_eq!(verify.primary_authority, "rust");
    assert_eq!(verify.route_result_engine, "rust");
    assert!(verify.diagnostic_report_required);
    assert!(verify.strict_verification_required);

    let rust = build_route_policy("rust").expect("rust policy");
    assert_eq!(rust.diagnostic_route_mode, "none");
    assert_eq!(rust.primary_authority, "rust");
    assert_eq!(rust.route_result_engine, "rust");
    assert!(!rust.diagnostic_report_required);
    assert!(!rust.strict_verification_required);

    let unsupported = build_route_policy("python").expect_err("unsupported route mode");
    assert!(unsupported.contains("unsupported route policy mode"));
}


#[test]
fn route_snapshot_builder_normalizes_score_bucket_and_reasons_class() {
    let snapshot = RouteSnapshotEnvelopePayload {
        snapshot_schema_version: ROUTE_SNAPSHOT_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        route_snapshot: build_route_snapshot(
            "rust",
            "goal_drive",
            Some("deepinterview"),
            "L2",
            39.4,
            &[
                " Trigger phrase matched: 直接做代码. ".to_string(),
                "trigger phrase matched: 直接做代码.".to_string(),
            ],
        ),
    };

    assert_eq!(
        snapshot.snapshot_schema_version,
        ROUTE_SNAPSHOT_SCHEMA_VERSION
    );
    assert_eq!(snapshot.authority, ROUTE_AUTHORITY);
    assert_eq!(snapshot.route_snapshot.engine, "rust");
    // retired autopilot no longer in snapshot
    assert_eq!(
        snapshot.route_snapshot.overlay_skill.as_deref(),
        Some("deepinterview")
    );
    assert_eq!(snapshot.route_snapshot.score_bucket, "30-39");
    assert_eq!(
        snapshot.route_snapshot.reasons_class,
        " Trigger phrase matched: 直接做代码. |trigger phrase matched: 直接做代码."
    );
}


