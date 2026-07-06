//! Ablation analysis engine — run baseline experiments against isolated component removals
//! to measure each component's contribution, gain, and damage.
//!
//! # Workflow
//!
//! 1. Run a **baseline** experiment with the full system enabled.
//! 2. For each **component**, run an ablated version that removes that component
//!    (signalled via `EXPERIMENT_SMOKE_ABLATION_REMOVED=<name>` environment variable).
//! 3. Parse each experiment's JSON result, compute per-metric deltas (Δ).
//! 4. Build a contribution matrix with quantified gain/damage/recommendations.
//!
//! # Template contract
//!
//! Template scripts must:
//! - Accept `EXPERIMENT_SMOKE_ABLATION_REMOVED` (set to component name when ablated,
//!   absent or empty for baseline).
//! - Output a JSON object on the last line of stdout with numeric metric keys,
//!   e.g. `{"accuracy": 0.85, "latency_ms": 42}`.
//!
//! # Concurrency
//!
//! All experiments (baseline + all components) run with bounded concurrency via
//! the same chunking mechanism as `smoke::run_experiments`.

use crate::smoke::{self, ExperimentResult};
use crate::smoke_cache::ExperimentCache;
use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;

// ── Public types ──

/// A single component to ablate.
#[derive(Debug, Clone)]
pub struct ComponentSpec {
    /// Human-readable component name (used for `SMOKE_ABLATION_REMOVED` env var).
    pub name: String,
    /// Short description of what this component does.
    pub description: String,
    /// Optional override params for the ablated run.
    /// If `None`, the same `baseline_params` are used and only the env var changes.
    pub ablation_params: Option<HashMap<String, String>>,
}

/// Configuration for an ablation test suite.
#[derive(Debug, Clone)]
pub struct AblationConfig {
    /// Template filename in `templates/` (must be executable).
    pub template: String,
    /// Baseline parameters (full system).
    pub baseline_params: HashMap<String, String>,
    /// Components to test individually.
    pub components: Vec<ComponentSpec>,
    /// Metric names to extract from result JSON for delta computation.
    /// If empty, all numeric fields in the result JSON are tracked.
    pub metrics: Vec<String>,
    /// Max parallel subprocesses (1–32, default 4).
    pub concurrency: usize,
    /// Per-experiment timeout in ms (default 60000).
    pub timeout_ms: u64,
    /// Bypass LRU+TTL cache.
    pub no_cache: bool,
}

/// Delta measurement for a single metric.
#[derive(Debug, Clone)]
pub struct MetricDelta {
    /// Metric name (e.g., "accuracy", "latency_ms").
    pub name: String,
    /// Baseline value.
    pub baseline: f64,
    /// Ablated value (component removed).
    pub ablated: f64,
    /// Delta = ablated - baseline.
    /// A negative delta means the component contributed positively (ablated is worse).
    /// A positive delta means the component hurt (ablated is better).
    pub delta: f64,
    /// Direction interpretation.
    pub direction: DeltaDirection,
}

/// Whether a positive delta is good (latency ↓) or bad (accuracy ↓).
#[derive(Debug, Clone, PartialEq)]
pub enum DeltaDirection {
    /// Higher is better (accuracy, throughput, recall).
    HigherIsBetter,
    /// Lower is better (latency, memory, error rate).
    LowerIsBetter,
}

/// Result for a single ablated component.
#[derive(Debug, Clone)]
pub struct ComponentResult {
    /// Component name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Raw experiment result for the ablated run.
    pub experiment: ExperimentResult,
    /// Per-metric deltas vs baseline.
    pub deltas: Vec<MetricDelta>,
    /// Overall contribution score (normalized 0..1).
    /// Higher = more valuable to keep.
    pub contribution_score: f64,
    /// Combined damage score (normalized 0..1).
    /// Higher = more harmful to keep.
    pub damage_score: f64,
    /// Recommendation string.
    pub recommendation: String,
}

/// Complete ablation result.
#[derive(Debug, Clone)]
pub struct AblationResult {
    /// Baseline experiment.
    pub baseline: ExperimentResult,
    /// Per-component results (ablated — component removed).
    pub components: Vec<ComponentResult>,
    /// Summary impact matrix (JSON for MCP response).
    pub matrix: Value,
}

// ── Public entry point ──

/// Run an ablation test suite.
///
/// Executes the baseline then each component in isolation, computes per-metric deltas,
/// and produces a contribution matrix.
///
/// # Arguments
/// - `repo_root`: project root containing `templates/` directory
/// - `config`: ablation test configuration
pub fn run_ablation(repo_root: &Path, config: &AblationConfig) -> Result<AblationResult, FrameworkError> {
    let concurrency = config.concurrency.max(1).min(32);
    let timeout_ms = config.timeout_ms.max(100);
    let artifacts_dir = repo_root.join("artifacts/research-log/smoke");
    let template_path = repo_root.join("templates").join(&config.template);

    // Basic validation
    if config.template.is_empty() {
        return Err(FrameworkError::validation("template name must not be empty"));
    }
    if config.components.is_empty() {
        return Err(FrameworkError::validation("at least one component is required for ablation"));
    }
    if !template_path.exists() {
        return Err(FrameworkError::not_found(format!(
            "template not found: {} (looked in templates/)",
            config.template
        )));
    }

    // Build experiment runs: baseline + each component
    let n_runs = 1 + config.components.len();
    let mut all_runs = Vec::with_capacity(n_runs);

    // Baseline run (full system)
    all_runs.push(smoke::ExperimentRun {
        run_id: format!("{}-baseline", config.template),
        template_name: config.template.clone(),
        template_path: template_path.clone(),
        params: config.baseline_params.clone(),
    });

    // Ablated runs (one per component)
    for (i, comp) in config.components.iter().enumerate() {
        let params = comp
            .ablation_params
            .clone()
            .unwrap_or_else(|| config.baseline_params.clone());
        // Inject ablation signal — template script can check EXPERIMENT_SMOKE_ABLATION_REMOVED
        let mut p = params;
        p.insert("SMOKE_ABLATION_REMOVED".to_string(), comp.name.clone());
        all_runs.push(smoke::ExperimentRun {
            run_id: format!("{}-abl-{i}-{}", config.template, comp.name),
            template_name: config.template.clone(),
            template_path: template_path.clone(),
            params: p,
        });
    }

    // Use the existing experiment runner infrastructure (made pub(crate))
    let cache = ExperimentCache::new(&artifacts_dir, config.no_cache);
    let results = crate::smoke::run_experiments(
        &all_runs,
        timeout_ms,
        concurrency,
        &cache,
        &artifacts_dir,
    );

    // Separate baseline from component results
    let mut baseline_result: Option<ExperimentResult> = None;
    let mut component_results: Vec<(usize, ExperimentResult)> = Vec::new();

    for (idx, result) in results.into_iter().enumerate() {
        if idx == 0 {
            baseline_result = Some(result);
        } else {
            component_results.push((idx - 1, result));
        }
    }

    let baseline = baseline_result.ok_or_else(|| {
        FrameworkError::not_found(
            "ablation: baseline experiment returned no result — check template and params",
        )
    })?;

    // Parse baseline metrics
    let baseline_metrics = extract_metrics(&baseline.result, &config.metrics);

    // Compute per-component deltas
    let mut components = Vec::with_capacity(config.components.len());
    let mut matrix_rows = Vec::with_capacity(config.components.len());

    for (comp_idx, comp) in config.components.iter().enumerate() {
        let ablated = &component_results
            .iter()
            .find(|(i, _)| *i == comp_idx)
            .map(|(_, r)| r)
            .cloned()
            .unwrap_or_else(|| {
                // Fallback: create a failed result entry
                ExperimentResult {
                    run_id: format!("{}-missing-{}", config.template, comp.name),
                    template_name: config.template.clone(),
                    params: comp.ablation_params.clone().unwrap_or_default(),
                    exit_code: -1,
                    result: Value::Null,
                    error: Some("ablation experiment did not produce a result".into()),
                    wall_time_ms: 0,
                }
            });

        let ablated_metrics = extract_metrics(&ablated.result, &config.metrics);
        let deltas = compute_deltas(&baseline_metrics, &ablated_metrics);

        let (contribution_score, damage_score, recommendation) =
            compute_scores(&deltas);

        let comp_result = ComponentResult {
            name: comp.name.clone(),
            description: comp.description.clone(),
            experiment: ablated.clone(),
            deltas,
            contribution_score,
            damage_score,
            recommendation,
        };

        // Build matrix row
        let mut metric_map = serde_json::Map::new();
        let mut delta_map = serde_json::Map::new();
        for d in &comp_result.deltas {
            metric_map.insert(
                d.name.clone(),
                json!({"baseline": d.baseline, "ablated": d.ablated}),
            );
            delta_map.insert(
                format!("{}_delta", d.name),
                json!(d.delta),
            );
        }

        matrix_rows.push(json!({
            "component": comp.name,
            "description": comp.description,
            "error": ablated.error,
            "metrics": Value::Object(metric_map),
            "deltas": Value::Object(delta_map),
            "contribution_score": contribution_score,
            "damage_score": damage_score,
            "recommendation": comp_result.recommendation,
        }));

        components.push(comp_result);
    }

    let matrix = json!({
        "template": config.template,
        "baseline": {
            "run_id": baseline.run_id,
            "result": baseline.result,
            "wall_time_ms": baseline.wall_time_ms,
        },
        "components": Value::Array(matrix_rows),
        "summary": {
            "total_components": components.len(),
            "critical": components.iter().filter(|c| c.recommendation == "critical").count(),
            "retain": components.iter().filter(|c| c.recommendation == "retain").count(),
            "optimizable": components.iter().filter(|c| c.recommendation == "optimizable").count(),
            "removable": components.iter().filter(|c| c.recommendation == "removable").count(),
            "unknown": components.iter().filter(|c| c.recommendation == "insufficient_data").count(),
        }
    });

    Ok(AblationResult {
        baseline,
        components,
        matrix,
    })
}

// ── Metric extraction ──

/// Extract numeric metrics from an experiment result JSON.
///
/// If `metric_names` is non-empty, only those keys are extracted (must be f64-compatible).
/// If empty, all numeric f64 values in the result are used.
fn extract_metrics(result: &Value, metric_names: &[String]) -> HashMap<String, f64> {
    let obj = match result.as_object() {
        Some(o) => o,
        None => return HashMap::new(),
    };

    let mut out = HashMap::new();

    if metric_names.is_empty() {
        // Auto-detect all numeric fields
        for (k, v) in obj {
            if let Some(n) = v.as_f64() {
                out.insert(k.clone(), n);
            }
        }
    } else {
        // Only named metrics
        for name in metric_names {
            if let Some(n) = obj.get(name).and_then(Value::as_f64) {
                out.insert(name.clone(), n);
            }
        }
    }
    out
}

// ── Delta computation ──

/// Compute per-metric deltas between baseline and ablated results.
fn compute_deltas(
    baseline: &HashMap<String, f64>,
    ablated: &HashMap<String, f64>,
) -> Vec<MetricDelta> {
    let mut deltas = Vec::new();

    // Union of all metric keys
    let mut all_keys: Vec<&String> = baseline.keys().chain(ablated.keys()).collect();
    all_keys.sort();
    all_keys.dedup();

    for key in all_keys {
        let b = baseline.get(key).copied().unwrap_or(0.0);
        let a = ablated.get(key).copied().unwrap_or(0.0);
        let delta = a - b;

        // Determine direction heuristically based on key name conventions
        let direction = if key.contains("latency")
            || key.contains("time")
            || key.contains("ms")
            || key.contains("cost")
            || key.contains("error")
            || key.contains("loss")
            || key.contains("memory")
            || key.contains("size")
        {
            DeltaDirection::LowerIsBetter
        } else {
            // Default: accuracy, f1, recall, score, throughput, etc. — higher is better
            DeltaDirection::HigherIsBetter
        };

        deltas.push(MetricDelta {
            name: key.clone(),
            baseline: b,
            ablated: a,
            delta,
            direction,
        });
    }
    deltas
}

// ── Scoring ──

/// Compute contribution and damage scores from deltas.
///
/// Returns `(contribution_score, damage_score, recommendation)`.
///
/// - `contribution_score` 0..1: how much the component helps
/// - `damage_score` 0..1: how much the component hurts
/// - `recommendation`: "critical" | "retain" | "optimizable" | "removable" | "insufficient_data"
///
/// Scoring balances two factors:
/// 1. **Relative impact**: delta / max_delta across all metrics (normalized 0..1)
/// 2. **Absolute impact**: delta.abs / baseline (how much the metric changed relative to its value)
///    — this prevents single-metric cases from always scoring 1.0
fn compute_scores(deltas: &[MetricDelta]) -> (f64, f64, String) {
    if deltas.is_empty() {
        return (0.0, 0.0, "insufficient_data".into());
    }

    let mut total_gain = 0.0_f64;
    let mut total_damage = 0.0_f64;
    let mut gain_count = 0_usize;
    let mut damage_count = 0_usize;

    for d in deltas {
        match d.direction {
            DeltaDirection::HigherIsBetter => {
                if d.delta < 0.0 {
                    total_gain += d.delta.abs();
                    gain_count += 1;
                } else if d.delta > 0.0 {
                    total_damage += d.delta;
                    damage_count += 1;
                }
            }
            DeltaDirection::LowerIsBetter => {
                if d.delta > 0.0 {
                    total_gain += d.delta;
                    gain_count += 1;
                } else if d.delta < 0.0 {
                    total_damage += d.delta.abs();
                    damage_count += 1;
                }
            }
        }
    }

    let max_delta = deltas
        .iter()
        .map(|d| d.delta.abs())
        .fold(0.0_f64, f64::max)
        .max(1e-10);

    // Compute average delta per metric, then normalize by two factors:
    // 1. relative to max_delta (cross-metric normalization)
    // 2. relative to baseline (absolute magnitude)
    // The final score uses the min of the two — a delta can be "the largest"
    // but still tiny in absolute terms.
    let mut rel_gain = 0.0_f64;
    if gain_count > 0 {
        let avg = total_gain / gain_count as f64;
        let rel_to_max = avg / max_delta;
        // Also consider relative change to baseline values
        let abs_to_baseline = deltas.iter()
            .filter(|d| match d.direction {
                DeltaDirection::HigherIsBetter => d.delta < 0.0,
                DeltaDirection::LowerIsBetter => d.delta > 0.0,
            })
            .map(|d| (d.delta.abs() / d.baseline.abs().max(1e-10)).min(1.0))
            .sum::<f64>() / gain_count as f64;
        rel_gain = rel_to_max.min(abs_to_baseline);  // use the more conservative measure
    }

    let mut rel_damage = 0.0_f64;
    if damage_count > 0 {
        let avg = total_damage / damage_count as f64;
        let rel_to_max = avg / max_delta;
        let abs_to_baseline = deltas.iter()
            .filter(|d| match d.direction {
                DeltaDirection::HigherIsBetter => d.delta > 0.0,
                DeltaDirection::LowerIsBetter => d.delta < 0.0,
            })
            .map(|d| (d.delta.abs() / d.baseline.abs().max(1e-10)).min(1.0))
            .sum::<f64>() / damage_count as f64;
        rel_damage = rel_to_max.min(abs_to_baseline);
    }

    let contribution_score = rel_gain.clamp(0.0, 1.0);
    let damage_score = rel_damage.clamp(0.0, 1.0);

    // Apply damage penalty: high damage reduces effective contribution
    let effective_contribution = contribution_score * (1.0 - damage_score * 0.5);

    // Recommendation rules (using effective_contribution which factors in damage)
    let recommendation = if gain_count == 0 && damage_count == 0 {
        "insufficient_data"
    } else if effective_contribution > 0.6 && damage_score < 0.2 {
        "critical"
    } else if effective_contribution > 0.4 {
        "retain"
    } else if damage_score > 0.5 {
        "removable"
    } else if effective_contribution > 0.2 {
        "optimizable"
    } else {
        "removable"
    };

    (effective_contribution, damage_score, recommendation.into())
}

// ── Tests ──

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn extract_metrics_from_object() {
        let result = json!({"accuracy": 0.85, "latency_ms": 42.0, "name": "test"});
        let metrics = extract_metrics(&result, &[]);
        assert_eq!(metrics.get("accuracy"), Some(&0.85));
        assert_eq!(metrics.get("latency_ms"), Some(&42.0));
        // "name" is a string, not f64 → excluded
        assert!(metrics.get("name").is_none());
    }

    #[test]
    fn extract_metrics_filtered() {
        let result = json!({"accuracy": 0.85, "latency_ms": 42.0, "throughput": 100.0});
        let metrics = extract_metrics(
            &result,
            &["accuracy".into(), "throughput".into()],
        );
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics.get("accuracy"), Some(&0.85));
        assert_eq!(metrics.get("throughput"), Some(&100.0));
        assert!(metrics.get("latency_ms").is_none());
    }

    #[test]
    fn extract_metrics_non_object() {
        let metrics = extract_metrics(&Value::Null, &[]);
        assert!(metrics.is_empty());
    }

    #[test]
    fn compute_deltas_basic() {
        let mut baseline = HashMap::new();
        baseline.insert("accuracy".into(), 0.90);
        baseline.insert("latency_ms".into(), 50.0);

        let mut ablated = HashMap::new();
        ablated.insert("accuracy".into(), 0.60);  // worse → component helped
        ablated.insert("latency_ms".into(), 30.0); // better → component hurt

        let deltas = compute_deltas(&baseline, &ablated);
        assert_eq!(deltas.len(), 2);

        for d in &deltas {
            match d.name.as_str() {
                "accuracy" => {
                    assert_eq!(d.baseline, 0.90);
                    assert_eq!(d.ablated, 0.60);
                    assert!((d.delta - (-0.30)).abs() < 1e-10);
                    assert_eq!(d.direction, DeltaDirection::HigherIsBetter);
                }
                "latency_ms" => {
                    assert_eq!(d.baseline, 50.0);
                    assert_eq!(d.ablated, 30.0);
                    assert_eq!(d.delta, -20.0);
                    assert_eq!(d.direction, DeltaDirection::LowerIsBetter);
                }
                n => panic!("unexpected metric: {n}"),
            }
        }
    }

    #[test]
    fn compute_scores_critical_component() {
        // Accuracy drops significantly when removed → critical
        let deltas = vec![MetricDelta {
            name: "accuracy".into(),
            baseline: 0.95,
            ablated: 0.30,
            delta: -0.65,
            direction: DeltaDirection::HigherIsBetter,
        }];
        let (contrib, damage, rec) = compute_scores(&deltas);
        // delta/baseline = 0.65/0.95 ≈ 0.68
        assert!((contrib - 0.6842105).abs() < 1e-5, "contribution should be ~0.68: {contrib}");
        assert_eq!(damage, 0.0, "no damage expected");
        assert_eq!(rec, "critical");
    }

    #[test]
    fn compute_scores_removable_component() {
        // Latency drops from 200ms to 50ms when removed → component hurts (adds 150ms overhead)
        // delta = -150.0 for LowerIsBetter → delta < 0 → burden → damage
        let deltas = vec![MetricDelta {
            name: "latency_ms".into(),
            baseline: 200.0,
            ablated: 50.0,
            delta: -150.0,
            direction: DeltaDirection::LowerIsBetter,
        }];
        let (contrib, damage, rec) = compute_scores(&deltas);
        // delta/baseline = 150/200 = 0.75
        assert!((damage - 0.75).abs() < 1e-5, "damage should be ~0.75: {damage}");
        assert_eq!(rec, "removable");
    }

    #[test]
    fn compute_scores_insufficient_data() {
        let deltas = vec![];
        let (contrib, damage, rec) = compute_scores(&deltas);
        assert_eq!(contrib, 0.0);
        assert_eq!(damage, 0.0);
        assert_eq!(rec, "insufficient_data");
    }

    #[test]
    fn compute_scores_optimizable_component() {
        // Throughput drops slightly (100→95) when removed → small positive contribution
        // delta/baseline = 5/100 = 0.05 → contribution ≈ 0.05 → falls below optimizable threshold
        let deltas = vec![MetricDelta {
            name: "throughput".into(),
            baseline: 100.0,
            ablated: 95.0,
            delta: -5.0,
            direction: DeltaDirection::HigherIsBetter,
        }];
        let (contrib, damage, rec) = compute_scores(&deltas);
        assert!((contrib - 0.05).abs() < 1e-5, "contribution should be ~0.05: {contrib}");
        assert_eq!(damage, 0.0);
        // 5% relative change is below optimizable threshold; tiny contribution
        assert!(rec == "removable" || rec == "optimizable",
            "recommendation should be removable or optimizable: {rec}");
    }

    #[test]
    fn run_ablation_validates_empty_template() {
        let config = AblationConfig {
            template: "".into(),
            baseline_params: HashMap::new(),
            components: vec![ComponentSpec {
                name: "test".into(),
                description: "test component".into(),
                ablation_params: None,
            }],
            metrics: vec![],
            concurrency: 1,
            timeout_ms: 1000,
            no_cache: true,
        };
        let result = run_ablation(Path::new("/nonexistent"), &config);
        assert!(result.is_err());
    }

    #[test]
    fn run_ablation_validates_empty_components() {
        let config = AblationConfig {
            template: "test.sh".into(),
            baseline_params: HashMap::new(),
            components: vec![],
            metrics: vec![],
            concurrency: 1,
            timeout_ms: 1000,
            no_cache: true,
        };
        let result = run_ablation(Path::new("/nonexistent"), &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one component"));
    }
}
