//! Approval policy generation for SKILL_APPROVAL_POLICY.json.
//!
//! Derives per-skill approval policies from the gate field in SKILL_MANIFEST.json.

use crate::constants;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalEntry {
    gate: String,
    requires_user_approval: bool,
    approval_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalPolicy {
    schema_version: String,
    source_of_truth: bool,
    derived_from: String,
    version: u32,
    policies: HashMap<String, ApprovalEntry>,
}

#[derive(Debug)]
pub enum ApprovalError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl std::error::Error for ApprovalError {}

impl From<std::io::Error> for ApprovalError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for ApprovalError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn approval_type_for_gate(gate: &str) -> (bool, String) {
    match gate {
        "approve" => (true, "pre-execution".into()),
        "artifact" => (false, "artifact-validation".into()),
        "source" => (false, "source-gate".into()),
        "evidence" => (false, "evidence-gate".into()),
        "delegation" => (false, "delegation-decision".into()),
        _ => (false, "no-approval".into()),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate `SKILL_APPROVAL_POLICY.json` from the manifest gate field.
///
/// `repo_root` is the project root (parent of `skills/`).
pub fn generate_approval_policy(repo_root: &Path) -> Result<(), ApprovalError> {
    let skills_root = crate::paths::skills_root(repo_root);
    let manifest_path = crate::paths::manifest_json(repo_root);
    let manifest_text = fs::read_to_string(&manifest_path)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)?;

    let keys: Vec<String> = manifest["keys"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let gate_idx = keys.iter().position(|k| k == "gate");
    let slug_idx = keys.iter().position(|k| k == "slug");

    let mut policies = HashMap::new();

    if let (Some(gate_idx), Some(slug_idx)) = (gate_idx, slug_idx) {
        if let Some(skills_arr) = manifest["skills"].as_array() {
            for row in skills_arr {
                if let (Some(slug), Some(gate)) = (
                    row.get(slug_idx).and_then(|v| v.as_str()),
                    row.get(gate_idx).and_then(|v| v.as_str()),
                ) {
                    if gate != "none" {
                        let (requires_approval, approval_type) = approval_type_for_gate(gate);
                        policies.insert(
                            slug.to_string(),
                            ApprovalEntry {
                                gate: gate.to_string(),
                                requires_user_approval: requires_approval,
                                approval_type,
                            },
                        );
                    }
                }
            }
        }
    }

    let policy = ApprovalPolicy {
        schema_version: constants::SCHEMA_APPROVAL.to_string(),
        source_of_truth: false,
        derived_from: "skills/SKILL_MANIFEST.json".into(),
        version: 1,
        policies,
    };

    let out_path = paths::approval_json(&repo_root);
    let json_val = serde_json::to_value(&policy).map_err(ApprovalError::Json)?;
    core_state::utils::atomic_write::write_atomic_json(&out_path, &json_val)
        .map_err(|e| ApprovalError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    eprintln!(
        "approval policy: wrote {} entries to {}",
        policy.policies.len(),
        out_path.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_from_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        let manifest = serde_json::json!({
            "schema_version": "skill-manifest-v2",
            "keys": ["slug", "gate", "description"],
            "skills": [
                ["deep-research", "approve", "Research skill"],
                ["code-review-deep", "none", "Review skill"],
                ["pdf", "artifact", "PDF skill"],
            ]
        });
        fs::write(
            skills_dir.join("SKILL_MANIFEST.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        generate_approval_policy(tmp.path()).unwrap();
        let out = skills_dir.join("SKILL_APPROVAL_POLICY.json");
        assert!(out.exists());

        let policy: ApprovalPolicy =
            serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
        assert!(policy.policies.contains_key("deep-research"));
        assert!(policy.policies["deep-research"].requires_user_approval);
        assert!(policy.policies.contains_key("pdf"));
        assert!(!policy.policies["pdf"].requires_user_approval);
        // "none" gate should not appear
        assert!(!policy.policies.contains_key("code-review-deep"));
    }
}
