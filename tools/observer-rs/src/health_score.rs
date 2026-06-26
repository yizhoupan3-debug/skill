use crate::config::{ObserverConfig, blended_health_score};
use crate::telemetry_journal::{TelemetryEvent, event_within_window, load_telemetry_journal};
use anyhow::Context;
use chrono::{Duration, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run_health_score(
    journal: &Path,
    output_dir: &Path,
    cfg: &ObserverConfig,
) -> anyhow::Result<PathBuf> {
    let journal_data = load_telemetry_journal(journal)?;
    let cutoff = Utc::now() - Duration::days(cfg.observer.audit_window_days);
    let mut skill_stats: HashMap<String, (u32, u32)> = HashMap::new();

    for stamped in &journal_data.events {
        if !event_within_window(stamped.ts.as_deref(), cutoff) {
            continue;
        }
        if let TelemetryEvent::RouteDecision { skill, reroute, .. } = &stamped.event {
            if skill.is_empty() || skill == "none" || skill == "general" {
                continue;
            }
            let entry = skill_stats.entry(skill.clone()).or_insert((0, 0));
            entry.0 += 1;
            if *reroute {
                entry.1 += 1;
            }
        }
    }

    let mut skills_map = HashMap::new();
    let mut blended_scores = Vec::new();
    let mut critical_outliers = Vec::new();

    for (skill, (total, reroutes)) in skill_stats {
        let dynamic_base = if total > 0 {
            100.0 * (1.0 - (reroutes as f32 / total as f32))
        } else {
            100.0
        };
        let static_score = cfg.thresholds.default_static_score;
        let blended = blended_health_score(
            dynamic_base,
            static_score,
            cfg.weights.dynamic_blend,
            cfg.weights.static_blend,
        );
        let status = if blended >= cfg.thresholds.healthy_score {
            "Healthy"
        } else if blended >= cfg.thresholds.stable_score {
            "Stable"
        } else {
            "Critical"
        };
        if blended < cfg.thresholds.stable_score {
            critical_outliers.push(skill.clone());
        }
        blended_scores.push(blended);
        let mut entry = json!({
            "dynamic_score": blended,
            "static_score": (static_score * 10.0).round() / 10.0,
            "usage": total,
            "reroutes": reroutes,
            "health_status": status,
        });
        if blended < cfg.thresholds.healthy_score {
            let steps: Vec<String> = if status == "Critical" {
                vec![
                    "检查 routing rules 和 trigger_hints 配置".into(),
                    "review telemetry journal 中该 skill 的 query 分布".into(),
                    "确认 skill 的入口点是否正常响应".into(),
                ]
            } else {
                vec![
                    "review trigger_hints 是否覆盖了所有常见 query".into(),
                    "观察 reroute 趋势是否持续恶化".into(),
                ]
            };
            entry["next_steps"] = json!(steps);
        }
        skills_map.insert(skill, entry);
    }

    let avg_health = if blended_scores.is_empty() {
        0.0
    } else {
        (blended_scores.iter().sum::<f32>() / blended_scores.len() as f32 * 10.0).round() / 10.0
    };

    let manifest = json!({
        "ts": Utc::now().to_rfc3339(),
        "source_journal": journal.display().to_string(),
        "window_days": cfg.observer.audit_window_days,
        "summary": {
            "total_skills": skills_map.len(),
            "critical_skills": critical_outliers.len(),
            "avg_health": avg_health,
        },
        "skills": skills_map,
        "critical_outliers": critical_outliers,
        "weights": {
            "dynamic_blend": cfg.weights.dynamic_blend,
            "static_blend": cfg.weights.static_blend,
        },
    });

    fs::create_dir_all(output_dir).with_context(|| format!("create {}", output_dir.display()))?;
    let out_path = output_dir.join("health-score.json");
    fs::write(
        &out_path,
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )
    .with_context(|| format!("write {}", out_path.display()))?;
    Ok(out_path)
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ObserverConfig;
    use std::io::Write;

    #[test]
    fn health_score_from_route_events() {
        let dir = std::env::temp_dir().join(format!(
            "obs-health-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let journal = dir.join("events.jsonl");
        let out = dir.join("observer");
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = std::fs::File::create(&journal).unwrap();
        writeln!(
            f,
            r#"{{"kind":"route_decision","task":"t","skill":"pdf","confidence":0.9,"reroute":true}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"kind":"route_decision","task":"t2","skill":"pdf","confidence":0.9,"reroute":false}}"#
        )
        .unwrap();
        let path = run_health_score(&journal, &out, &ObserverConfig::default()).unwrap();
        let raw = std::fs::read_to_string(path).unwrap();
        assert!(raw.contains("\"pdf\""));
        assert!(raw.contains("avg_health"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
