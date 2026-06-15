use crate::config::EvolutionConfig;
use crate::telemetry_journal::{TelemetryEvent, load_telemetry_journal};
use anyhow::Context;
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run_health_score(
    journal: &Path,
    output_dir: &Path,
    cfg: &EvolutionConfig,
) -> anyhow::Result<PathBuf> {
    let journal_data = load_telemetry_journal(journal)?;
    let mut skill_stats: HashMap<String, (u32, u32)> = HashMap::new();

    for stamped in &journal_data.events {
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
        let blended = (((dynamic_base * cfg.weights.dynamic_blend)
            + (static_score * cfg.weights.static_blend))
            * 10.0)
            .round()
            / 10.0;
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
        skills_map.insert(
            skill,
            json!({
                "dynamic_score": blended,
                "static_score": (static_score * 10.0).round() / 10.0,
                "usage": total,
                "reroutes": reroutes,
                "health_status": status,
            }),
        );
    }

    let avg_health = if blended_scores.is_empty() {
        0.0
    } else {
        (blended_scores.iter().sum::<f32>() / blended_scores.len() as f32 * 10.0).round() / 10.0
    };

    let manifest = json!({
        "ts": Utc::now().to_rfc3339(),
        "source_journal": journal.display().to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EvolutionConfig;
    use std::io::Write;

    #[test]
    fn health_score_from_route_events() {
        let dir = std::env::temp_dir().join(format!(
            "evo-health-{}",
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
            r#"{{"kind":"route_decision","task":"t","skill":"pdf","confidence":0.9,"reroute":true}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"kind":"route_decision","task":"t2","skill":"pdf","confidence":0.9,"reroute":false}}"#
        )
        .unwrap();
        let path = run_health_score(&journal, &out, &EvolutionConfig::default()).unwrap();
        let raw = std::fs::read_to_string(path).unwrap();
        assert!(raw.contains("\"pdf\""));
        assert!(raw.contains("avg_health"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
