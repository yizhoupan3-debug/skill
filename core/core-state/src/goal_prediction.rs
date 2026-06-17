//! Optional GOAL_STATE prediction (EV-6): `extra.prediction` for closeout dry-run verification.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Prediction attached to a macro goal; stored under `GOAL_STATE.extra.prediction`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GoalStatePrediction {
    /// Expected `closeout_record.verification_status` at task completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_verification_status: Option<String>,
    /// Falsifiable hypothesis the closeout `summary` should reflect (substring, case-insensitive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hypothesis: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PredictionVerification {
    pub matched: bool,
    pub rule: String,
    pub detail: String,
    /// Dry-run default: `warn` — does not block `closeout_allowed`.
    pub severity: String,
}

const ALLOWED_STATUSES: &[&str] = &["passed", "failed", "partial", "not_run"];

/// Read prediction from `extra.prediction`, then `metadata.prediction`, then top-level `prediction`.
pub fn read_goal_prediction(goal_state: &Value) -> Option<GoalStatePrediction> {
    for pred in [
        goal_state.get("extra").and_then(|e| e.get("prediction")),
        goal_state.get("metadata").and_then(|m| m.get("prediction")),
        goal_state.get("prediction"),
    ].into_iter().flatten() {
        if pred.is_null() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_value::<GoalStatePrediction>(pred.clone())
            && prediction_is_nonempty(&parsed) {
                return Some(parsed);
            }
    }
    None
}

fn prediction_is_nonempty(pred: &GoalStatePrediction) -> bool {
    pred.expected_verification_status
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty())
        || pred
            .hypothesis
            .as_deref()
            .map(str::trim)
            .is_some_and(|s| !s.is_empty())
}

/// Compare a goal prediction against closeout outcome fields (minimal dry-run).
pub fn verify_prediction_against_closeout(
    prediction: &GoalStatePrediction,
    verification_status: &str,
    summary: &str,
) -> Vec<PredictionVerification> {
    let mut out = Vec::new();
    let actual_status = verification_status.trim().to_ascii_lowercase();
    if let Some(expected) = prediction
        .expected_verification_status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let expected_lower = expected.to_ascii_lowercase();
        if ALLOWED_STATUSES.contains(&expected_lower.as_str()) && actual_status != expected_lower {
            out.push(PredictionVerification {
                matched: false,
                rule: "prediction_verification_status_mismatch".to_string(),
                detail: format!(
                    "GOAL_STATE.extra.prediction expected verification_status={expected_lower}, closeout has {actual_status}"
                ),
                severity: "warn".to_string(),
            });
        } else if actual_status == expected_lower {
            out.push(PredictionVerification {
                matched: true,
                rule: "prediction_verification_status_match".to_string(),
                detail: format!("prediction matched verification_status={actual_status}"),
                severity: "info".to_string(),
            });
        }
    }
    if let Some(hypothesis) = prediction
        .hypothesis
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let summary_lc = summary.to_ascii_lowercase();
        let needle = hypothesis.to_ascii_lowercase();
        if summary_lc.contains(&needle) {
            out.push(PredictionVerification {
                matched: true,
                rule: "prediction_hypothesis_reflected".to_string(),
                detail: format!("closeout summary reflects hypothesis: {hypothesis}"),
                severity: "info".to_string(),
            });
        } else {
            out.push(PredictionVerification {
                matched: false,
                rule: "prediction_hypothesis_not_reflected".to_string(),
                detail: format!(
                    "GOAL_STATE.extra.prediction hypothesis not found in closeout summary: {hypothesis}"
                ),
                severity: "warn".to_string(),
            });
        }
    }
    out
}

/// Merge optional `extra` / `prediction` from a goal-drive payload into a GOAL_STATE object.
pub fn merge_prediction_from_payload(obj: &mut serde_json::Map<String, Value>, payload: &Value) {
    let prediction_value = payload
        .get("extra")
        .and_then(|e| e.get("prediction"))
        .or_else(|| payload.get("prediction"));
    let Some(pred) = prediction_value else {
        if let Some(extra) = payload.get("extra").and_then(Value::as_object)
            && !extra.is_empty() {
                let entry = obj
                    .entry("extra".to_string())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()));
                if let Some(target) = entry.as_object_mut() {
                    for (k, v) in extra {
                        if k != "prediction" {
                            target.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
        return;
    };
    let entry = obj
        .entry("extra".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if let Some(extra) = entry.as_object_mut() {
        extra.insert("prediction".to_string(), pred.clone());
    }
    if let Some(extra) = payload.get("extra").and_then(Value::as_object)
        && let Some(target) = obj.get_mut("extra").and_then(Value::as_object_mut) {
            for (k, v) in extra {
                if k != "prediction" {
                    target.insert(k.clone(), v.clone());
                }
            }
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn read_prediction_from_extra() {
        let state = json!({
            "extra": {
                "prediction": {
                    "expected_verification_status": "passed",
                    "hypothesis": "router-rs green"
                }
            }
        });
        let pred = read_goal_prediction(&state).expect("prediction");
        assert_eq!(pred.expected_verification_status.as_deref(), Some("passed"));
        assert_eq!(pred.hypothesis.as_deref(), Some("router-rs green"));
    }

    #[test]
    fn verify_status_mismatch_is_warn() {
        let pred = GoalStatePrediction {
            expected_verification_status: Some("passed".to_string()),
            hypothesis: None,
        };
        let checks = verify_prediction_against_closeout(&pred, "failed", "done");
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].rule, "prediction_verification_status_mismatch");
        assert_eq!(checks[0].severity, "warn");
        assert!(!checks[0].matched);
    }

    #[test]
    fn verify_hypothesis_reflected() {
        let pred = GoalStatePrediction {
            expected_verification_status: None,
            hypothesis: Some("965 pass".to_string()),
        };
        let checks =
            verify_prediction_against_closeout(&pred, "passed", "cargo test: 965 pass / 0 fail");
        assert!(
            checks
                .iter()
                .any(|c| c.rule == "prediction_hypothesis_reflected")
        );
    }
}
