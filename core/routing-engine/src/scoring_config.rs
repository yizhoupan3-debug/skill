//! Externalized scoring weights loaded from `configs/scoring_weights.json`.
//!
//! All numeric tuning knobs for skill-route scoring live in the JSON file.
//! If the file is missing or malformed, the compile-time-embedded defaults
//! (matching the values originally hardcoded in `scoring.rs`) are used.

use serde::Deserialize;
use std::sync::LazyLock;

/// Compile-time-embedded fallback matching the values in
/// `configs/scoring_weights.json`.
const DEFAULTS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/scoring_weights.json"
));

/// Resolve the runtime path for scoring weights.
/// Priority: FRAMEWORK_ROOT env var > use embedded defaults.
fn resolve_runtime_weights_path() -> Option<String> {
    // Try FRAMEWORK_ROOT environment variable (canonical way to find project root)
    if let Ok(root) = std::env::var("FRAMEWORK_ROOT") {
        let path = format!("{root}/configs/scoring_weights.json");
        return Some(path);
    }
    None
}

const EXPECTED_SCHEMA: &str = "scoring-weights-v1";

/// Returns the schema version constant embedded at compile time for
/// `scoring_weights.json`. Used by integration tests to verify
/// the embedded JSON matches the disk file.
pub fn embedded_schema_version() -> &'static str {
    EXPECTED_SCHEMA
}

/// All tuneable numeric weights for the routing scoring pipeline.
#[derive(Debug, Clone, Deserialize)]
pub struct ScoringWeights {
    // -- score_route_candidate: agent-swarm --
    pub agent_swarm_boost: f64,
    pub parallel_execution_boost: f64,
    pub parallel_review_boost: f64,
    pub token_budget_boost: f64,

    // -- score_route_candidate: design-md --
    pub design_md_quick_suppression_factor: f64,
    pub design_md_boost: f64,

    // -- score_route_candidate: alias / name match --
    pub framework_alias_explicit_boost: f64,
    pub exact_skill_name_boost: f64,

    // -- score_route_candidate: gate phrases --
    pub gate_match_base: f64,
    pub gate_match_max_extra: i32,
    pub gate_match_per_additional: i32,

    // -- score_route_candidate: name / trigger / keyword / alias --
    pub name_tokens_base: f64,
    pub name_tokens_per_token: f64,
    pub trigger_hint_per_match: f64,
    pub metadata_trigger_per_match: f64,
    pub keywords_max: f64,
    pub keywords_per_keyword: f64,
    pub alias_hits_base: f64,
    pub alias_hits_per_hit: f64,

    // -- score_route_candidate: session-start / review / misc --
    pub session_start_required_boost: f64,
    pub session_start_preferred_boost: f64,
    pub code_review_deep_boost: f64,
    pub gate_owner_boost: f64,
    pub visual_review_boost: f64,
    pub visual_review_weak_factor: f64,
    pub do_not_use_penalty_max_ratio: f64,
    pub do_not_use_penalty_per_hit: f64,
    pub paper_workbench_boost: f64,
    pub codegraph_boost: f64,
    pub overlay_suppression_factor: f64,

    // -- pick_owner thresholds --
    pub agent_swarm_candidate_threshold: f64,
    pub top_owner_score_threshold: f64,
    pub gate_before_owner_threshold: f64,

    // -- layer thresholds (pick_owner / routing.rs) --
    #[serde(rename = "layer_threshold_L0")]
    pub layer_threshold_l0: f64,
    #[serde(rename = "layer_threshold_L1")]
    pub layer_threshold_l1: f64,
    #[serde(rename = "layer_threshold_L2_L3")]
    pub layer_threshold_l2_l3: f64,
    pub layer_threshold_default: f64,
}

impl ScoringWeights {
    /// Return the score floor for the given layer name.
    pub fn layer_threshold(&self, layer: &str) -> f64 {
        match layer {
            "L0" => self.layer_threshold_l0,
            "L1" => self.layer_threshold_l1,
            "L2" | "L3" => self.layer_threshold_l2_l3,
            _ => self.layer_threshold_default,
        }
    }
}

static WEIGHTS: LazyLock<&'static ScoringWeights> = LazyLock::new(|| {
    // 1. Try runtime path first (FRAMEWORK_ROOT env var allows edits without recompile).
    if let Some(path) = resolve_runtime_weights_path() {
        if let Ok(json) = std::fs::read_to_string(&path) {
            if let Ok(w) = serde_json::from_str::<ScoringWeights>(&json) {
                return Box::leak(Box::new(w));
            }
            tracing::warn!(
                "[scoring_config] {path} exists but failed to parse; using embedded defaults."
            );
        }
    }
    // 2. Fallback: compile-time embedded JSON.
    let w: ScoringWeights = serde_json::from_str(DEFAULTS_JSON)
        .expect("BUG: embedded scoring_weights.json failed to deserialize");
    Box::leak(Box::new(w))
});

/// Get the singleton scoring weights (loaded once, then cached).
pub fn scoring_weights() -> &'static ScoringWeights {
    &WEIGHTS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_defaults_deserialize() {
        let weights: ScoringWeights = serde_json::from_str(DEFAULTS_JSON).expect("defaults json");
        assert!(weights.agent_swarm_boost > 0.0);
        assert!(weights.top_owner_score_threshold > 0.0);
    }

    #[test]
    fn scoring_weights_singleton_matches_embedded_defaults() {
        let embedded: ScoringWeights = serde_json::from_str(DEFAULTS_JSON).unwrap();
        let live = scoring_weights();
        assert!(
            (live.agent_swarm_boost - embedded.agent_swarm_boost).abs() < f64::EPSILON,
            "runtime weights should match embedded defaults when config file is valid"
        );
    }

    #[test]
    fn layer_threshold_maps_known_layers() {
        let weights: ScoringWeights = serde_json::from_str(DEFAULTS_JSON).unwrap();
        assert_eq!(weights.layer_threshold("L0"), weights.layer_threshold_l0);
        assert_eq!(weights.layer_threshold("L2"), weights.layer_threshold_l2_l3);
        assert_eq!(
            weights.layer_threshold("unknown"),
            weights.layer_threshold_default
        );
    }
}
