//! Health manifest generation for SKILL_HEALTH_MANIFEST.json.

use crate::constants;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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

#[derive(Debug)]
pub enum HealthError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for HealthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl std::error::Error for HealthError {}

impl From<std::io::Error> for HealthError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for HealthError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Read the existing health manifest (if any) and update it with fresh
/// timestamps.  If no evolution-rs output is available, preserves existing
/// scores and updates the `generated_at` timestamp.
///
/// `repo_root` is the project root (parent of `skills/`).
pub fn generate_health_manifest(
    repo_root: &Path,
) -> Result<(), HealthError> {
    let skills_root = crate::paths::skills_root(repo_root);
    let manifest_path = crate::paths::health_json(repo_root);

    // Try to read existing manifest
    let mut skills = HashMap::new();
    if manifest_path.exists() {
        let text = std::fs::read_to_string(&manifest_path)?;
        if let Ok(existing) = serde_json::from_str::<HealthManifest>(&text) {
            skills = existing.skills;
        }
    }

    // Scan skill directories — any skill not yet in the manifest gets a default entry
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
                    if !skills.contains_key(&slug) {
                        skills.insert(
                            slug,
                            HealthEntry {
                                blended_score: 0.0,
                                status: "Unknown".into(),
                                route_count: None,
                                reroute_count: None,
                            },
                        );
                    }
                }
            }
        }
    }

    let manifest = HealthManifest {
        schema_version: constants::SCHEMA_HEALTH.to_string(),
        source_of_truth: false,
        version: 1,
        lifecycle: "write-only".into(),
        generated_at: utc_now(),
        skills,
    };

    let json_val = serde_json::to_value(&manifest).map_err(|e| HealthError::Json(e))?;
    core_state::utils::atomic_write::write_atomic_json(&manifest_path, &json_val)
        .map_err(|e| HealthError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    eprintln!(
        "health manifest: wrote {} entries to {}",
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
    loop {
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
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
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
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
        assert_eq!(manifest.version, 1);
    }
}
