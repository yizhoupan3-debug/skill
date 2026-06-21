//! Exit gate evaluator trait for dual-gate goal completion.
//!
//! **v9 退出门架构**：Goal complete 时必须通过两个独立门：
//! 1. **核查门 (Verification Gate)** — 防幻觉/防欺诈 (R19-R21)
//! 2. **质量门 (Quality Gate)** — 质量检验账本 (supervisor_decision + convergence)
//!
//! 注意：此 trait 不同于 `GoalCompletionGates` struct（`task_state.rs:788`）。
//! `GoalCompletionGates` 是 `completion_gates` JSON 字段的解析器；
//! `ExitGateEvaluator` 是退出门评估的抽象接口。

use std::path::Path;

/// Exit gate evaluation result.
#[derive(Debug, Clone)]
pub struct GateResult {
    /// Whether the gate passed.
    pub passed: bool,
    /// Gate type identifier.
    pub gate_type: GateType,
    /// Human-readable summary.
    pub summary: String,
    /// Blocking reason if `passed == false`.
    pub block_reason: Option<String>,
    /// Evidence references supporting this evaluation.
    pub evidence_refs: Vec<String>,
}

/// Which exit gate is being evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateType {
    /// Verification gate (防幻觉/防欺诈, R19-R21).
    Verification,
    /// Quality gate (原 RFV, supervisor_decision + convergence).
    Quality,
}

/// Trait for evaluating exit gates before Goal completion.
///
/// Implementors evaluate whether a Goal has satisfied the conditions
/// for each gate type. Both gates must pass for Goal to complete.
pub trait ExitGateEvaluator: Send + Sync {
    /// Evaluate the verification gate (R19-R21).
    ///
    /// Checks:
    /// - R19: `commands_run` ↔ `EVIDENCE_INDEX` cross-validation
    /// - R20: `fs::metadata` independent file verification
    /// - R21: `source=model_manual` flagged as warning
    fn check_verification_gate(
        &self,
        repo_root: &Path,
        task_id: &str,
    ) -> GateResult;

    /// Evaluate the quality gate (原 RFV gate).
    ///
    /// Checks:
    /// - `loop_status == closed` with convergence
    /// - `supervisor_decision` recorded
    /// - Cross-link to EVIDENCE_INDEX
    fn check_quality_gate(
        &self,
        repo_root: &Path,
        task_id: &str,
    ) -> GateResult;
}

/// No-op evaluator for testing or when gates are disabled.
pub struct NoopExitGateEvaluator;

impl ExitGateEvaluator for NoopExitGateEvaluator {
    fn check_verification_gate(
        &self,
        _repo_root: &Path,
        _task_id: &str,
    ) -> GateResult {
        GateResult {
            passed: true,
            gate_type: GateType::Verification,
            summary: "verification gate: no-op (always passes)".into(),
            block_reason: None,
            evidence_refs: vec![],
        }
    }

    fn check_quality_gate(
        &self,
        _repo_root: &Path,
        _task_id: &str,
    ) -> GateResult {
        GateResult {
            passed: true,
            gate_type: GateType::Quality,
            summary: "quality gate: no-op (always passes)".into(),
            block_reason: None,
            evidence_refs: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn noop_evaluator_always_passes() {
        let eval = NoopExitGateEvaluator;
        let root = PathBuf::from("/tmp/test");
        let vr = eval.check_verification_gate(&root, "task-1");
        assert!(vr.passed);
        assert_eq!(vr.gate_type, GateType::Verification);

        let qr = eval.check_quality_gate(&root, "task-1");
        assert!(qr.passed);
        assert_eq!(qr.gate_type, GateType::Quality);
    }

    #[test]
    fn gate_types_are_distinct() {
        assert_ne!(GateType::Verification, GateType::Quality);
    }
}
