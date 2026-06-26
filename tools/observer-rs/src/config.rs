use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ObserverConfig {
    #[serde(default)]
    pub observer: ObserverSection,
    #[serde(default)]
    pub thresholds: ThresholdSection,
    #[serde(default)]
    pub weights: WeightSection,
    #[serde(default)]
    pub audit: AuditSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObserverSection {
    #[serde(default = "default_audit_window_days")]
    pub audit_window_days: i64,
    #[serde(default = "default_min_candidate_frequency")]
    pub min_candidate_frequency: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThresholdSection {
    #[serde(default = "default_jaccard_near_match")]
    pub jaccard_near_match: f32,
    #[serde(default = "default_low_confidence_threshold")]
    pub low_confidence_threshold: f32,
    #[serde(default = "default_healthy_score")]
    pub healthy_score: f32,
    #[serde(default = "default_stable_score")]
    pub stable_score: f32,
    #[serde(default = "default_default_static_score")]
    pub default_static_score: f32,
    #[serde(default = "default_boundary_collision_min_overlap")]
    pub boundary_collision_min_overlap: usize,
    #[serde(default = "default_min_correlation_count")]
    pub min_correlation_count: i32,
    #[serde(default = "default_min_usage_for_pruning_hint")]
    pub min_usage_for_pruning_hint: usize,
    #[serde(default = "default_min_word_length")]
    pub min_word_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WeightSection {
    #[serde(default = "default_dynamic_blend")]
    pub dynamic_blend: f32,
    #[serde(default = "default_static_blend")]
    pub static_blend: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditSection {
    #[serde(default = "default_top_ngram_candidates")]
    pub top_ngram_candidates: usize,
}


impl Default for ObserverSection {
    fn default() -> Self {
        Self {
            audit_window_days: default_audit_window_days(),
            min_candidate_frequency: default_min_candidate_frequency(),
        }
    }
}

impl Default for ThresholdSection {
    fn default() -> Self {
        Self {
            jaccard_near_match: default_jaccard_near_match(),
            low_confidence_threshold: default_low_confidence_threshold(),
            healthy_score: default_healthy_score(),
            stable_score: default_stable_score(),
            default_static_score: default_default_static_score(),
            boundary_collision_min_overlap: default_boundary_collision_min_overlap(),
            min_correlation_count: default_min_correlation_count(),
            min_usage_for_pruning_hint: default_min_usage_for_pruning_hint(),
            min_word_length: default_min_word_length(),
        }
    }
}

impl Default for WeightSection {
    fn default() -> Self {
        Self {
            dynamic_blend: default_dynamic_blend(),
            static_blend: default_static_blend(),
        }
    }
}

impl Default for AuditSection {
    fn default() -> Self {
        Self {
            top_ngram_candidates: default_top_ngram_candidates(),
        }
    }
}

fn default_audit_window_days() -> i64 {
    30
}
fn default_min_candidate_frequency() -> i32 {
    2
}
fn default_jaccard_near_match() -> f32 {
    0.25
}
fn default_low_confidence_threshold() -> f32 {
    0.45
}
fn default_healthy_score() -> f32 {
    85.0
}
fn default_stable_score() -> f32 {
    60.0
}
fn default_default_static_score() -> f32 {
    85.0
}
fn default_boundary_collision_min_overlap() -> usize {
    4
}
fn default_min_correlation_count() -> i32 {
    2
}
fn default_min_usage_for_pruning_hint() -> usize {
    6
}
fn default_min_word_length() -> usize {
    4
}
fn default_dynamic_blend() -> f32 {
    0.6
}
fn default_static_blend() -> f32 {
    0.4
}
fn default_top_ngram_candidates() -> usize {
    10
}

/// Load TOML config from disk; missing file yields defaults.
pub fn load_config(path: Option<&Path>) -> anyhow::Result<ObserverConfig> {
    let Some(path) = path else {
        return Ok(ObserverConfig::default());
    };
    if !path.is_file() {
        return Ok(ObserverConfig::default());
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read observer config {}", path.display()))?;
    let cfg: ObserverConfig = toml::from_str(&raw)
        .with_context(|| format!("parse observer config {}", path.display()))?;
    Ok(cfg)
}

pub fn blended_health_score(
    dynamic_base: f32,
    static_score: f32,
    dynamic_blend: f32,
    static_blend: f32,
) -> f32 {
    (((dynamic_base * dynamic_blend) + (static_score * static_blend)) * 10.0).round() / 10.0
}

pub fn default_observer_config_path() -> &'static str {
    "configs/observer/observer.toml"
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn defaults_match_legacy_hardcoded_thresholds() {
        let cfg = ObserverConfig::default();
        assert_eq!(cfg.observer.audit_window_days, 30);
        assert_eq!(cfg.observer.min_candidate_frequency, 2);
        assert_eq!(cfg.thresholds.jaccard_near_match, 0.25);
        assert_eq!(cfg.thresholds.low_confidence_threshold, 0.45);
        assert_eq!(cfg.thresholds.healthy_score, 85.0);
        assert_eq!(cfg.thresholds.stable_score, 60.0);
        assert_eq!(cfg.weights.dynamic_blend, 0.6);
        assert_eq!(cfg.weights.static_blend, 0.4);
        assert_eq!(cfg.thresholds.boundary_collision_min_overlap, 4);
    }

    #[test]
    fn load_config_from_toml_overrides_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "observer-cfg-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("observer.toml");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(
            file,
            r#"
[observer]
audit_window_days = 14
min_candidate_frequency = 3

[thresholds]
jaccard_near_match = 0.3
healthy_score = 90.0
"#
        )
        .unwrap();
        let cfg = load_config(Some(&path)).unwrap();
        assert_eq!(cfg.observer.audit_window_days, 14);
        assert_eq!(cfg.observer.min_candidate_frequency, 3);
        assert_eq!(cfg.thresholds.jaccard_near_match, 0.3);
        assert_eq!(cfg.thresholds.healthy_score, 90.0);
        assert_eq!(cfg.thresholds.stable_score, 60.0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn blended_health_score_rounds_to_one_decimal() {
        let score = blended_health_score(80.0, 90.0, 0.6, 0.4);
        assert_eq!(score, 84.0);
    }

    #[test]
    fn blended_health_score_full_dynamic() {
        let score = blended_health_score(100.0, 0.0, 0.6, 0.4);
        assert_eq!(score, 60.0);
    }

    #[test]
    fn blended_health_score_full_static() {
        let score = blended_health_score(0.0, 100.0, 0.6, 0.4);
        assert_eq!(score, 40.0);
    }
}
