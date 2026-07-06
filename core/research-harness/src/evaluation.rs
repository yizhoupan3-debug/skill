//! Solution evaluation engine — compares current vs candidate implementations
//! across multiple dimensions: functionality, performance, integration cost, maintenance.
//!
//! # Workflow
//!
//! 1. Accept baseline (current) and candidate specs with their respective templates + params.
//! 2. Run both via the smoke experiment engine to collect objective metrics.
//! 3. Perform gap analysis on functionality coverage.
//! 4. Estimate integration cost based on API surface delta.
//! 5. Produce a structured verdict ("replace" / "conditional" / "reject").

use crate::smoke::{self, ExperimentResult};
use crate::smoke_cache::ExperimentCache;
use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;

// ── Public types ──

/// Specifies one "solution" (baseline or candidate).
#[derive(Debug, Clone)]
pub struct SolutionSpec {
    /// Human-readable name (e.g., "current classifier", "candidate v2").
    pub name: String,
    /// Template filename in `templates/` (must be executable).
    pub template: String,
    /// Experiment parameters for this solution.
    pub params: HashMap<String, String>,
    /// Known functionality points this solution covers.
    pub capabilities: Vec<String>,
}

/// A dimension to evaluate.
#[derive(Debug, Clone)]
pub struct EvalDimension {
    pub name: String,
    /// Higher is better for this dimension.
    pub higher_is_better: bool,
    /// Weight in the final score (0..1, sum across dims can be >1).
    pub weight: f64,
}

/// Integration cost estimate.
#[derive(Debug, Clone)]
pub struct IntegrationCost {
    /// Description of integration effort.
    pub description: String,
    /// Estimated person-days.
    pub person_days: f64,
    /// Risk level.
    pub risk: String,
}

/// Evaluation verdict.
#[derive(Debug, Clone)]
pub struct EvalVerdict {
    pub recommendation: String,
    pub confidence: f64,
    pub reasoning: Vec<String>,
}

/// Complete evaluation result.
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    pub baseline_name: String,
    pub candidate_name: String,
    pub baseline_metrics: HashMap<String, f64>,
    pub candidate_metrics: HashMap<String, f64>,
    pub dimension_scores: Vec<DimensionScore>,
    pub coverage_gap: CoverageGap,
    pub integration_cost: IntegrationCost,
    pub verdict: EvalVerdict,
}

/// Per-dimension comparison.
#[derive(Debug, Clone)]
pub struct DimensionScore {
    pub dimension: String,
    pub baseline: f64,
    pub candidate: f64,
    pub delta: f64,
    pub winner: String,
}

/// Functionality coverage gap analysis.
#[derive(Debug, Clone)]
pub struct CoverageGap {
    pub baseline_only: Vec<String>,
    pub candidate_only: Vec<String>,
    pub shared: Vec<String>,
    pub gap_score: f64,
}

/// Configuration for an evaluation.
#[derive(Debug, Clone)]
pub struct EvaluationConfig {
    /// Baseline (current) solution spec.
    pub baseline: SolutionSpec,
    /// Candidate solution spec.
    pub candidate: SolutionSpec,
    /// Evaluation dimensions to score.
    pub dimensions: Vec<EvalDimension>,
    /// Concurrency for parallel experiments.
    pub concurrency: usize,
    /// Per-experiment timeout.
    pub timeout_ms: u64,
    /// Bypass cache.
    pub no_cache: bool,
}

// ── Public entry point ──

/// Run a solution evaluation (current vs candidate).
///
/// Runs experiments for both solutions, compares metrics, analyzes capability gaps,
/// estimates integration cost, and produces a structured verdict.
pub fn run_evaluation(repo_root: &Path, config: &EvaluationConfig) -> Result<EvaluationResult, FrameworkError> {
    if config.baseline.template.is_empty() || config.candidate.template.is_empty() {
        return Err(FrameworkError::validation("both baseline and candidate templates must be non-empty"));
    }
    if config.dimensions.is_empty() {
        return Err(FrameworkError::validation("at least one evaluation dimension is required"));
    }

    let concurrency = config.concurrency.max(1).min(32);
    let timeout_ms = config.timeout_ms.max(100);
    let artifacts_dir = repo_root.join("artifacts/research-log/smoke");

    // Build experiment runs for both solutions
    let baseline_path = repo_root.join("templates").join(&config.baseline.template);
    let candidate_path = repo_root.join("templates").join(&config.candidate.template);

    ensure_template_exists(&baseline_path, &config.baseline.template)?;
    ensure_template_exists(&candidate_path, &config.candidate.template)?;

    let runs = vec![
        smoke::ExperimentRun {
            run_id: format!("{}-baseline", config.baseline.template),
            template_name: config.baseline.template.clone(),
            template_path: baseline_path,
            params: config.baseline.params.clone(),
        },
        smoke::ExperimentRun {
            run_id: format!("{}-candidate", config.candidate.template),
            template_name: config.candidate.template.clone(),
            template_path: candidate_path,
            params: config.candidate.params.clone(),
        },
    ];

    let cache = ExperimentCache::new(&artifacts_dir, config.no_cache);
    let results = crate::smoke::run_experiments(&runs, timeout_ms, concurrency, &cache, &artifacts_dir);

    if results.len() < 2 {
        return Err(FrameworkError::not_found(
            "evaluation: one or both experiments did not produce results",
        ));
    }

    // Extract metrics
    let baseline_metrics = extract_numeric_metrics(&results[0].result);
    let candidate_metrics = extract_numeric_metrics(&results[1].result);

    // Per-dimension scoring
    let mut dimension_scores = Vec::with_capacity(config.dimensions.len());
    for dim in &config.dimensions {
        let b = baseline_metrics.get(&dim.name).copied().unwrap_or(0.0);
        let c = candidate_metrics.get(&dim.name).copied().unwrap_or(0.0);
        let delta = c - b;
        let winner = if dim.higher_is_better {
            if delta > 0.0 { config.candidate.name.clone() }
            else if delta < 0.0 { config.baseline.name.clone() }
            else { "tie".into() }
        } else {
            if delta < 0.0 { config.candidate.name.clone() }
            else if delta > 0.0 { config.baseline.name.clone() }
            else { "tie".into() }
        };
        dimension_scores.push(DimensionScore {
            dimension: dim.name.clone(),
            baseline: b,
            candidate: c,
            delta,
            winner,
        });
    }

    // Coverage gap analysis
    let coverage_gap = analyze_coverage(&config.baseline.capabilities, &config.candidate.capabilities);

    // Integration cost estimation based on capability delta
    let integration_cost = estimate_integration_cost(&coverage_gap);

    // Weighted verdict
    let verdict = compute_verdict(&dimension_scores, &config.dimensions, &coverage_gap, &integration_cost);

    Ok(EvaluationResult {
        baseline_name: config.baseline.name.clone(),
        candidate_name: config.candidate.name.clone(),
        baseline_metrics,
        candidate_metrics,
        dimension_scores,
        coverage_gap,
        integration_cost,
        verdict,
    })
}

// ── Template validation ──

fn ensure_template_exists(path: &Path, name: &str) -> Result<(), FrameworkError> {
    if !path.exists() {
        return Err(FrameworkError::not_found(format!(
            "template not found: {name} (looked at {})",
            path.display(),
        )));
    }
    Ok(())
}

// ── Metric extraction ──

fn extract_numeric_metrics(result: &Value) -> HashMap<String, f64> {
    let obj = match result.as_object() {
        Some(o) => o,
        None => return HashMap::new(),
    };
    let mut out = HashMap::new();
    for (k, v) in obj {
        if let Some(n) = v.as_f64() {
            out.insert(k.clone(), n);
        }
    }
    out
}

// ── Coverage analysis ──

fn analyze_coverage(baseline: &[String], candidate: &[String]) -> CoverageGap {
    let b_set: HashSet<&str> = baseline.iter().map(String::as_str).collect();
    let c_set: HashSet<&str> = candidate.iter().map(String::as_str).collect();

    let baseline_only: Vec<String> = b_set.difference(&c_set).map(|s| s.to_string()).collect();
    let candidate_only: Vec<String> = c_set.difference(&b_set).map(|s| s.to_string()).collect();
    let shared: Vec<String> = b_set.intersection(&c_set).map(|s| s.to_string()).collect();

    let total = baseline.len().max(candidate.len()).max(1);
    let gap_score = baseline_only.len() as f64 / total as f64;

    CoverageGap {
        baseline_only,
        candidate_only,
        shared,
        gap_score,
    }
}

// ── Integration cost estimation ──

fn estimate_integration_cost(gap: &CoverageGap) -> IntegrationCost {
    // Heuristic: each missing capability = ~1 person-day, plus API adaptation overhead
    let missing = gap.baseline_only.len() as f64;
    let extra = gap.candidate_only.len() as f64;
    let base_days = missing * 0.8 + extra * 0.5;
    let person_days = base_days.max(0.5);

    let risk = if person_days < 2.0 {
        "low"
    } else if person_days < 4.0 {
        "medium"
    } else {
        "high"
    };

    let description = format!(
        "Baseline gap: {} missing capabilities, Candidate extras: {} (est. {:.0} person-days)",
        gap.baseline_only.len(),
        gap.candidate_only.len(),
        person_days,
    );

    IntegrationCost {
        description,
        person_days,
        risk: risk.into(),
    }
}

// ── Verdict computation ──

fn compute_verdict(
    scores: &[DimensionScore],
    dimensions: &[EvalDimension],
    coverage: &CoverageGap,
    cost: &IntegrationCost,
) -> EvalVerdict {
    let mut total_weight = 0.0_f64;
    let mut weighted_win = 0.0_f64;

    for ds in scores {
        if let Some(dim) = dimensions.iter().find(|d| d.name == ds.dimension) {
            total_weight += dim.weight;
            if ds.winner != "tie" {
                weighted_win += dim.weight;
            }
        }
    }

    let win_ratio = if total_weight > 0.0 { weighted_win / total_weight } else { 0.0 };
    let coverage_penalty = coverage.gap_score;
    let cost_penalty = match cost.risk.as_str() {
        "low" => 0.0,
        "medium" => 0.2,
        _ => 0.4,
    };

    let score = win_ratio * (1.0 - coverage_penalty) * (1.0 - cost_penalty);

    let (recommendation, confidence) = if score > 0.6 {
        ("replace", score)
    } else if score > 0.3 {
        ("conditional", score)
    } else {
        ("reject", score)
    };

    let mut reasoning = Vec::new();
    reasoning.push(format!(
        "Dimension win ratio: {:.1}% of weighted dims favor the candidate",
        win_ratio * 100.0,
    ));
    reasoning.push(format!(
        "Coverage gap: {} baseline capabilities missing in candidate ({:.0}%)",
        coverage.baseline_only.len(),
        coverage.gap_score * 100.0,
    ));
    reasoning.push(format!(
        "Integration cost: {:.0} person-days ({})",
        cost.person_days, cost.risk,
    ));

    if coverage.baseline_only.is_empty() {
        reasoning.push("All baseline capabilities are covered by the candidate ✓".into());
    } else {
        reasoning.push(format!(
            "Missing baseline capabilities: {}",
            coverage.baseline_only.join(", "),
        ));
    }

    EvalVerdict {
        recommendation: recommendation.into(),
        confidence,
        reasoning,
    }
}

/// Helper: format an EvaluationResult as a JSON value for MCP response.
pub fn evaluation_to_json(result: &EvaluationResult) -> Value {
    json!({
        "baseline": result.baseline_name,
        "candidate": result.candidate_name,
        "metrics": {
            "baseline": result.baseline_metrics,
            "candidate": result.candidate_metrics,
        },
        "dimensions": result.dimension_scores.iter().map(|d| json!({
            "name": d.dimension,
            "baseline": d.baseline,
            "candidate": d.candidate,
            "delta": d.delta,
            "winner": d.winner,
        })).collect::<Vec<_>>(),
        "coverage": {
            "shared": result.coverage_gap.shared,
            "baseline_only": result.coverage_gap.baseline_only,
            "candidate_only": result.coverage_gap.candidate_only,
            "gap_score": result.coverage_gap.gap_score,
        },
        "integration_cost": {
            "description": result.integration_cost.description,
            "person_days": result.integration_cost.person_days,
            "risk": result.integration_cost.risk,
        },
        "verdict": {
            "recommendation": result.verdict.recommendation,
            "confidence": result.verdict.confidence,
            "reasoning": result.verdict.reasoning,
        }
    })
}

// ── Tests ──

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn coverage_analysis_full_overlap() {
        let b = vec!["a".into(), "b".into()];
        let c = vec!["a".into(), "b".into()];
        let gap = analyze_coverage(&b, &c);
        assert_eq!(gap.shared.len(), 2);
        assert_eq!(gap.baseline_only.len(), 0);
        assert_eq!(gap.candidate_only.len(), 0);
        assert_eq!(gap.gap_score, 0.0);
    }

    #[test]
    fn coverage_analysis_partial_gap() {
        let b = vec!["a".into(), "b".into(), "c".into()];
        let c = vec!["a".into()];
        let gap = analyze_coverage(&b, &c);
        assert_eq!(gap.shared.len(), 1);
        assert_eq!(gap.baseline_only.len(), 2); // b, c
        assert!(gap.gap_score > 0.5);
    }

    #[test]
    fn coverage_analysis_empty_candidate() {
        let b = vec!["a".into()];
        let c: Vec<String> = vec![];
        let gap = analyze_coverage(&b, &c);
        assert_eq!(gap.baseline_only.len(), 1);
        assert_eq!(gap.gap_score, 1.0);
    }

    #[test]
    fn integration_cost_low_risk() {
        let gap = CoverageGap {
            baseline_only: vec![],
            candidate_only: vec!["extra".into()],
            shared: vec!["a".into()],
            gap_score: 0.0,
        };
        let cost = estimate_integration_cost(&gap);
        assert_eq!(cost.risk, "low");
        assert!(cost.person_days > 0.0);
    }

    #[test]
    fn integration_cost_high_risk() {
        let gap = CoverageGap {
            baseline_only: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
            candidate_only: vec![],
            shared: vec![],
            gap_score: 1.0,
        };
        let cost = estimate_integration_cost(&gap);
        assert_eq!(cost.risk, "high");
        assert!(cost.person_days >= 4.0, "person_days should be >= 4.0: {}", cost.person_days);
    }

    #[test]
    fn extract_numeric_metrics_from_object() {
        let v = json!({"accuracy": 0.85, "latency_ms": 42.0, "name": "test", "enabled": true});
        let m = extract_numeric_metrics(&v);
        assert_eq!(m.get("accuracy"), Some(&0.85));
        assert_eq!(m.get("latency_ms"), Some(&42.0));
        assert!(m.get("name").is_none(), "string should be excluded");
        assert!(m.get("enabled").is_none(), "bool should be excluded");
    }

    #[test]
    fn compute_verdict_reject_when_score_low() {
        let scores = vec![DimensionScore {
            dimension: "accuracy".into(),
            baseline: 0.9,
            candidate: 0.5,
            delta: -0.4,
            winner: "baseline".into(),
        }];
        let dims = vec![EvalDimension {
            name: "accuracy".into(),
            higher_is_better: true,
            weight: 1.0,
        }];
        let coverage = CoverageGap {
            baseline_only: vec!["feature_x".into()],
            candidate_only: vec![],
            shared: vec![],
            gap_score: 0.5,
        };
        let cost = IntegrationCost {
            description: "test".into(),
            person_days: 5.0,
            risk: "high".into(),
        };
        let verdict = compute_verdict(&scores, &dims, &coverage, &cost);
        assert_eq!(verdict.recommendation, "reject");
    }

    #[test]
    fn compute_verdict_replace_when_clear_win() {
        let scores = vec![DimensionScore {
            dimension: "throughput".into(),
            baseline: 100.0,
            candidate: 200.0,
            delta: 100.0,
            winner: "candidate".into(),
        }];
        let dims = vec![EvalDimension {
            name: "throughput".into(),
            higher_is_better: true,
            weight: 1.0,
        }];
        let coverage = CoverageGap {
            baseline_only: vec![],
            candidate_only: vec![],
            shared: vec!["throughput".into()],
            gap_score: 0.0,
        };
        let cost = IntegrationCost {
            description: "trivial".into(),
            person_days: 0.5,
            risk: "low".into(),
        };
        let verdict = compute_verdict(&scores, &dims, &coverage, &cost);
        assert_eq!(verdict.recommendation, "replace");
    }

    #[test]
    fn empty_template_validation() {
        let config = EvaluationConfig {
            baseline: SolutionSpec {
                name: "base".into(),
                template: "".into(),
                params: HashMap::new(),
                capabilities: vec![],
            },
            candidate: SolutionSpec {
                name: "cand".into(),
                template: "valid.sh".into(),
                params: HashMap::new(),
                capabilities: vec![],
            },
            dimensions: vec![],
            concurrency: 1,
            timeout_ms: 1000,
            no_cache: true,
        };
        let result = run_evaluation(Path::new("/nonexistent"), &config);
        assert!(result.is_err());
    }

    #[test]
    fn empty_dimensions_validation() {
        let config = EvaluationConfig {
            baseline: SolutionSpec {
                name: "base".into(),
                template: "base.sh".into(),
                params: HashMap::new(),
                capabilities: vec![],
            },
            candidate: SolutionSpec {
                name: "cand".into(),
                template: "cand.sh".into(),
                params: HashMap::new(),
                capabilities: vec![],
            },
            dimensions: vec![],
            concurrency: 1,
            timeout_ms: 1000,
            no_cache: true,
        };
        let result = run_evaluation(Path::new("/nonexistent"), &config);
        assert!(result.is_err());
    }

    #[test]
    fn evaluation_to_json_format() {
        let result = EvaluationResult {
            baseline_name: "current".into(),
            candidate_name: "candidate".into(),
            baseline_metrics: [("accuracy".into(), 0.85)].into(),
            candidate_metrics: [("accuracy".into(), 0.90)].into(),
            dimension_scores: vec![DimensionScore {
                dimension: "accuracy".into(),
                baseline: 0.85,
                candidate: 0.90,
                delta: 0.05,
                winner: "candidate".into(),
            }],
            coverage_gap: CoverageGap {
                shared: vec!["accuracy".into()],
                baseline_only: vec![],
                candidate_only: vec!["speed".into()],
                gap_score: 0.0,
            },
            integration_cost: IntegrationCost {
                description: "minimal".into(),
                person_days: 1.0,
                risk: "low".into(),
            },
            verdict: EvalVerdict {
                recommendation: "replace".into(),
                confidence: 0.85,
                reasoning: vec!["candidate wins on accuracy".into()],
            },
        };
        let json = evaluation_to_json(&result);
        assert_eq!(json["verdict"]["recommendation"], "replace");
        assert_eq!(json["coverage"]["shared"][0], "accuracy");
        assert_eq!(json["dimensions"][0]["delta"], 0.05);
    }
}
