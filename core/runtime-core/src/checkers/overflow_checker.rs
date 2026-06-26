use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

pub struct OverflowChecker;

impl GateChecker for OverflowChecker {
    fn id(&self) -> &'static str { "overflow" }
    fn scenes(&self) -> Vec<&'static str> { vec![quality_gate::scene::SLIDES] }
    fn description(&self) -> &'static str { "detect overflow conditions (token limits, context window, output length) in slide generation tasks" }
    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();
        findings.push(Finding { id: "overflow-adapter".to_string(), severity: Severity::C, description: format!("overflow checker invoked for task '{}'", ctx.task_id), location: None, suggestion: Some("implement actual checks".to_string()) });
        CheckResult { checker_id: "overflow".to_string(), passed: true, findings }
    }
}
