//! 测试：RFV 全流程测试 + 校验函数矩阵测试。

use super::*;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn rfv_start_append_roundtrip() {
    let _nudge_env = core_policy::test_env_sync::harness_nudges_env_test_lock();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/rfv-task")).expect("mkdir");
    let skill_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let nudge_src = skill_root.join("configs/framework/HARNESS_OPERATOR_NUDGES.json");
    fs::create_dir_all(repo.join("configs/framework")).expect("nudge dir");
    fs::copy(
        &nudge_src,
        repo.join("configs/framework/HARNESS_OPERATOR_NUDGES.json"),
    )
    .expect("copy harness nudges fixture");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"rfv-task"}"#,
    )
    .expect("pointer");

    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr,
        "operation": "start",
        "task_id": "rfv-task",
        "goal": "harden loop",
        "max_rounds": 100,
        "allow_external_research": true,
        "verify_commands": ["cargo test -q"],
        "stop_when": ["verifier pass", "max_rounds"],
    }))
    .expect("start");

    framework_quality_gate(json!({
        "repo_root": rr,
        "operation": "append_round",
        "task_id": "rfv-task",
        "round": 1u64,
        "review_summary": "r1",
        "external_research_summary": "web: none",
        "fix_summary": "f1",
        "verify_result": "PASS",
        "adversarial_findings": [
            {"id":"A1","hypothesis":"panic on empty input","severity":"high"}
        ],
        "falsification_tests": [
            {"id":"T1","command":"cargo test -q","expect":"pass"}
        ],
        "supervisor_decision": "continue",
        "reason": "ok",
    }))
    .expect("append");

    let st = framework_quality_gate(json!({
        "repo_root": rr,
        "operation": "status",
    }))
    .expect("status");
    let gs = st["quality_gate_state"].as_object().expect("obj");
    assert_eq!(gs["external_research_strict"], json!(true));
    assert_eq!(gs["current_round"], json!(1));
    assert_eq!(gs["loop_status"], json!("active"));
    let rounds = gs["rounds"].as_array().expect("rounds");
    let r1 = rounds[0].as_object().expect("round1 obj");
    assert!(r1.get("adversarial_findings").is_some());
    assert!(r1.get("falsification_tests").is_some());
    let _via_api = read_quality_gate_state(&repo, None)
        .expect("read api")
        .expect("state");

    let _ = fs::remove_dir_all(&repo);
}

/// P0-A: invalid `verify_result` is rejected (not silently coerced).
#[test]
fn append_round_rejects_unknown_verify_result() {
    let _nudge_env = core_policy::test_env_sync::harness_nudges_env_test_lock();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-vr-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/t-vr")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-vr"}"#,
    )
    .expect("active");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "t-vr",
        "goal": "verify enum",
        "max_rounds": 5u64,
    }))
    .expect("start");
    let err = framework_quality_gate(json!({
        "repo_root": rr,
        "operation": "append_round",
        "task_id": "t-vr",
        "round": 1u64,
        "verify_result": "kinda passed",
    }))
    .expect_err("invalid verify_result must error");
    assert!(
        err.contains("verify_result must be one of"),
        "unexpected error: {err}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// P1-B: PASS round with no successful EVIDENCE_INDEX rows surfaces `cross_check=no_evidence_window`.
#[test]
fn append_round_marks_pass_without_evidence() {
    let _nudge_env = core_policy::test_env_sync::harness_nudges_env_test_lock();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-cl-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    let task_dir = repo.join("artifacts/current/t-cl");
    fs::create_dir_all(&task_dir).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-cl"}"#,
    )
    .expect("active");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "t-cl",
        "goal": "cross-link",
        "max_rounds": 3u64,
    }))
    .expect("start");
    // No EVIDENCE_INDEX yet → PASS should land with no_evidence_window.
    let out = framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "t-cl",
        "round": 1u64,
        "verify_result": "PASS",
    }))
    .expect("append");
    let rounds = out["quality_gate_state"]["rounds"]
        .as_array()
        .expect("rounds array");
    let r1 = &rounds[0];
    assert_eq!(r1["cross_check"], json!("no_evidence_window"));
    assert!(r1["evidence_refs"].as_array().expect("refs").is_empty());

    // Now write a successful EVIDENCE row newer than the round timestamp and append round 2.
    // Use a timestamp far in the future so it deterministically beats round 1's `at`.
    fs::write(
        task_dir.join("EVIDENCE_INDEX.json"),
        r#"{"schema_version":"evidence-index-v2","artifacts":[{"recorded_at":"2099-12-31T23:59:59Z","exit_code":0,"success":true}]}"#,
    )
    .expect("evidence");
    let out2 = framework_quality_gate(json!({
        "repo_root": rr,
        "operation": "append_round",
        "task_id": "t-cl",
        "round": 2u64,
        "verify_result": "PASS",
    }))
    .expect("append 2");
    let rounds2 = out2["quality_gate_state"]["rounds"]
        .as_array()
        .expect("rounds 2");
    let r2 = &rounds2[1];
    assert!(
        r2.get("cross_check").is_none(),
        "expected cross_check absent on PASS-with-evidence; round={r2}"
    );
    assert!(
        !r2["evidence_refs"].as_array().expect("refs2").is_empty(),
        "expected non-empty evidence_refs; round={r2}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// RFV 与 GOAL 同 task 互斥：RFV start 应删除已存在的 GOAL_STATE。
#[test]
fn rfv_start_clears_goal_same_task() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-goal-rfv-mutex-rfv-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/rfv-mx")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"rfv-mx"}"#,
    )
    .expect("pointer");
    let rr = repo.display().to_string();

    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "rfv-mx",
        "goal": "macro first",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    let gpath =
        core_state::state_manager::goal_state_path_for_task(&repo, "rfv-mx").expect("gpath");
    assert!(gpath.is_file());

    let out = framework_quality_gate(json!({
        "repo_root": rr,
        "operation": "start",
        "task_id": "rfv-mx",
        "goal": "rfv mode",
        "max_rounds": 2u64,
    }))
    .expect("rfv start");
    assert_eq!(out["goal_state_cleared"], json!(true));
    // Goal state is now marked superseded rather than deleted (symmetric with goal supersede RFV)
    assert!(
        gpath.is_file(),
        "GOAL_STATE should still exist after RFV supersede"
    );
    let goal_state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&gpath).expect("read goal state"))
            .expect("parse goal state");
    assert_eq!(goal_state["status"], "superseded");
    assert_eq!(goal_state["metadata"]["superseded_by"], "quality_gate");

    let _ = fs::remove_dir_all(&repo);
}

fn minimal_external_research_loose_only() -> Value {
    json!({
        "claims": [{"claim": "c1", "sources": ["https://a.example/foo"]}],
        "contradiction_sweep": [{
            "related_claim_or_topic": "t1",
            "contradicting_or_limiting_evidence": "e1",
            "sources": ["https://contradicts.example/bar"],
        }],
        "retrieval_trace": {
            "queries_used": ["duckdb reproducibility"],
            "inclusion_rules": "official docs first",
            "exclusions": "forum posts without primary cites",
            "exclusion_rationale": "noise",
        }
    })
}

/// Satisfies both [`validate_external_research_structured`] and [`validate_external_research_strict`].
fn minimal_external_research() -> Value {
    let t40 = "0123456789012345678901234567890123456789";
    json!({
        "claims": [{
            "claim": "c1",
            "sources": [
                "https://a.example/foo",
                "doi:10.1000/182"
            ]
        }],
        "contradiction_sweep": [
            {
                "related_claim_or_topic": "t1",
                "contradicting_or_limiting_evidence": "e1",
                "sources": ["https://contradicts.example/bar"]
            },
            {
                "related_claim_or_topic": "t2",
                "contradicting_or_limiting_evidence": "e2",
                "sources": ["arxiv:2301.00001v1"]
            }
        ],
        "unknowns": [],
        "retrieval_trace": {
            "queries_used": ["q1 scope literature", "q2 methods survey", "q3 risk edge cases"],
            "inclusion_rules": t40,
            "exclusions": t40,
            "exclusion_rationale": t40
        }
    })
}

#[test]
fn source_traceable_heuristic_matrix() {
    assert!(source_traceable_heuristic("  https://x/y "));
    assert!(source_traceable_heuristic("HTTP://LOCALHOST/z"));
    assert!(source_traceable_heuristic("doi:10.1000/182"));
    assert!(source_traceable_heuristic("DOI:10.9999/zenodo.123"));
    assert!(source_traceable_heuristic("10.5281/zenodo.12345"));
    assert!(source_traceable_heuristic("ArXiv:2301.00001"));
    assert!(source_traceable_heuristic("PMID:12345678"));
    assert!(source_traceable_heuristic("ISBN:978-3-16-148410-0"));
    assert!(source_traceable_heuristic("dataset:gov.example/series/v1"));
    assert!(source_traceable_heuristic("official_doc:eu-reg-2024/001"));
    // New prefixes
    assert!(source_traceable_heuristic("huggingface:transformers"));
    assert!(source_traceable_heuristic("HF:bert-base"));
    assert!(source_traceable_heuristic("github:anthropic/claude"));
    assert!(source_traceable_heuristic("kaggle:datasets/imagenet"));
    assert!(source_traceable_heuristic(
        "geojson:https://example.com/data.geojson"
    ));

    assert!(!source_traceable_heuristic(""));
    assert!(!source_traceable_heuristic("random blog title"));
    assert!(!source_traceable_heuristic("ftp://files.example/a"));
    assert!(!source_traceable_heuristic("doi:9.1234/nope"));
    assert!(!source_traceable_heuristic("10.1234"));
}

#[test]
fn validate_external_research_strict_matrix() {
    validate_external_research_strict(&minimal_external_research()).expect("strict minimal");

    let err = validate_external_research_strict(&minimal_external_research_loose_only())
        .expect_err("loose missing unknowns");
    assert!(err.contains("missing `unknowns` key"), "unexpected: {err}");

    let mut one_src = minimal_external_research();
    one_src.as_object_mut().unwrap().insert(
        "claims".to_string(),
        json!([{"claim":"x","sources":["https://a.example/only-one"]}]),
    );
    let err = validate_external_research_strict(&one_src).expect_err("single-source claim");
    assert!(err.contains("`sources` must have at least 2"), "{err}");

    let mut bad_sweep = minimal_external_research();
    bad_sweep.as_object_mut().unwrap().insert(
        "contradiction_sweep".to_string(),
        json!([{
            "related_claim_or_topic":"t",
            "contradicting_or_limiting_evidence":"e",
            "sources":["https://x"]
        }]),
    );
    let err = validate_external_research_strict(&bad_sweep).expect_err("short sweep");
    assert!(
        err.contains("contradiction_sweep must have at least"),
        "{err}"
    );

    let mut q2 = minimal_external_research();
    let tr = q2
        .as_object_mut()
        .unwrap()
        .get_mut("retrieval_trace")
        .unwrap()
        .as_object_mut()
        .unwrap();
    tr.insert("queries_used".to_string(), json!(["a", "b"]));
    let err = validate_external_research_strict(&q2).expect_err("queries");
    assert!(err.contains("queries_used must have at least 3"), "{err}");

    let mut short_trace = minimal_external_research();
    let tr = short_trace
        .as_object_mut()
        .unwrap()
        .get_mut("retrieval_trace")
        .unwrap()
        .as_object_mut()
        .unwrap();
    tr.insert("inclusion_rules".to_string(), json!("short"));
    let err = validate_external_research_strict(&short_trace).expect_err("trace len");
    assert!(
        err.contains("inclusion_rules") && err.contains("at least 40"),
        "{err}"
    );

    let mut bad_unk = minimal_external_research();
    bad_unk
        .as_object_mut()
        .unwrap()
        .insert("unknowns".to_string(), json!("nope"));
    let err = validate_external_research_strict(&bad_unk).expect_err("unknowns type");
    assert!(err.contains("unknowns` must be array or null"), "{err}");
}

#[test]
fn append_round_strict_rejects_without_round_write() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-er-strict-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/t-er-st")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-er-st"}"#,
    )
    .expect("active");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "t-er-st",
        "goal": "strict ext",
        "max_rounds": 3u64,
    }))
    .expect("start");

    let err = framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "t-er-st",
        "round": 1u64,
        "external_research": minimal_external_research_loose_only(),
        "verify_result": "PASS",
    }))
    .expect_err("strict rejects loose blob");
    assert!(
        err.contains("external_research strict"),
        "unexpected err: {err}"
    );
    let st = framework_quality_gate(json!({"repo_root": rr.clone(), "operation": "status"}))
        .expect("st");
    assert!(
        st["quality_gate_state"]["rounds"]
            .as_array()
            .expect("rounds")
            .is_empty()
    );

    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "t-er-st",
        "round": 1u64,
        "external_research": minimal_external_research(),
        "verify_result": "PASS",
    }))
    .expect("strict ok append");

    let st2 = framework_quality_gate(json!({"repo_root": rr, "operation": "status"})).expect("st2");
    assert_eq!(st2["quality_gate_state"]["rounds"].as_array().unwrap().len(), 1);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn append_round_legacy_missing_strict_flag_accepts_loose_blob() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-legacy-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/t-leg")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-leg"}"#,
    )
    .expect("active");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "t-leg",
        "goal": "legacy",
        "max_rounds": 3u64,
    }))
    .expect("start");

    let path = quality_gate_state_path(&repo, "t-leg").expect("path");
    let mut v: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
    v.as_object_mut()
        .expect("obj")
        .remove("external_research_strict");
    write_atomic_json(&path, &v).expect("rewrite");

    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "t-leg",
        "round": 1u64,
        "external_research": minimal_external_research_loose_only(),
        "verify_result": "PASS",
    }))
    .expect("legacy append with loose blob");

    let st = framework_quality_gate(json!({"repo_root": rr, "operation": "status"})).expect("st");
    assert_eq!(st["quality_gate_state"]["rounds"].as_array().unwrap().len(), 1);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn append_round_respects_explicit_external_research_strict_false() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-loose-flag-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/t-loose")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-loose"}"#,
    )
    .expect("active");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "t-loose",
        "goal": "loose",
        "max_rounds": 3u64,
        "external_research_strict": false,
    }))
    .expect("start");

    let st = framework_quality_gate(json!({"repo_root": rr.clone(), "operation": "status"}))
        .expect("st");
    assert_eq!(
        st["quality_gate_state"]["external_research_strict"],
        json!(false)
    );

    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "t-loose",
        "round": 1u64,
        "external_research": minimal_external_research_loose_only(),
        "verify_result": "PASS",
    }))
    .expect("append loose");

    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn validate_external_research_struct_matrix() {
    validate_external_research_structured(&minimal_external_research()).expect("minimal ok");

    validate_external_research_structured(&json!({})).expect_err("empty root must reject");
    validate_external_research_structured(&json!({"claims": [], "contradiction_sweep": [], "retrieval_trace": {"queries_used":[], "inclusion_rules":"a","exclusions":"b","exclusion_rationale":"c"}}))
        .expect_err("empty arrays");

    validate_external_research_structured(&json!({
        "claims": [{"claim":"","sources":["u"]}],
        "contradiction_sweep": [{"related_claim_or_topic":"a","contradicting_or_limiting_evidence":"b","sources":["s"]}],
        "retrieval_trace": {"queries_used":["q"],"inclusion_rules":"i","exclusions":"x","exclusion_rationale":"r"}
    })).expect_err("empty claim trim");

    let mut with_unknown = minimal_external_research();
    let uo = with_unknown.as_object_mut().unwrap();
    uo.insert(
        "unknowns".to_string(),
        json!([
            {"question": "pq", "why_insufficient": "no data"}
        ]),
    );
    validate_external_research_structured(&with_unknown).expect("unknowns optional");

    let mut qr_none = minimal_external_research();
    qr_none
        .as_object_mut()
        .unwrap()
        .insert("quantitative_replays".to_string(), json!("NONE"));
    validate_external_research_structured(&qr_none).expect("uppercase NONE");

    let mut qr_arr = minimal_external_research();
    qr_arr.as_object_mut().unwrap().insert(
        "quantitative_replays".to_string(),
        json!([{
            "dataset_or_source_id": "d",
            "version_or_snapshot": "v",
            "window": "2020-2025",
            "replay_command": "python - <<'PY'\nPY",
        }]),
    );
    validate_external_research_structured(&qr_arr).expect("quant array");
}

#[test]
fn append_round_rejects_bad_external_research_without_write() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-er-bad-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/t-er-bad")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-er-bad"}"#,
    )
    .expect("active");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "t-er-bad",
        "goal": "structured ext",
        "max_rounds": 3u64,
    }))
    .expect("start");

    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "t-er-bad",
        "round": 1u64,
        "external_research": {"claims":[],"contradiction_sweep":[],"retrieval_trace":{}},
        "verify_result": "PASS",
    }))
    .expect_err("invalid external payload");

    let st = framework_quality_gate(json!({"repo_root": rr, "operation": "status"})).expect("st");
    let rounds = st["quality_gate_state"]["rounds"].as_array().expect("arr");
    assert!(rounds.is_empty(), "rounds unchanged on validation failure");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn append_round_persists_valid_external_research() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-er-good-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/t-er-good")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-er-good"}"#,
    )
    .expect("active");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "t-er-good",
        "goal": "x",
        "max_rounds": 3u64,
    }))
    .expect("start");

    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "t-er-good",
        "round": 1u64,
        "external_research": minimal_external_research(),
        "verify_result": "PASS",
    }))
    .expect("append ok");

    let rounds = framework_quality_gate(json!({"repo_root": rr, "operation": "status"}))
        .expect("st")["quality_gate_state"]["rounds"]
        .as_array()
        .expect("rounds")
        .clone();
    assert_eq!(rounds.len(), 1);
    let er = rounds[0]
        .get("external_research")
        .expect("external_research");
    assert!(
        validate_external_research_structured(er).is_ok(),
        "stored blob should re-validate",
    );
    assert!(
        validate_external_research_strict(er).is_ok(),
        "stored blob should satisfy strict when task default strict",
    );

    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn rfv_start_writes_prefer_structured_flag() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-pref-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/pref-task")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"pref-task"}"#,
    )
    .expect("ptr");
    let rr = repo.display().to_string();

    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "pref-task",
        "goal": "g",
        "max_rounds": 2u64,
        "prefer_structured_external_research": true,
    }))
    .expect("start");

    let st =
        framework_quality_gate(json!({"repo_root": rr, "operation": "status"})).expect("status");
    assert_eq!(
        st["quality_gate_state"]["prefer_structured_external_research"],
        json!(true)
    );
    assert_eq!(
        st["quality_gate_state"]["external_research_strict"],
        json!(true)
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn rfv_start_defaults_prefer_structured_when_allow_external() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-prefdef-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/extdef")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"extdef"}"#,
    )
    .expect("ptr");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "extdef",
        "goal": "g",
        "max_rounds": 2u64,
        "allow_external_research": true,
    }))
    .expect("start");
    let st =
        framework_quality_gate(json!({"repo_root": rr, "operation": "status"})).expect("status");
    assert_eq!(
        st["quality_gate_state"]["prefer_structured_external_research"],
        json!(true)
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn append_round_close_gates_reject_skipped_when_require_pass() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-closegate-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/cg-task")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"cg-task"}"#,
    )
    .expect("ptr");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "cg-task",
        "goal": "g",
        "max_rounds": 3u64,
        "close_gates": {
            "enabled": true,
            "require_last_round_verify_pass": true
        }
    }))
    .expect("start");
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "cg-task",
        "round": 1u64,
        "review_summary": "r",
        "fix_summary": "f",
        "verify_result": "PASS",
        "supervisor_decision": "continue",
    }))
    .expect("r1");
    let err = framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "cg-task",
        "round": 2u64,
        "review_summary": "r",
        "fix_summary": "f",
        "verify_result": "SKIPPED",
        "supervisor_decision": "close",
    }))
    .expect_err("close with SKIPPED should fail gates");
    assert!(
        err.contains("close_gates") && err.contains("verify_result"),
        "err={err}"
    );
    let st =
        framework_quality_gate(json!({"repo_root": rr, "operation": "status"})).expect("status");
    assert_eq!(
        st["quality_gate_state"]["rounds"].as_array().map(|a| a.len()),
        Some(1)
    );
    let _ = fs::remove_dir_all(&repo);
}

/// `close_gates` 在 **`max_rounds` 耗尽**（非显式 close）路径与显式 close 一致：仍校验收口轮。
#[test]
fn append_round_close_gates_enforced_on_max_rounds_cap() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-capgate-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/cap-g")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"cap-g"}"#,
    )
    .expect("ptr");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "cap-g",
        "goal": "g",
        "max_rounds": 2u64,
        "close_gates": {
            "enabled": true,
            "require_last_round_verify_pass": true
        }
    }))
    .expect("start");
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "cap-g",
        "round": 1u64,
        "verify_result": "PASS",
        "supervisor_decision": "continue",
    }))
    .expect("r1");
    let err = framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "cap-g",
        "round": 2u64,
        "verify_result": "SKIPPED",
        "supervisor_decision": "continue",
    }))
    .expect_err("max_rounds cap close must still enforce verify_pass gate");
    assert!(
        err.contains("close_gates") && err.contains("verify_result"),
        "unexpected err: {err}"
    );
    let st =
        framework_quality_gate(json!({"repo_root": rr, "operation": "status"})).expect("status");
    assert_eq!(
        st["quality_gate_state"]["rounds"].as_array().map(|a| a.len()),
        Some(1)
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn append_round_max_rounds_cap_passes_close_gates_when_verify_pass() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("router-rs-rfv-capgate-ok-{suffix}"));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/cap-ok")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"cap-ok"}"#,
    )
    .expect("ptr");
    let rr = repo.display().to_string();
    framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "start",
        "task_id": "cap-ok",
        "goal": "g",
        "max_rounds": 1u64,
        "close_gates": {
            "enabled": true,
            "require_last_round_verify_pass": true
        }
    }))
    .expect("start");
    let out = framework_quality_gate(json!({
        "repo_root": rr.clone(),
        "operation": "append_round",
        "task_id": "cap-ok",
        "round": 1u64,
        "verify_result": "PASS",
        "supervisor_decision": "continue",
    }))
    .expect("single round hits cap");
    let gs = out["quality_gate_state"].as_object().expect("obj");
    assert_eq!(gs.get("loop_status"), Some(&json!("closed")));
    assert_eq!(gs["rounds"].as_array().map(|a| a.len()), Some(1));
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn parse_iso_datetime_accepts_rfc3339() {
    assert!(parse_iso_datetime("2024-01-01T12:00:00Z").is_some());
    assert!(parse_iso_datetime("2024-01-01T12:00:00+00:00").is_some());
    assert!(parse_iso_datetime("2024-01-01T12:00:00-05:00").is_some());
}

#[test]
fn parse_iso_datetime_rejects_non_rfc3339() {
    // Non-RFC3339 formats are now rejected
    assert!(parse_iso_datetime("2024-01-01 12:00:00").is_none());
    assert!(parse_iso_datetime("2024-01-01T12:00:00").is_none());
    assert!(parse_iso_datetime("2024/01/01 12:00:00").is_none());
}
