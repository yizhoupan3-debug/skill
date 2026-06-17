use chrono::{Duration, Utc};
use evolution_rs::EvolutionConfig;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::utils::{canonical_skill_name, entry_is_recent};

pub fn generate_manifest(
    journal: PathBuf,
    scores_json: Option<PathBuf>,
    manifest_path: Option<PathBuf>,
    days: i64,
    cfg: &EvolutionConfig,
) -> anyhow::Result<()> {
    let entries = evolution_rs::load_audit_journal_entries(&journal)?;
    let cutoff = Utc::now() - Duration::days(days);

    let mut static_scores: HashMap<String, f32> = HashMap::new();
    if let Some(path) = scores_json
        && let Ok(content) = std::fs::read_to_string(path)
            && let Ok(payload) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(skills) = payload.get("skills").and_then(|value| value.as_array()) {
                    for entry in skills {
                        if let (Some(name), Some(total)) =
                            (entry["name"].as_str(), entry["total"].as_f64())
                        {
                            static_scores.insert(name.to_string(), total as f32);
                        }
                    }
                } else if let Some(skills) =
                    payload.get("skills").and_then(|value| value.as_object())
                {
                    for (name, entry) in skills {
                        if let Some(score) = entry
                            .get("static_score")
                            .or_else(|| entry.get("dynamic_score"))
                            .and_then(|value| value.as_f64())
                        {
                            static_scores.insert(name.clone(), score as f32);
                        }
                    }
                } else if let Some(obj) = payload.as_object() {
                    for (name, value) in obj {
                        if let Some(score) = value.as_f64() {
                            static_scores.insert(name.clone(), score as f32);
                        }
                    }
                }
            }

    let mut all_skills = HashSet::new();
    if let Some(path) = manifest_path
        && let Ok(content) = std::fs::read_to_string(path)
            && let Ok(payload) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(skills) = payload.get("skills").and_then(|value| value.as_array()) {
                    for row in skills {
                        if let Some(name) = row.get(0).and_then(|value| value.as_str()) {
                            all_skills.insert(name.to_string());
                        }
                    }
                }

    if all_skills.is_empty() {
        all_skills.extend(static_scores.keys().cloned());
    }

    let mut skill_stats: HashMap<String, (i32, i32)> = HashMap::new();
    for e in entries
        .iter()
        .filter(|entry| entry_is_recent(entry, cutoff))
    {
        let Some(skill) = canonical_skill_name(&e.final_skill, &all_skills) else {
            continue;
        };
        let stats = skill_stats.entry(skill).or_insert((0, 0));
        stats.0 += 1;
        if e.reroute {
            stats.1 += 1;
        }
    }

    let mut skills_map = HashMap::new();
    let mut critical_outliers = Vec::new();
    let mut blended_scores = Vec::new();

    for skill in all_skills {
        let (total, reroutes) = skill_stats.get(&skill).cloned().unwrap_or((0, 0));
        let dynamic_base = if total > 0 {
            100.0 * (1.0 - (reroutes as f32 / total as f32))
        } else {
            100.0
        };
        let static_score = *static_scores
            .get(&skill)
            .unwrap_or(&cfg.thresholds.default_static_score);
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
            serde_json::json!({
                "dynamic_score": blended,
                "static_score": (static_score * 10.0).round() / 10.0,
                "usage_30d": total,
                "reroutes_30d": reroutes,
                "health_status": status
            }),
        );
    }

    let avg_health = if !blended_scores.is_empty() {
        (blended_scores.iter().sum::<f32>() / blended_scores.len() as f32 * 10.0).round() / 10.0
    } else {
        0.0
    };

    let manifest = serde_json::json!({
        "ts": Utc::now().to_rfc3339(),
        "summary": {
            "total_skills": skills_map.len(),
            "critical_skills": critical_outliers.len(),
            "avg_health": avg_health,
        },
        "skills": skills_map,
        "critical_outliers": critical_outliers,
    });

    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}
