// NOTE: v9 路线图预留 — ExitGateEvaluator/NoopExitGateEvaluator 当前无外部引用。
// 原核心类型已从 core-state-types 移除（零调用方），本地保留以备 v9 启用。
//! Exit gate evaluator trait for dual-gate goal completion.
//!
//! **v9 退出门架构**：Goal complete 时必须通过两个独立门：
//! 1. **核查门 (Verification Gate)** — 防幻觉/防欺诈 (R19-R21)
//! 2. **质量门 (Quality Gate)** — 质量检验账本 (supervisor_decision + convergence)

use std::path::Path;

#[derive(Debug, Clone)]
pub struct GateResult {
    pub passed: bool,
    pub gate_type: GateType,
    pub summary: String,
    pub block_reason: Option<String>,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateType {
    Verification,
    Quality,
}

pub trait ExitGateEvaluator: Send + Sync {
    fn check_verification_gate(&self, repo_root: &Path, task_id: &str) -> GateResult;
    fn check_quality_gate(&self, repo_root: &Path, task_id: &str) -> GateResult;
}

pub struct NoopExitGateEvaluator;

impl ExitGateEvaluator for NoopExitGateEvaluator {
    fn check_verification_gate(&self, _repo_root: &Path, _task_id: &str) -> GateResult {
        GateResult {
            passed: true,
            gate_type: GateType::Verification,
            summary: "verification gate: no-op (always passes)".into(),
            block_reason: None,
            evidence_refs: vec![],
        }
    }
    fn check_quality_gate(&self, _repo_root: &Path, _task_id: &str) -> GateResult {
        GateResult {
            passed: true,
            gate_type: GateType::Quality,
            summary: "quality gate: no-op (always passes)".into(),
            block_reason: None,
            evidence_refs: vec![],
        }
    }
}
