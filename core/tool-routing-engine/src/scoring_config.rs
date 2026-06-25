//! Externalized scoring weights loaded from `configs/tool_scoring_weights.json`.
//!
//! All numeric tuning knobs for tool-route scoring live in the JSON file.
//! If the file is missing or malformed, the compile-time-embedded defaults are used.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Compile-time-embedded fallback.
const DEFAULTS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/tool_scoring_weights.json"
));

/// All tuneable numeric weights for the tool routing scoring pipeline.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolScoringWeights {
    pub exact_name_boost: f64,
    pub display_name_per_match: f64,
    pub name_tokens_base: f64,
    pub name_tokens_per_token: f64,
    pub trigger_hint_per_match: f64,
    pub keyword_per_keyword: f64,
    pub keyword_max: f64,
    pub alias_hits_base: f64,
    pub alias_hits_per_hit: f64,
    pub description_per_match: f64,
    pub description_max: f64,
    pub do_not_use_penalty_max_ratio: f64,
    pub do_not_use_penalty_per_hit: f64,
    /// Layer penalty mapping: layer name → penalty score.
    /// e.g. {"builtin": 0.0, "external": -2.0}
    pub layer_penalties: HashMap<String, f64>,
    /// Minimum Jaccard similarity for fuzzy rescue (0.0–1.0).
    #[serde(default = "default_fuzzy_min")]
    pub fuzzy_min_similarity: f64,
}

fn default_fuzzy_min() -> f64 {
    0.35
}

/// Parse weights from a JSON string. Returns `None` on parse error.
fn parse_weights(json: &str) -> Option<ToolScoringWeights> {
    serde_json::from_str(json).ok()
}

/// Resolve the runtime path for scoring weights.
/// Priority: hook > FRAMEWORK_ROOT env var > None (use embedded defaults).
fn resolve_runtime_weights_path() -> Option<String> {
    // 1. Try hook-injected path
    if let Some(path) = crate::hooks::discover_scoring_weights_path() {
        return Some(path);
    }
    // 2. Try FRAMEWORK_ROOT environment variable
    if let Ok(root) = std::env::var("FRAMEWORK_ROOT") {
        let path = format!("{root}/configs/tool_scoring_weights.json");
        return Some(path);
    }
    // 3. No runtime path available; use embedded defaults
    None
}

/// Runtime-loaded weights with compile-time fallback.
#[allow(clippy::expect_used)]
static WEIGHTS: LazyLock<ToolScoringWeights> = LazyLock::new(|| {
    // Try runtime config first (hook or FRAMEWORK_ROOT)
    if let Some(runtime_path) = resolve_runtime_weights_path()
        && let Ok(content) = std::fs::read_to_string(&runtime_path)
            && let Some(w) = parse_weights(&content) {
                return w;
            }
    // Fallback to compile-time embedded defaults
    parse_weights(DEFAULTS_JSON).expect("embedded tool_scoring_weights.json is invalid")
});

/// Get the tool scoring weights. Loads from JSON at first access, caches forever.
pub(crate) fn tool_scoring_weights() -> ToolScoringWeights {
    WEIGHTS.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let w = tool_scoring_weights();
        assert!(w.exact_name_boost > 0.0);
        assert!(w.name_tokens_base >= 0.0);
        assert!(w.trigger_hint_per_match > 0.0);
        assert!(w.description_per_match >= 0.0);
        assert!(w.description_max > 0.0);
        assert!(w.layer_penalties.contains_key("builtin"));
        assert!(w.layer_penalties.contains_key("external"));
        assert!(w.fuzzy_min_similarity > 0.0);
    }

    #[test]
    fn embedded_defaults_parse() {
        let w = parse_weights(DEFAULTS_JSON).expect("embedded JSON parse failed");
        assert!(w.exact_name_boost > 0.0);
    }

    #[test]
    fn v2_fields_have_defaults() {
        // Test that v1 config missing v2 fields still works via serde default
        let v1_json = r#"{
            "exact_name_boost": 100.0,
            "display_name_per_match": 8.0,
            "name_tokens_base": 14.0,
            "name_tokens_per_token": 4.0,
            "trigger_hint_per_match": 20.0,
            "keyword_per_keyword": 3.0,
            "keyword_max": 24.0,
            "alias_hits_base": 12.0,
            "alias_hits_per_hit": 4.0,
            "description_per_match": 1.0,
            "description_max": 15.0,
            "do_not_use_penalty_max_ratio": 0.3,
            "do_not_use_penalty_per_hit": 5.0,
            "layer_penalties": {"builtin": 0.0, "external": -2.0}
        }"#;
        let w: ToolScoringWeights = serde_json::from_str(v1_json).expect("v1 JSON with defaults");
        assert!((w.fuzzy_min_similarity - 0.35).abs() < f64::EPSILON);
    }
}
