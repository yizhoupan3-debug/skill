//! Shared telemetry event types used by framework-kernel (writer) and evolution-rs (reader).
//!
//! This micro-crate is the **single source of truth** for `TelemetryEvent` and
//! `PredictionOutcomeCheck`. Previously both `framework-kernel::telemetry` and
//! `evolution-rs::telemetry_journal` defined their own copies, which drifted
//! (framework-kernel's `RouteDecision` had extra fields like `latency_ms`,
//! `reasons`, `matched_tokens`, `parity_gate`, `candidates`).

use serde::{Deserialize, Serialize};

/// Canonical telemetry event enum emitted by the runtime and consumed by
/// offline analysis tools (evolution-rs audit/manifest).
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
