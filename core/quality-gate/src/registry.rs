//! CheckerRegistry: scene → checker mapping, evaluate dispatch, and aggregate.

use std::collections::HashMap;

use crate::checker::GateChecker;
use crate::scene;
use crate::types::{CheckContext, CheckResult, Finding, GateVerdict, Severity};

/// Registry mapping scene identifiers to their registered checkers.
///
/// Populated at startup via `register()`. The `evaluate()` method runs
/// all checkers registered for the given scene and aggregates results.
pub struct CheckerRegistry {
    /// scene → ordered list of checkers.
    checkers: HashMap<&'static str, Vec<Box<dyn GateChecker>>>,
}

impl CheckerRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            checkers: HashMap::new(),
        }
    }

    /// Register a checker for a specific scene.
    ///
    /// Panics if `scene` is not a valid constant (see `scene::ALL`).
    /// Scene dispatch is determined by the registration call, not by the checker.
    pub fn register(&mut self, scene: &'static str, checker: Box<dyn GateChecker>) {
        assert!(
            scene::is_valid(scene),
            "CheckerRegistry::register: invalid scene '{scene}'",
        );
        self.checkers.entry(scene).or_default().push(checker);
    }

    /// Evaluate all checkers for the given scene, optionally filtered by sub_scene.
    ///
    /// When `ctx.sub_scene` is `Some`, only checkers whose `sub_scene_affinity()`
    /// matches (or returns `None` — meaning "all sub-scenes") are invoked.
    /// Returns an aggregated `GateVerdict`. Unknown/empty scenes produce a
    /// passed verdict with reason "no registered checkers".
    pub fn evaluate(&self, scene_str: &str, ctx: &CheckContext) -> GateVerdict {
        let norm_scene = scene::normalize(scene_str);

        // Detect scene normalization: if the input differs from the normalized
        // result, the caller passed an invalid scene. This is a routing bug.
        let scene_normalized = scene_str != norm_scene;

        let Some(checkers) = self.checkers.get(norm_scene) else {
            let mut advisories = Vec::new();
            if scene_normalized {
                advisories.push(Finding {
                    id: "scene_normalized".to_string(),
                    severity: Severity::Warning,
                    description: format!(
                        "scene '{scene_str}' is not a valid scene constant — normalized to '{norm_scene}'"
                    ),
                    location: None,
                    suggestion: Some(
                        "fix the scene string at the call site to use a valid scene constant".to_string(),
                    ),
                });
            }
            return GateVerdict {
                passed: true,
                scene: norm_scene.to_string(),
                checkers_ran: 0,
                blockers: vec![],
                advisories,
                reason: Some(format!("scene '{norm_scene}' has no registered checkers")),
            };
        };

        let mut results: Vec<CheckResult> = Vec::with_capacity(checkers.len());

        // Inject a synthetic finding if scene was normalized, so downstream
        // consumers can detect the routing anomaly.
        if scene_normalized {
            results.push(CheckResult {
                checker_id: "scene_routing".to_string(),
                passed: true,
                findings: vec![Finding {
                    id: "scene_normalized".to_string(),
                    severity: Severity::Warning,
                    description: format!(
                        "scene '{scene_str}' is not a valid scene constant — normalized to '{norm_scene}'"
                    ),
                    location: None,
                    suggestion: Some(
                        "fix the scene string at the call site to use a valid scene constant".to_string(),
                    ),
                }],
            });
        }

        for checker in checkers {
            // Sub-scene filtering (Wave 6): skip checkers with a mismatched affinity.
            if let Some(ref sub) = ctx.sub_scene
                && let Some(affinity) = checker.sub_scene_affinity()
                && affinity != sub.as_str()
            {
                continue;
            }

            let result = checker.check(ctx);
            if !result.findings.is_empty() || !result.passed {
                tracing::debug!(
                    checker_id = %result.checker_id,
                    passed = %result.passed,
                    findings = %result.findings.len(),
                    "checker result",
                );
            }
            results.push(result);
        }

        aggregate(&results, norm_scene)
    }
}

impl Default for CheckerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// --- aggregation ---

/// Aggregate multiple `CheckResult`s into a single `GateVerdict`.
///
/// Rules (§2.5):
///   - Any finding severity = P0   → gate fails (unconditional)
///   - Any finding severity = A/B → gate fails
///   - Checker self-judged `passed=false` with no findings → synthetic B blocker
///   - All findings ≤ Warning (or empty) → gate passes
///   - Anti-fraud gate (Stage 1) is separate — not handled here.
fn aggregate(results: &[CheckResult], scene: &str) -> GateVerdict {
    let checkers_ran = results.len();
    let mut blockers: Vec<Finding> = Vec::new();
    let mut advisories: Vec<Finding> = Vec::new();

    for r in results {
        // If checker self-judged failed but produced no findings,
        // synthesize a generic blocker so its verdict is not silently ignored.
        if !r.passed && r.findings.is_empty() {
            blockers.push(Finding {
                id: format!("{}_self_blocked", r.checker_id),
                severity: Severity::B,
                description: format!(
                    "checker '{}' judged the gate as failed but produced no specific findings",
                    r.checker_id,
                ),
                location: None,
                suggestion: None,
            });
        }

        for f in &r.findings {
            match f.severity {
                Severity::P0 | Severity::A | Severity::B => blockers.push(f.clone()),
                Severity::Warning | Severity::C => advisories.push(f.clone()),
            }
        }
    }

    let passed = blockers.is_empty();
    let reason = if !passed {
        Some(format!(
            "gate blocked: {} finding(s) at P0/A/B severity",
            blockers.len(),
        ))
    } else if !advisories.is_empty() {
        Some(format!(
            "gate passed with {} advisory finding(s)",
            advisories.len(),
        ))
    } else {
        None
    };

    GateVerdict {
        passed,
        scene: scene.to_string(),
        checkers_ran,
        blockers,
        advisories,
        reason,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::scene;
    use crate::types::Severity;

    struct DummyChecker {
        id: &'static str,
        result: CheckResult,
    }

    impl GateChecker for DummyChecker {
        fn id(&self) -> &'static str {
            self.id
        }
        fn description(&self) -> &'static str {
            "dummy checker for tests"
        }
        fn check(&self, _ctx: &CheckContext) -> CheckResult {
            self.result.clone()
        }
    }

    fn make_ctx(task_id: &str) -> CheckContext {
        CheckContext {
            scene: scene::GENERAL.to_string(),
            sub_scene: None,
            goal: "test".to_string(),
            round: 1,
            repo_root: std::path::PathBuf::from("."),
            task_id: task_id.to_string(),
            evidence_path: None,
            runtime_handle: None,
            output_data: None,
            evaluated_at: "2026-06-30T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_empty_registry_passes() {
        let registry = CheckerRegistry::new();
        let v = registry.evaluate(scene::GENERAL, &make_ctx("t1"));
        assert!(v.passed);
        assert_eq!(v.checkers_ran, 0);
    }

    #[test]
    fn test_unknown_scene_falls_back_to_general() {
        let mut registry = CheckerRegistry::new();
        registry.register(
            scene::GENERAL,
            Box::new(DummyChecker {
                id: "gen",
                result: CheckResult {
                    checker_id: "gen".to_string(),
                    passed: true,
                    findings: vec![],
                },
            }),
        );
        let v = registry.evaluate("nonexistent", &make_ctx("t1"));
        // Falls back to "general" which has the gen checker + scene_routing synthetic result
        assert_eq!(v.checkers_ran, 2);
        // Verify the scene normalization advisory is present
        assert!(v.advisories.iter().any(|f| f.id == "scene_normalized"));
    }

    #[test]
    fn test_p0_blocks_gate() {
        let mut registry = CheckerRegistry::new();
        registry.register(
            scene::GENERAL,
            Box::new(DummyChecker {
                id: "p0-checker",
                result: CheckResult {
                    checker_id: "p0-checker".to_string(),
                    passed: false,
                    findings: vec![Finding {
                        id: "f1".to_string(),
                        severity: Severity::P0,
                        description: "critical bug".to_string(),
                        location: None,
                        suggestion: None,
                    }],
                },
            }),
        );
        let v = registry.evaluate(scene::GENERAL, &make_ctx("t1"));
        assert!(!v.passed);
        assert_eq!(v.blockers.len(), 1);
        assert_eq!(v.blockers[0].severity, Severity::P0);
    }

    #[test]
    fn test_warning_only_passes() {
        let mut registry = CheckerRegistry::new();
        registry.register(
            scene::GENERAL,
            Box::new(DummyChecker {
                id: "warn-checker",
                result: CheckResult {
                    checker_id: "warn-checker".to_string(),
                    passed: true,
                    findings: vec![Finding {
                        id: "w1".to_string(),
                        severity: Severity::Warning,
                        description: "minor style issue".to_string(),
                        location: None,
                        suggestion: None,
                    }],
                },
            }),
        );
        let v = registry.evaluate(scene::GENERAL, &make_ctx("t1"));
        assert!(v.passed);
        assert_eq!(v.advisories.len(), 1);
    }

    #[test]
    fn test_empty_findings_passes() {
        let mut registry = CheckerRegistry::new();
        registry.register(
            scene::GENERAL,
            Box::new(DummyChecker {
                id: "empty",
                result: CheckResult {
                    checker_id: "empty".to_string(),
                    passed: true,
                    findings: vec![],
                },
            }),
        );
        let v = registry.evaluate(scene::GENERAL, &make_ctx("t1"));
        assert!(v.passed);
        assert_eq!(v.checkers_ran, 1);
        assert_eq!(v.blockers.len(), 0);
        assert_eq!(v.advisories.len(), 0);
    }

    #[test]
    fn test_mixed_severity_blockers_only() {
        let mut registry = CheckerRegistry::new();
        registry.register(
            scene::GENERAL,
            Box::new(DummyChecker {
                id: "mixed",
                result: CheckResult {
                    checker_id: "mixed".to_string(),
                    passed: false,
                    findings: vec![
                        Finding {
                            id: "b1".to_string(),
                            severity: Severity::B,
                            description: "blocker".to_string(),
                            location: None,
                            suggestion: None,
                        },
                        Finding {
                            id: "w1".to_string(),
                            severity: Severity::Warning,
                            description: "advice".to_string(),
                            location: None,
                            suggestion: None,
                        },
                    ],
                },
            }),
        );
        let v = registry.evaluate(scene::GENERAL, &make_ctx("t1"));
        assert!(!v.passed);
        assert_eq!(v.blockers.len(), 1);
        assert_eq!(v.advisories.len(), 1);
    }

    #[test]
    fn test_self_blocked_without_findings() {
        let mut registry = CheckerRegistry::new();
        registry.register(
            scene::GENERAL,
            Box::new(DummyChecker {
                id: "self-block",
                result: CheckResult {
                    checker_id: "self-block".to_string(),
                    passed: false,
                    findings: vec![], // no findings, but passed=false
                },
            }),
        );
        let v = registry.evaluate(scene::GENERAL, &make_ctx("t1"));
        assert!(!v.passed);
        assert_eq!(v.blockers.len(), 1);
        assert_eq!(v.blockers[0].id, "self-block_self_blocked");
        assert_eq!(v.blockers[0].severity, Severity::B);
    }
}
