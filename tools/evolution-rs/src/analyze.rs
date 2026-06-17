use crate::config::EvolutionConfig;
use crate::telemetry_journal::{TelemetryEvent, event_within_window, load_telemetry_journal};
use anyhow::Context;
use chrono::{Duration, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run_analyze(
    journal: &Path,
    output_dir: &Path,
    days: i64,
    cfg: &EvolutionConfig,
) -> anyhow::Result<PathBuf> {
    let journal_data = load_telemetry_journal(journal)?;
    let cutoff = Utc::now() - Duration::days(days);

    let mut route_total = 0usize;
    let mut reroute_count = 0usize;
    let mut low_confidence = 0usize;
    let mut tool_total = 0usize;
    let mut tool_failures = 0usize;
    let mut hook_counts: HashMap<String, usize> = HashMap::new();
    let mut rfv_round_total = 0usize;
    let mut rfv_verdict_by_bucket: HashMap<String, usize> = HashMap::new();
    let mut skill_usage: HashMap<String, usize> = HashMap::new();
    let mut prediction_outcome_total = 0usize;
    let mut prediction_outcome_matched = 0usize;
    let mut prediction_outcome_mismatched = 0usize;

    for stamped in &journal_data.events {
        if !event_within_window(stamped.ts.as_deref(), cutoff) {
            continue;
        }
        match &stamped.event {
            TelemetryEvent::RouteDecision {
                skill,
                confidence,
                reroute,
                ..
            } => {
                route_total += 1;
                if *reroute {
                    reroute_count += 1;
                }
                if *confidence < cfg.thresholds.low_confidence_threshold {
                    low_confidence += 1;
                }
                *skill_usage.entry(skill.clone()).or_insert(0) += 1;
            }
            TelemetryEvent::ToolCall { success, .. } => {
                tool_total += 1;
                if !success {
                    tool_failures += 1;
                }
            }
            TelemetryEvent::HookFired { hook_name, .. } => {
                *hook_counts.entry(hook_name.clone()).or_insert(0) += 1;
            }
            TelemetryEvent::GoalTransition { .. } => {}
            TelemetryEvent::RfvRound { verdict, .. } => {
                rfv_round_total += 1;
                let bucket = normalize_rfv_verdict_bucket(verdict);
                *rfv_verdict_by_bucket.entry(bucket).or_insert(0) += 1;
            }
            TelemetryEvent::DevExempt { .. } => {}
            TelemetryEvent::PredictionOutcome { matched, .. } => {
                prediction_outcome_total += 1;
                if *matched {
                    prediction_outcome_matched += 1;
                } else {
                    prediction_outcome_mismatched += 1;
                }
            }
        }
    }

    let report = json!({
        "ts": Utc::now().to_rfc3339(),
        "source_journal": journal.display().to_string(),
        "window_days": days,
        "cutoff": cutoff.to_rfc3339(),
        "route_decisions": route_total,
        "reroute_count": reroute_count,
        "low_confidence_count": low_confidence,
        "tool_calls": tool_total,
        "tool_failures": tool_failures,
        "hook_fired_by_name": hook_counts,
        "rfv_round_total": rfv_round_total,
        "rfv_verdict_by_bucket": rfv_verdict_by_bucket,
        "skill_usage": skill_usage,
        "prediction_outcome_total": prediction_outcome_total,
        "prediction_outcome_matched": prediction_outcome_matched,
        "prediction_outcome_mismatched": prediction_outcome_mismatched,
        "thresholds": {
            "jaccard_near_match": cfg.thresholds.jaccard_near_match,
            "low_confidence_threshold": cfg.thresholds.low_confidence_threshold,
            "healthy_score": cfg.thresholds.healthy_score,
        },
    });

    fs::create_dir_all(output_dir).with_context(|| format!("create {}", output_dir.display()))?;
    let out_path = output_dir.join("analysis.json");
    fs::write(
        &out_path,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )
    .with_context(|| format!("write {}", out_path.display()))?;
    Ok(out_path)
}

fn normalize_rfv_verdict_bucket(verdict: &str) -> String {
    match verdict.trim().to_ascii_uppercase().as_str() {
        "PASS" => "pass".to_string(),
        "FAIL" => "fail".to_string(),
        "SKIPPED" => "skipped".to_string(),
        "UNKNOWN" | "" => "unknown".to_string(),
        // Truncate unknown verdicts to prevent bucket explosion
        other if other.len() <= 32 => other.to_ascii_lowercase(),
        other => format!("unknown_{}", &other[..32.min(other.len())]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EvolutionConfig;
    use std::io::Write;

    #[test]
    fn analyze_writes_report_file() {
        let dir = std::env::temp_dir().join(format!(
            "evo-analyze-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let journal = dir.join("events.jsonl");
        let out = dir.join("evolution");
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = std::fs::File::create(&journal).unwrap();
        writeln!(
            f,
            r#"{{"kind":"hook_fired","hook_name":"stop","action":"allow"}}"#
        )
        .unwrap();
        writeln!(f, r#"{{"kind":"rfv_round","round":1,"verdict":"PASS"}}"#).unwrap();
        writeln!(f, r#"{{"kind":"rfv_round","round":2,"verdict":"FAIL"}}"#).unwrap();
        let path = run_analyze(&journal, &out, 30, &EvolutionConfig::default()).unwrap();
        let raw = std::fs::read_to_string(path).unwrap();
        assert!(raw.contains("hook_fired_by_name"));
        assert!(raw.contains("\"rfv_round_total\": 2"));
        assert!(raw.contains("\"pass\": 1"));
        assert!(raw.contains("\"fail\": 1"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn analyze_filters_events_outside_window_by_ts() {
        let dir = std::env::temp_dir().join(format!(
            "evo-analyze-window-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let journal = dir.join("events.jsonl");
        let out = dir.join("evolution");
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = std::fs::File::create(&journal).unwrap();
        writeln!(
            f,
            r#"{{"ts":"2020-01-01T00:00:00Z","kind":"route_decision","task":"old","skill":"pdf","confidence":0.9,"reroute":false}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"ts":"{}","kind":"route_decision","task":"recent","skill":"pdf","confidence":0.9,"reroute":false}}"#,
            Utc::now().to_rfc3339()
        )
        .unwrap();
        let path = run_analyze(&journal, &out, 30, &EvolutionConfig::default()).unwrap();
        let raw = std::fs::read_to_string(path).unwrap();
        assert!(raw.contains("\"route_decisions\": 1"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn analyze_counts_prediction_outcomes() {
        let dir = std::env::temp_dir().join(format!(
            "evo-analyze-pred-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let journal = dir.join("events.jsonl");
        let out = dir.join("evolution");
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = std::fs::File::create(&journal).unwrap();
        writeln!(
            f,
            r#"{{"kind":"prediction_outcome","task_id":"t-1","matched":true,"predicted_verification_status":"passed","predicted_hypothesis":null,"actual_verification_status":"passed","checks_summary":"ok","checks":[]}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"kind":"prediction_outcome","task_id":"t-2","matched":false,"predicted_verification_status":"passed","predicted_hypothesis":null,"actual_verification_status":"failed","checks_summary":"mismatch","checks":[]}}"#
        )
        .unwrap();
        let path = run_analyze(&journal, &out, 30, &EvolutionConfig::default()).unwrap();
        let raw = std::fs::read_to_string(path).unwrap();
        assert!(raw.contains("\"prediction_outcome_total\": 2"));
        assert!(raw.contains("\"prediction_outcome_matched\": 1"));
        assert!(raw.contains("\"prediction_outcome_mismatched\": 1"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn normalize_rfv_verdict_bucket_maps_known_values() {
        assert_eq!(normalize_rfv_verdict_bucket("PASS"), "pass");
        assert_eq!(normalize_rfv_verdict_bucket("fail"), "fail");
        assert_eq!(normalize_rfv_verdict_bucket(""), "unknown");
    }
}
