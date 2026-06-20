//! 收敛判定 — 检查审稿循环是否已达到收敛条件。
//!
//! 收敛条件：连续 `consecutive_stable_required` 轮无新 P0/A/B 级 findings，
//! 且已至少完成 `min_rounds` 轮。

use crate::types::{ConvergenceState, Finding};

/// Check if the loop has converged given the current state and this round's findings.
///
/// Returns true if:
/// - current_round >= min_rounds (hard floor met)
/// - This round has no P0/A/B findings (round is "stable")
/// - consecutive_stable_count (after this round) >= consecutive_stable_required
pub fn check_convergence(state: &ConvergenceState, findings: &[Finding]) -> bool {
    let has_blocking = findings.iter().any(|f| f.severity.blocks_convergence());

    if has_blocking {
        // Not converged — blocking findings reset the stable counter
        return false;
    }

    // This round is stable. Check if we've accumulated enough consecutive stable rounds.
    let new_stable = state.consecutive_stable_count + 1;
    state.current_round >= state.min_rounds && new_stable >= state.consecutive_stable_required
}

/// Check if the loop is at the hard ceiling (max_rounds reached).
pub fn is_at_ceiling(state: &ConvergenceState) -> bool {
    state.current_round >= state.max_rounds
}

/// Determine whether the supervisor should be allowed to close.
/// Enforces both min_rounds and consecutive_stable_required.
pub fn can_close(state: &ConvergenceState, findings: &[Finding]) -> CloseDecision {
    if is_at_ceiling(state) {
        return CloseDecision {
            allowed: true,
            reason: "max_rounds reached — hard ceiling".to_string(),
        };
    }

    if state.current_round < state.min_rounds {
        return CloseDecision {
            allowed: false,
            reason: format!(
                "min_rounds not met: current={} < min={}",
                state.current_round, state.min_rounds
            ),
        };
    }

    let has_blocking = findings.iter().any(|f| f.severity.blocks_convergence());
    let new_stable = if has_blocking {
        0
    } else {
        state.consecutive_stable_count + 1
    };

    if new_stable < state.consecutive_stable_required {
        return CloseDecision {
            allowed: false,
            reason: format!(
                "convergence not met: stable_count={} < required={} (blocking_findings={})",
                new_stable,
                state.consecutive_stable_required,
                findings.iter().filter(|f| f.severity.blocks_convergence()).count()
            ),
        };
    }

    CloseDecision {
        allowed: true,
        reason: "converged: min_rounds met and consecutive_stable_count sufficient".to_string(),
    }
}

/// Decision from the convergence checker.
#[derive(Debug, Clone)]
pub struct CloseDecision {
    pub allowed: bool,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Severity;

    fn make_state(current: u64, min: u64, stable: u64, stable_req: u64, max: u64) -> ConvergenceState {
        ConvergenceState {
            min_rounds: min,
            consecutive_stable_required: stable_req,
            consecutive_stable_count: stable,
            max_rounds: max,
            current_round: current,
        }
    }

    fn make_finding(severity: Severity) -> Finding {
        Finding {
            id: "f1".to_string(),
            severity,
            dimension: "test".to_string(),
            location: "§1".to_string(),
            description: "test finding".to_string(),
            suggestion: None,
        }
    }

    #[test]
    fn test_converged() {
        let state = make_state(5, 3, 1, 2, 10);
        let findings: Vec<Finding> = vec![];
        assert!(check_convergence(&state, &findings));
    }

    #[test]
    fn test_not_converged_below_min() {
        let state = make_state(2, 3, 2, 2, 10);
        let findings: Vec<Finding> = vec![];
        assert!(!check_convergence(&state, &findings));
    }

    #[test]
    fn test_not_converged_blocking_findings() {
        let state = make_state(5, 3, 1, 2, 10);
        let findings = vec![make_finding(Severity::A)];
        assert!(!check_convergence(&state, &findings));
    }

    #[test]
    fn test_can_close_at_ceiling() {
        let state = make_state(10, 5, 0, 2, 10);
        let findings = vec![make_finding(Severity::A)]; // blocking, but ceiling overrides
        let decision = can_close(&state, &findings);
        assert!(decision.allowed);
    }

    #[test]
    fn test_cannot_close_below_min() {
        let state = make_state(1, 5, 0, 2, 10);
        let findings: Vec<Finding> = vec![];
        let decision = can_close(&state, &findings);
        assert!(!decision.allowed);
    }
}
