//! Routing evolution telemetry analysis tool handler (`domain:routing-evolution`).
//! Reads the telemetry journal, aggregates, and reports.

use core_policy::error::FrameworkError;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;

// ── Routing evolution types ──

#[derive(serde::Deserialize)]
struct RouteLogEntry {
    ts: Option<String>,
    kind: Option<String>,
    task: Option<String>,
    skill: Option<String>,
    confidence: Option<f32>,
    reroute: Option<bool>,
    parity_gate: Option<String>,
}

// ── Public dispatch function ──

/// routing_evolution: read telemetry journal, aggregate, and report.
pub fn routing_evolution_dispatch(
    arguments: &Value,
    repo_root: &Path,
) -> Result<String, FrameworkError> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: operation (stats|analyze|extract|calibrate)"))?;
    let skill_filter = arguments.get("skill").and_then(Value::as_str);
    let lookback_days = arguments.get("days").and_then(Value::as_u64).unwrap_or(0);

    let journal_path = repo_root.join("artifacts/telemetry/events.jsonl");
    if !journal_path.exists() {
        return Err(FrameworkError::validation(format!("Telemetry journal not found at {}", journal_path.display())));
    }

    let file = std::fs::File::open(&journal_path)
        .map_err(|e| format!("open journal: {e}"))?;
    file.lock_shared()
        .map_err(|e| format!("lock journal: {e}"))?;
    let reader = std::io::BufReader::new(file);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff = if lookback_days > 0 {
        now.saturating_sub(lookback_days * 86400)
    } else {
        0
    };

    let mut entries: Vec<RouteLogEntry> = Vec::new();
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("[routing_evolution] read journal line failed: {e}");
                continue;
            }
        };
        if line.trim().is_empty() { continue; }
        let entry: RouteLogEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.kind.as_deref() != Some("route_decision") { continue; }
        if cutoff > 0
            && let Some(ts) = &entry.ts
            && let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(ts)
            && parsed.timestamp() < cutoff as i64 {
                continue;
            }
        if let Some(filter) = skill_filter
            && entry.skill.as_deref() != Some(filter) {
                continue;
            }
        entries.push(entry);
    }

    match operation {
        "stats" => Ok(routing_stats(&entries)),
        "analyze" => Ok(routing_analyze(&entries)),
        "extract" => Ok(routing_extract(&entries)),
        "calibrate" => Ok(routing_calibrate(&entries)),
        _ => Err(FrameworkError::validation(format!("Unknown operation: {operation}. Use stats|analyze|extract|calibrate"))),
    }
}

// ── Private helper functions ──

fn routing_stats(entries: &[RouteLogEntry]) -> String {
    #[derive(serde::Serialize)]
    struct RouteStats {
        total: usize,
        per_skill: Vec<serde_json::Value>,
        gate_distribution: Vec<serde_json::Value>,
        total_reroute: u64,
    }

    let total = entries.len();
    let mut per_skill: HashMap<&str, (u64, f64, u64)> = HashMap::new();
    let mut gate_counts: HashMap<&str, u64> = HashMap::new();
    let mut total_reroute = 0u64;

    for e in entries {
        let skill = e.skill.as_deref().unwrap_or("none");
        let (count, sum, reroute) = per_skill.entry(skill).or_insert((0, 0.0, 0));
        *count += 1;
        *sum += e.confidence.unwrap_or(0.0) as f64;
        if e.reroute.unwrap_or(false) { *reroute += 1; total_reroute += 1; }
        let gate = e.parity_gate.as_deref().unwrap_or("unknown");
        *gate_counts.entry(gate).or_insert(0) += 1;
    }

    let skills: Vec<serde_json::Value> = per_skill
        .iter()
        .map(|(slug, (count, sum, reroute))| {
            json!({
                "slug": slug,
                "count": count,
                "avg_confidence": if *count > 0 { format!("{:.2}", sum / *count as f64) } else { "0.00".to_string() },
                "reroute_count": reroute,
            })
        })
        .collect();

    let gate_distribution: Vec<serde_json::Value> = gate_counts
        .iter()
        .map(|(gate, count)| json!({"gate": gate, "count": count}))
        .collect();

    serde_json::to_string_pretty(&RouteStats { total, per_skill: skills, gate_distribution, total_reroute })
        .unwrap_or_else(|_| "{}".to_string())
}

fn routing_analyze(entries: &[RouteLogEntry]) -> String {
    let mut low_conf: Vec<(&str, f64)> = Vec::new();
    let mut high_reroute: Vec<(&str, u64, u64)> = Vec::new();

    let mut per_skill: HashMap<&str, (u64, f64, u64)> = HashMap::new();
    for e in entries {
        let skill = e.skill.as_deref().unwrap_or("none");
        let (count, sum, reroute) = per_skill.entry(skill).or_insert((0, 0.0, 0));
        *count += 1;
        *sum += e.confidence.unwrap_or(0.0) as f64;
        if e.reroute.unwrap_or(false) { *reroute += 1; }
    }

    for (slug, (count, sum, reroute)) in &per_skill {
        let avg_conf = if *count > 0 { sum / *count as f64 } else { 0.0 };
        if avg_conf < 60.0 { low_conf.push((slug, avg_conf)); }
        if *reroute > 0 && *count > 0 { high_reroute.push((slug, *reroute, *count)); }
    }

    low_conf.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    high_reroute.sort_by(|a, b| b.0.cmp(a.0));

    let analysis = json!({
        "total_entries": entries.len(),
        "low_confidence_skills": low_conf.iter().take(10).map(|(s, c)| json!({"slug": s, "avg_confidence": format!("{:.2}", c)})).collect::<Vec<_>>(),
        "reroute_analysis": high_reroute.iter().take(10).map(|(s, r, c)| json!({"slug": s, "reroute_count": r, "total_count": c})).collect::<Vec<_>>(),
    });

    serde_json::to_string_pretty(&analysis).unwrap_or_else(|_| "{}".to_string())
}

fn routing_extract(entries: &[RouteLogEntry]) -> String {
    let extracts: Vec<serde_json::Value> = entries.iter().map(|e| {
        json!({
            "ts": e.ts,
            "task": e.task,
            "skill": e.skill,
            "confidence": e.confidence,
            "reroute": e.reroute,
        })
    }).collect();

    serde_json::to_string_pretty(&extracts).unwrap_or_else(|_| "[]".to_string())
}

fn routing_calibrate(entries: &[RouteLogEntry]) -> String {
    let mut total_conf = 0.0f64;
    let mut conf_count = 0u64;
    for e in entries {
        if let Some(c) = e.confidence {
            total_conf += c as f64;
            conf_count += 1;
        }
    }
    let baseline = if conf_count > 0 { total_conf / conf_count as f64 } else { 70.0 };

    let calibration = json!({
        "baseline_confidence": format!("{:.2}", baseline),
        "suggestion": if baseline < 60.0 {
            "增加 NL 调整规则以提高路由准确性"
        } else if baseline < 75.0 {
            "微调 trigger_hints 和 keyword 权重"
        } else {
            "当前路由表现良好，无需调整"
        },
    });

    serde_json::to_string_pretty(&calibration).unwrap_or_else(|_| "{}".to_string())
}
