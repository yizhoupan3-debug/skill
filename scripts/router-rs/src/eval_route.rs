use crate::route::{load_records, ROUTE_AUTHORITY, ROUTE_DECISION_SCHEMA_VERSION};
use crate::route_task_with_manifest_fallback;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

pub const EVAL_ROUTE_REPORT_SCHEMA_VERSION: &str = "routing-eval-report-v1";
pub const EVAL_ROUTE_AUTHORITY: &str = "rust-eval-route";

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct EvalCasePayload {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub category: String,
    pub task: String,
    #[serde(default)]
    pub expected_owner: Option<String>,
    #[serde(default)]
    pub expected_overlay: Option<String>,
    #[serde(default)]
    pub expected_layer: Option<String>,
    #[serde(default)]
    pub forbidden_owners: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct EvalCasesPayload {
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub cases: Vec<EvalCasePayload>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvalCaseFailure {
    pub case_id: String,
    pub field: String,
    pub expected: Value,
    pub got: Value,
    pub task: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvalRouteReport {
    pub schema_version: String,
    pub authority: String,
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub route_accuracy: f64,
    pub wrong_owner_rate: f64,
    pub wrong_overlay_rate: f64,
    pub wrong_layer_rate: f64,
    pub failures: Vec<EvalCaseFailure>,
}

pub fn load_eval_cases(path: &Path) -> Result<EvalCasesPayload, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read eval cases file {}: {err}", path.display()))?;
    let payload: EvalCasesPayload = serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse eval cases {}: {err}", path.display()))?;
    Ok(payload)
}

pub fn evaluate_route_cases(
    records: &[crate::route::SkillRecord],
    cases: &[EvalCasePayload],
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> EvalRouteReport {
    let mut passed = 0_usize;
    let mut failed = 0_usize;
    let mut wrong_owner = 0_usize;
    let mut wrong_overlay = 0_usize;
    let mut wrong_layer = 0_usize;
    let mut failures: Vec<EvalCaseFailure> = Vec::new();

    for case in cases {
        if case.task.trim().is_empty() {
            continue;
        }

        let session_id = format!("eval-route::{}", case.id);
        let decision = match route_task_with_manifest_fallback(
            records,
            runtime_path,
            manifest_path,
            &case.task,
            &session_id,
            true,
            true,
        ) {
            Ok(d) => d,
            Err(err) => {
                failures.push(EvalCaseFailure {
                    case_id: case.id.clone(),
                    field: "route_error".to_string(),
                    expected: json!("valid decision"),
                    got: json!(err),
                    task: case.task.clone(),
                });
                failed += 1;
                continue;
            }
        };

        let mut case_failed = false;

        // Check owner
        if let Some(ref expected) = case.expected_owner {
            let expected_norm = expected.trim();
            if !expected_norm.is_empty() && decision.selected_skill != expected_norm {
                failures.push(EvalCaseFailure {
                    case_id: case.id.clone(),
                    field: "selected_skill".to_string(),
                    expected: json!(expected_norm),
                    got: json!(decision.selected_skill),
                    task: case.task.clone(),
                });
                wrong_owner += 1;
                case_failed = true;
            }
        }

        // Check overlay
        if let Some(ref expected) = case.expected_overlay {
            let expected_norm = expected.trim();
            if !expected_norm.is_empty() {
                let got = decision.overlay_skill.clone().unwrap_or_default();
                if got != expected_norm {
                    failures.push(EvalCaseFailure {
                        case_id: case.id.clone(),
                        field: "overlay_skill".to_string(),
                        expected: json!(expected_norm),
                        got: json!(got),
                        task: case.task.clone(),
                    });
                    wrong_overlay += 1;
                    case_failed = true;
                }
            }
        } else if decision.overlay_skill.is_some() {
            // Expected no overlay but got one
            failures.push(EvalCaseFailure {
                case_id: case.id.clone(),
                field: "overlay_skill".to_string(),
                expected: json!(null),
                got: json!(decision.overlay_skill),
                task: case.task.clone(),
            });
            wrong_overlay += 1;
            case_failed = true;
        }

        // Check layer
        if let Some(ref expected) = case.expected_layer {
            let expected_norm = expected.trim();
            if !expected_norm.is_empty() && decision.layer != expected_norm {
                failures.push(EvalCaseFailure {
                    case_id: case.id.clone(),
                    field: "layer".to_string(),
                    expected: json!(expected_norm),
                    got: json!(decision.layer),
                    task: case.task.clone(),
                });
                wrong_layer += 1;
                case_failed = true;
            }
        }

        // Check forbidden owners
        for forbidden in &case.forbidden_owners {
            let forbidden_norm = forbidden.trim();
            if !forbidden_norm.is_empty() && decision.selected_skill == forbidden_norm {
                failures.push(EvalCaseFailure {
                    case_id: case.id.clone(),
                    field: "forbidden_owner".to_string(),
                    expected: json!(format!("not {forbidden_norm}")),
                    got: json!(decision.selected_skill),
                    task: case.task.clone(),
                });
                wrong_owner += 1;
                case_failed = true;
            }
        }

        if case_failed {
            failed += 1;
        } else {
            passed += 1;
        }
    }

    let total = passed + failed;
    let route_accuracy = if total > 0 {
        passed as f64 / total as f64
    } else {
        0.0
    };
    let wrong_owner_rate = if total > 0 {
        wrong_owner as f64 / total as f64
    } else {
        0.0
    };
    let wrong_overlay_rate = if total > 0 {
        wrong_overlay as f64 / total as f64
    } else {
        0.0
    };
    let wrong_layer_rate = if total > 0 {
        wrong_layer as f64 / total as f64
    } else {
        0.0
    };

    EvalRouteReport {
        schema_version: EVAL_ROUTE_REPORT_SCHEMA_VERSION.to_string(),
        authority: EVAL_ROUTE_AUTHORITY.to_string(),
        total_cases: total,
        passed,
        failed,
        route_accuracy,
        wrong_owner_rate,
        wrong_overlay_rate,
        wrong_layer_rate,
        failures,
    }
}

pub fn run_eval_route(
    cases_path: &Path,
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<EvalRouteReport, String> {
    let cases = load_eval_cases(cases_path)?;
    let records = load_records(runtime_path, manifest_path)?;
    Ok(evaluate_route_cases(
        &records,
        &cases.cases,
        runtime_path,
        manifest_path,
    ))
}

pub fn eval_route_contract() -> Value {
    json!({
        "schema_version": EVAL_ROUTE_REPORT_SCHEMA_VERSION,
        "authority": EVAL_ROUTE_AUTHORITY,
        "route_decision_schema_version": ROUTE_DECISION_SCHEMA_VERSION,
        "route_authority": ROUTE_AUTHORITY,
        "description": "Evaluates routing decisions against expected outcomes using live skill records from SKILL_ROUTING_RUNTIME.json.",
        "metrics": ["route_accuracy", "wrong_owner_rate", "wrong_overlay_rate", "wrong_layer_rate"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::SkillRecord;
    use std::collections::HashSet;

    fn make_record(
        slug: &str,
        layer: &str,
        owner: &str,
        gate: &str,
        hints: &[&str],
    ) -> SkillRecord {
        SkillRecord {
            slug: slug.to_string(),
            skill_path: Some(format!("skills/{slug}/SKILL.md")),
            layer: layer.to_string(),
            owner: owner.to_string(),
            gate: gate.to_string(),
            priority: "P1".to_string(),
            session_start: "preferred".to_string(),
            summary: format!("{slug} description"),
            slug_lower: slug.to_lowercase(),
            owner_lower: owner.to_lowercase(),
            gate_lower: gate.to_lowercase(),
            session_start_lower: "preferred".to_string(),
            gate_phrases: vec![],
            trigger_hints: hints.iter().map(|s| s.to_string()).collect(),
            name_tokens: HashSet::from([slug.to_string()]),
            keyword_tokens: HashSet::new(),
            alias_tokens: HashSet::new(),
            do_not_use_tokens: HashSet::new(),
            framework_alias_entrypoints: vec![],
        }
    }

    #[test]
    fn eval_route_all_correct_reports_full_accuracy() {
        let records = vec![
            make_record(
                "slides",
                "L3",
                "owner",
                "artifact",
                &["演示文稿", "ppt", "幻灯片"],
            ),
            make_record(
                "gitx",
                "L0",
                "owner",
                "none",
                &["提交代码", "git", "commit"],
            ),
        ];
        let cases = vec![
            EvalCasePayload {
                id: "c1".to_string(),
                category: "should-trigger".to_string(),
                task: "帮我做一个演示文稿".to_string(),
                expected_owner: Some("slides".to_string()),
                expected_layer: Some("L3".to_string()),
                ..Default::default()
            },
            EvalCasePayload {
                id: "c2".to_string(),
                category: "should-trigger".to_string(),
                task: "提交代码".to_string(),
                expected_owner: Some("gitx".to_string()),
                expected_layer: Some("L0".to_string()),
                ..Default::default()
            },
        ];
        let report = evaluate_route_cases(&records, &cases, None, None);
        assert_eq!(report.total_cases, 2);
        assert_eq!(report.passed, 2);
        assert_eq!(report.failed, 0);
        assert!((report.route_accuracy - 1.0).abs() < 0.001);
        assert!((report.wrong_owner_rate - 0.0).abs() < 0.001);
    }

    #[test]
    fn eval_route_wrong_owner_reports_failure() {
        let records = vec![make_record(
            "gitx",
            "L0",
            "owner",
            "none",
            &["提交代码", "git"],
        )];
        let cases = vec![EvalCasePayload {
            id: "c1".to_string(),
            category: "should-trigger".to_string(),
            task: "xyzzynomatch_12345_nonexistent".to_string(),
            expected_owner: Some("slides".to_string()),
            ..Default::default()
        }];
        let report = evaluate_route_cases(&records, &cases, None, None);
        assert_eq!(report.total_cases, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].field, "selected_skill");
        assert!((report.route_accuracy - 0.0).abs() < 0.001);
    }

    #[test]
    fn eval_route_forbidden_owner_detected() {
        let records = vec![
            make_record(
                "gitx",
                "L0",
                "owner",
                "none",
                &["提交代码", "git", "commit"],
            ),
            make_record("slides", "L3", "owner", "artifact", &["演示文稿", "ppt"]),
        ];
        let cases = vec![EvalCasePayload {
            id: "c1".to_string(),
            category: "should-not-trigger".to_string(),
            task: "提交代码".to_string(),
            expected_owner: Some("gitx".to_string()),
            forbidden_owners: vec!["gitx".to_string()],
            ..Default::default()
        }];
        let report = evaluate_route_cases(&records, &cases, None, None);
        assert_eq!(report.failed, 1);
        assert!(report.failures.iter().any(|f| f.field == "forbidden_owner"));
    }

    #[test]
    fn eval_route_empty_task_skipped() {
        let records = vec![make_record(
            "slides",
            "L3",
            "owner",
            "artifact",
            &["演示文稿"],
        )];
        let cases = vec![EvalCasePayload {
            id: "empty".to_string(),
            category: "should-trigger".to_string(),
            task: "   ".to_string(),
            expected_owner: Some("slides".to_string()),
            ..Default::default()
        }];
        let report = evaluate_route_cases(&records, &cases, None, None);
        assert_eq!(report.total_cases, 0);
    }

    #[test]
    fn eval_route_contract_has_metrics() {
        let payload = eval_route_contract();
        assert_eq!(payload["schema_version"], EVAL_ROUTE_REPORT_SCHEMA_VERSION);
        let metrics = payload["metrics"].as_array().expect("metrics array");
        assert!(metrics.iter().any(|v| v == "route_accuracy"));
        assert!(metrics.iter().any(|v| v == "wrong_owner_rate"));
    }
}
