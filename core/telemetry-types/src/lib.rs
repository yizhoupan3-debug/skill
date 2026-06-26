//! Shared telemetry event types used by framework-kernel (writer) and observer-rs (reader).
//!
//! This micro-crate is the **single source of truth** for `TelemetryEvent` and
//! `PredictionOutcomeCheck`. Previously both `framework-kernel::telemetry` and
//! `observer-rs::telemetry_journal` defined their own copies, which drifted
//! (framework-kernel's `RouteDecision` had extra fields like `latency_ms`,
//! `reasons`, `matched_tokens`, `parity_gate`, `candidates`).

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use serde::{Deserialize, Serialize};

/// Canonical telemetry event enum emitted by the runtime and consumed by
/// offline analysis tools (observer-rs audit/manifest).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TelemetryEvent {
    RouteDecision {
        task: String,
        skill: String,
        confidence: f32,
        reroute: bool,
        #[serde(default)]
        latency_ms: u64,
        #[serde(default)]
        reasons: Vec<String>,
        #[serde(default)]
        matched_tokens: usize,
        #[serde(default)]
        parity_gate: String,
        #[serde(default)]
        candidates: Vec<String>,
    },
    GoalTransition {
        from: String,
        to: String,
        task_id: String,
    },
    ToolCall {
        tool: String,
        duration_ms: u64,
        success: bool,
    },
    RfvRound {
        round: u32,
        verdict: String,
    },
    HookFired {
        hook_name: String,
        action: String,
    },
    DevExempt {
        path: String,
        action: String,
    },
    PredictionOutcome {
        task_id: String,
        matched: bool,
        predicted_verification_status: Option<String>,
        predicted_hypothesis: Option<String>,
        actual_verification_status: String,
        checks_summary: String,
        checks: Vec<PredictionOutcomeCheck>,
    },
}

/// A single check result within a `PredictionOutcome` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PredictionOutcomeCheck {
    pub rule: String,
    pub matched: bool,
    pub severity: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(event: TelemetryEvent) -> TelemetryEvent {
        let json = serde_json::to_string(&event).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn serde_roundtrip_route_decision() {
        let event = TelemetryEvent::RouteDecision {
            task: "audit-v9".into(),
            skill: "code-review".into(),
            confidence: 0.95,
            reroute: true,
            latency_ms: 42,
            reasons: vec!["high_priority".into()],
            matched_tokens: 128,
            parity_gate: "stable".into(),
            candidates: vec!["code-review".into(), "simplify".into()],
        };
        assert_eq!(event, roundtrip(event.clone()));
    }

    #[test]
    fn serde_roundtrip_goal_transition() {
        let event = TelemetryEvent::GoalTransition {
            from: "explore".into(),
            to: "implement".into(),
            task_id: "task-42".into(),
        };
        assert_eq!(event, roundtrip(event.clone()));
    }

    #[test]
    fn serde_roundtrip_tool_call() {
        let event = TelemetryEvent::ToolCall {
            tool: "bash".into(),
            duration_ms: 1500,
            success: true,
        };
        assert_eq!(event, roundtrip(event.clone()));
    }

    #[test]
    fn serde_roundtrip_rfv_round() {
        let event = TelemetryEvent::RfvRound {
            round: 3,
            verdict: "approved".into(),
        };
        assert_eq!(event, roundtrip(event.clone()));
    }

    #[test]
    fn serde_roundtrip_hook_fired() {
        let event = TelemetryEvent::HookFired {
            hook_name: "pre-commit".into(),
            action: "lint".into(),
        };
        assert_eq!(event, roundtrip(event.clone()));
    }

    #[test]
    fn serde_roundtrip_dev_exempt() {
        let event = TelemetryEvent::DevExempt {
            path: "/tmp/test".into(),
            action: "bypass".into(),
        };
        assert_eq!(event, roundtrip(event.clone()));
    }

    #[test]
    fn serde_roundtrip_prediction_outcome() {
        let event = TelemetryEvent::PredictionOutcome {
            task_id: "task-99".into(),
            matched: true,
            predicted_verification_status: Some("verified".into()),
            predicted_hypothesis: None,
            actual_verification_status: "verified".into(),
            checks_summary: "3/5 passed".into(),
            checks: vec![
                PredictionOutcomeCheck {
                    rule: "ineq_nonneg".into(),
                    matched: true,
                    severity: "error".into(),
                },
                PredictionOutcomeCheck {
                    rule: "type_safety".into(),
                    matched: false,
                    severity: "warning".into(),
                },
            ],
        };
        assert_eq!(event, roundtrip(event.clone()));
    }

    #[test]
    fn serde_default_fields() {
        let json = serde_json::json!({
            "kind": "route_decision",
            "task": "audit-v9",
            "skill": "code-review",
            "confidence": 0.8,
            "reroute": false,
        });
        let event: TelemetryEvent = serde_json::from_value(json).unwrap();
        match event {
            TelemetryEvent::RouteDecision {
                task,
                skill,
                confidence,
                reroute,
                latency_ms,
                reasons,
                matched_tokens,
                parity_gate,
                candidates,
            } => {
                assert_eq!(task, "audit-v9");
                assert_eq!(skill, "code-review");
                assert!((confidence - 0.8).abs() < f32::EPSILON);
                assert!(!reroute);
                assert_eq!(latency_ms, 0);
                assert!(reasons.is_empty());
                assert_eq!(matched_tokens, 0);
                assert_eq!(parity_gate, "");
                assert!(candidates.is_empty());
            }
            _ => panic!("expected RouteDecision"),
        }
    }

    #[test]
    fn serde_json_tag_naming() {
        let pairs: Vec<(TelemetryEvent, &str)> = vec![
            (
                TelemetryEvent::RouteDecision {
                    task: "t".into(),
                    skill: "s".into(),
                    confidence: 0.5,
                    reroute: false,
                    latency_ms: 0,
                    reasons: vec![],
                    matched_tokens: 0,
                    parity_gate: "".into(),
                    candidates: vec![],
                },
                "route_decision",
            ),
            (
                TelemetryEvent::GoalTransition {
                    from: "a".into(),
                    to: "b".into(),
                    task_id: "t".into(),
                },
                "goal_transition",
            ),
            (
                TelemetryEvent::ToolCall {
                    tool: "bash".into(),
                    duration_ms: 0,
                    success: true,
                },
                "tool_call",
            ),
            (
                TelemetryEvent::RfvRound {
                    round: 1,
                    verdict: "ok".into(),
                },
                "rfv_round",
            ),
            (
                TelemetryEvent::HookFired {
                    hook_name: "h".into(),
                    action: "a".into(),
                },
                "hook_fired",
            ),
            (
                TelemetryEvent::DevExempt {
                    path: "/p".into(),
                    action: "a".into(),
                },
                "dev_exempt",
            ),
            (
                TelemetryEvent::PredictionOutcome {
                    task_id: "t".into(),
                    matched: true,
                    predicted_verification_status: None,
                    predicted_hypothesis: None,
                    actual_verification_status: "ok".into(),
                    checks_summary: "".into(),
                    checks: vec![],
                },
                "prediction_outcome",
            ),
        ];
        for (event, expected_kind) in pairs {
            let value = serde_json::to_value(&event).unwrap();
            assert_eq!(
                value.get("kind").and_then(|v| v.as_str()),
                Some(expected_kind),
                "kind mismatch for {expected_kind}",
            );
        }
    }

    #[test]
    fn prediction_outcome_check_roundtrip() {
        let check = PredictionOutcomeCheck {
            rule: "ineq_nonneg".into(),
            matched: true,
            severity: "error".into(),
        };
        let json = serde_json::to_string(&check).unwrap();
        let deserialized: PredictionOutcomeCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(check, deserialized);
    }

    #[test]
    fn partial_eq_same_variants_equal() {
        let a = TelemetryEvent::ToolCall {
            tool: "bash".into(),
            duration_ms: 100,
            success: true,
        };
        let b = TelemetryEvent::ToolCall {
            tool: "bash".into(),
            duration_ms: 100,
            success: true,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn partial_eq_diff_variants_not_equal() {
        let a = TelemetryEvent::ToolCall {
            tool: "bash".into(),
            duration_ms: 100,
            success: true,
        };
        let b = TelemetryEvent::RfvRound {
            round: 1,
            verdict: "ok".into(),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn partial_eq_diff_fields_not_equal() {
        let a = TelemetryEvent::ToolCall {
            tool: "bash".into(),
            duration_ms: 100,
            success: true,
        };
        let b = TelemetryEvent::ToolCall {
            tool: "read".into(),
            duration_ms: 100,
            success: true,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn prediction_outcome_optional_fields_null() {
        let json = serde_json::json!({
            "kind": "prediction_outcome",
            "task_id": "t1",
            "matched": false,
            "actual_verification_status": "failed",
            "checks_summary": "all failed",
            "checks": [],
        });
        let event: TelemetryEvent = serde_json::from_value(json).unwrap();
        match event {
            TelemetryEvent::PredictionOutcome {
                task_id,
                matched,
                predicted_verification_status,
                predicted_hypothesis,
                actual_verification_status,
                checks_summary: _,
                checks,
            } => {
                assert_eq!(task_id, "t1");
                assert!(!matched);
                assert_eq!(predicted_verification_status, None);
                assert_eq!(predicted_hypothesis, None);
                assert_eq!(actual_verification_status, "failed");
                assert!(checks.is_empty());
            }
            _ => panic!("expected PredictionOutcome"),
        }
    }
}
