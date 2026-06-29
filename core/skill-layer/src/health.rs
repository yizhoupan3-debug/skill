//! Health manifest generation for SKILL_HEALTH_MANIFEST.json.
//!
//! Computes a blended health score per skill from observable signals:
//! - SKILL.md existence on disk (0.2)
//! - Registration in SKILL_ROUTING_RUNTIME.json (0.3)
//! - Valid frontmatter layer (0.2)
//! - Trigger hints count — more hints = better routing maturity (0.2)
//! - Description present (0.1)
//!
//! Score range: 0.0 – 1.0
//! Status: >= 0.7 "Healthy" | >= 0.4 "Degraded" | < 0.4 "Unhealthy"

use crate::constants;
use core_errors::FrameworkError;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

pub type Result<T> = std::result::Result<T, FrameworkError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HealthEntry {
    blended_score: f64,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    route_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reroute_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HealthManifest {
    schema_version: String,
    source_of_truth: bool,
    version: u32,
    lifecycle: String,
    generated_at: String,
    skills: std::collections::HashMap<String, HealthEntry>,
}

/// Minimal view of a routing runtime entry for health scoring.
#[derive(Debug, Deserialize)]
struct RoutingEntry {
    slug: String,
    layer: String,
    description: Option<String>,
    trigger_hints: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RoutingRuntimeFile {
    keys: Vec<String>,
    entries: Vec<Vec<serde_json::Value>>,
}

// ---------------------------------------------------------------------------
// Health scoring
// ---------------------------------------------------------------------------

/// Compute the blended health score for a skill from observable signals.
fn compute_health_score(
    slug: &str,
    has_skill_md: bool,
    routing_lookup: &HashMap<String, RoutingEntry>,
) -> (f64, String) {
    let mut score: f64 = 0.0;

    // 1. SKILL.md exists on disk (0.2)
    if has_skill_md {
        score += 0.2;
    }

    // 2. Registered in routing runtime (0.3)
    if let Some(entry) = routing_lookup.get(slug) {
        score += 0.3;

        // 3. Valid layer field (0.2) — L0–L4 are valid
        let layer_valid = matches!(
            entry.layer.as_str(),
            "L0" | "L1" | "L2" | "L3" | "L4"
        );
        if layer_valid {
            score += 0.2;
        }

        // 4. Trigger hints — 0.04 per hint, capped at 0.2
        let hint_count = entry
            .trigger_hints
            .as_ref()
            .map(|h| h.len())
            .unwrap_or(0);
        let hint_score = (hint_count as f64 * 0.04).min(0.2);
        score += hint_score;

        // 5. Description present (0.1)
        if entry.description.as_ref().is_some_and(|d| !d.is_empty()) {
            score += 0.1;
        }
    }

    let status = if score >= 0.7 {
        "Healthy"
    } else if score >= 0.4 {
        "Degraded"
    } else {
        "Unhealthy"
    };

    (score.round_to_two_decimals(), status.to_string())
}

/// Round to 2 decimal places.
trait RoundToTwoDecimals {
    fn round_to_two_decimals(self) -> f64;
}
impl RoundToTwoDecimals for f64 {
    fn round_to_two_decimals(self) -> f64 {
        (self * 100.0).round() / 100.0
    }
}

/// Load the routing runtime JSON and build a slug→entry lookup.
fn load_routing_lookup(repo_root: &Path) -> HashMap<String, RoutingEntry> {
    let routing_path = repo_root
        .join("skills")
        .join("SKILL_ROUTING_RUNTIME.json");
    let Ok(text) = fs::read_to_string(&routing_path) else {
        return HashMap::new();
    };
    let Ok(file) = serde_json::from_str::<RoutingRuntimeFile>(&text) else {
        return HashMap::new();
    };

    let keys: Vec<&str> = file.keys.iter().map(|s| s.as_str()).collect();
    let mut lookup = HashMap::new();

    for row in &file.entries {
        let get_val = |key: &str| -> Option<&serde_json::Value> {
            let idx = keys.iter().position(|k| *k == key)?;
            row.get(idx)
        };
        let slug = match get_val("slug").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let layer = get_val("layer")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let description = get_val("description")
            .and_then(|v| v.as_str())
            .map(String::from);
        let trigger_hints = get_val("trigger_hints").and_then(|v| {
            v.as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
        });

        lookup.insert(
            slug.clone(),
            RoutingEntry {
                slug,
                layer,
                description,
                trigger_hints,
            },
        );
    }

    lookup
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate the health manifest with real blended scores for all skills.
///
/// `repo_root` is the project root (parent of `skills/`).
pub fn generate_health_manifest(repo_root: &Path) -> Result<()> {
    let skills_root = crate::paths::skills_root(repo_root);
    let manifest_path = crate::paths::health_json(repo_root);

    // Exclusive advisory lock guard
    let lock_path = manifest_path.with_extension("json.lock");
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(&lock_path)?;
    let _lock = lock_file.lock_exclusive().map_err(|e| {
        FrameworkError::lock(format!("health manifest lock: {e}"))
    })?;

    // Load routing lookup for scoring
    let routing_lookup = load_routing_lookup(repo_root);

    // Try to read existing manifest (preserve runtime metrics if present)
    let mut existing_runtime: HashMap<String, (Option<u64>, Option<u64>)> = HashMap::new();
    if manifest_path.exists() {
        if let Ok(text) = std::fs::read_to_string(&manifest_path) {
            if let Ok(existing) = serde_json::from_str::<HealthManifest>(&text) {
                for (slug, entry) in &existing.skills {
                    existing_runtime
                        .insert(slug.clone(), (entry.route_count, entry.reroute_count));
                }
            }
        }
    }

    // Scan skill directories
    let mut skills = HashMap::new();
    let mut found_slugs = HashSet::new();
    if let Ok(entries) = fs::read_dir(skills_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() {
                    let slug = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                        .to_string();
                    if slug.is_empty() {
                        continue;
                    }
                    found_slugs.insert(slug.clone());

                    let (score, status) =
                        compute_health_score(&slug, true, &routing_lookup);

                    // Preserve runtime metrics from previous manifest
                    let (route_count, reroute_count) = existing_runtime
                        .get(&slug)
                        .copied()
                        .unwrap_or((None, None));

                    skills.insert(
                        slug,
                        HealthEntry {
                            blended_score: score,
                            status,
                            route_count,
                            reroute_count,
                        },
                    );
                }
            }
        }
    }

    // Remove entries for deleted skills
    skills.retain(|k, _| found_slugs.contains(k));

    let manifest = HealthManifest {
        schema_version: constants::SCHEMA_HEALTH.to_string(),
        source_of_truth: true,
        version: 2,
        lifecycle: "active".into(),
        generated_at: utc_now(),
        skills,
    };

    let json_val = serde_json::to_value(&manifest)?;
    core_state_utils::atomic_write::write_atomic_json(&manifest_path, &json_val)
        .map_err(std::io::Error::other)?;
    tracing::info!(
        "health manifest: wrote {} entries (v2, active) to {}",
        manifest.skills.len(),
        manifest_path.display()
    );
    Ok(())
}

/// Generate a UTC ISO-8601 timestamp using std only.
fn utc_now() -> String {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "1970-01-01T00:00:00Z".to_string();
    };
    let secs = duration.as_secs();
    // Simple date calculation (no chrono dependency)
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Approximate year/month/day from days since epoch
    let mut y = 1970u64;
    let mut remaining = days;
    const MAX_YEAR: u64 = 2099;
    loop {
        if y > MAX_YEAR {
            break;
        }
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 1u64;
    for &md in &month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        m += 1;
    }
    format!(
        "{y:04}-{m:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z",
        y = y,
        m = m,
        day = remaining + 1,
    )
}

fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn generate_creates_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        // Create a skill directory under skills/
        let skill_dir = tmp.path().join("skills").join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# test").unwrap();

        generate_health_manifest(tmp.path()).unwrap();
        let manifest_path = tmp.path().join("skills").join("SKILL_HEALTH_MANIFEST.json");
        assert!(manifest_path.exists());

        let text = fs::read_to_string(&manifest_path).unwrap();
        let manifest: HealthManifest = serde_json::from_str(&text).unwrap();
        assert!(manifest.skills.contains_key("test-skill"));
        assert_eq!(manifest.version, 2);
        assert_eq!(manifest.lifecycle, "active");
        assert!(manifest.source_of_truth);
    }

    #[test]
    fn health_score_with_no_routing() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("skills").join("unregistered-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# test").unwrap();

        generate_health_manifest(tmp.path()).unwrap();
        let manifest_path = tmp.path().join("skills").join("SKILL_HEALTH_MANIFEST.json");
        let text = fs::read_to_string(&manifest_path).unwrap();
        let manifest: HealthManifest = serde_json::from_str(&text).unwrap();

        let entry = manifest.skills.get("unregistered-skill").unwrap();
        // Only SKILL.md signal (0.2) → Unhealthy
        assert!((entry.blended_score - 0.2).abs() < 0.01);
        assert_eq!(entry.status, "Unhealthy");
    }

    #[test]
    fn health_score_degraded_range() {
        let (score, status) = compute_health_score("test", true, &HashMap::new());
        // has_skill_md=true (0.2) but no routing → 0.2 → Unhealthy
        assert!((score - 0.2).abs() < 0.01);
        assert_eq!(status, "Unhealthy");
    }

    #[test]
    fn health_score_healthy_with_routing() {
        let mut routing_lookup = HashMap::new();
        routing_lookup.insert(
            "test".to_string(),
            RoutingEntry {
                slug: "test".to_string(),
                layer: "L2".to_string(),
                description: Some("A test skill".to_string()),
                trigger_hints: Some(vec!["hint1".into(), "hint2".into(), "hint3".into()]),
            },
        );
        let (score, status) = compute_health_score("test", true, &routing_lookup);
        // 0.2 + 0.3 + 0.2 + 0.12 + 0.1 = 0.92 → Healthy
        assert!((score - 0.92).abs() < 0.01);
        assert_eq!(status, "Healthy");
    }

    #[test]
    fn removed_skills_pruned() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("skills").join("ephemeral");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# test").unwrap();

        generate_health_manifest(tmp.path()).unwrap();
        assert!(manifest_contains(&tmp, "ephemeral"));

        // Remove the skill directory
        fs::remove_dir_all(&skill_dir).unwrap();
        generate_health_manifest(tmp.path()).unwrap();
        assert!(!manifest_contains(&tmp, "ephemeral"));
    }

    fn manifest_contains(tmp: &tempfile::TempDir, slug: &str) -> bool {
        let path = tmp.path().join("skills").join("SKILL_HEALTH_MANIFEST.json");
        let text = fs::read_to_string(&path).unwrap();
        let manifest: HealthManifest = serde_json::from_str(&text).unwrap();
        manifest.skills.contains_key(slug)
    }
}
