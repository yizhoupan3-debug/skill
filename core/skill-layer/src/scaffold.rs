//! Skill scaffold: generate new skill directories from a template.

use crate::frontmatter::{
    RoutingGate, RoutingLayer, RoutingOwner, RoutingPriority, SessionStart,
};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Options for scaffolding a new skill.
#[derive(Debug, Clone)]
pub struct ScaffoldOptions {
    pub name: String,
    pub description: String,
    pub routing_layer: RoutingLayer,
    pub routing_owner: RoutingOwner,
    pub routing_gate: RoutingGate,
    pub routing_priority: RoutingPriority,
    pub session_start: SessionStart,
    pub trigger_hints: Vec<String>,
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// Outcome of a scaffold operation.
#[derive(Debug)]
pub struct ScaffoldResult {
    pub skill_dir: PathBuf,
    pub skill_md_path: PathBuf,
    pub files_created: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ScaffoldError {
    /// The skill directory already exists.
    AlreadyExists(PathBuf),
    /// I/O error during file creation.
    Io(std::io::Error),
}

impl fmt::Display for ScaffoldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists(p) => write!(f, "skill directory already exists: {}", p.display()),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for ScaffoldError {}

impl From<std::io::Error> for ScaffoldError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn routing_layer_str(layer: RoutingLayer) -> &'static str {
    match layer {
        RoutingLayer::L0 => "L0",
        RoutingLayer::L1 => "L1",
        RoutingLayer::L2 => "L2",
        RoutingLayer::L3 => "L3",
        RoutingLayer::L4 => "L4",
    }
}

fn routing_owner_str(owner: RoutingOwner) -> &'static str {
    match owner {
        RoutingOwner::Owner => "owner",
        RoutingOwner::Gate => "gate",
        RoutingOwner::User => "user",
    }
}

fn routing_gate_str(gate: RoutingGate) -> &'static str {
    match gate {
        RoutingGate::None => "none",
        RoutingGate::Artifact => "artifact",
        RoutingGate::Source => "source",
        RoutingGate::Evidence => "evidence",
        RoutingGate::Delegation => "delegation",
        RoutingGate::Approve => "approve",
    }
}

fn routing_priority_str(priority: RoutingPriority) -> &'static str {
    match priority {
        RoutingPriority::P1 => "P1",
        RoutingPriority::P2 => "P2",
        RoutingPriority::P3 => "P3",
    }
}

fn session_start_str(ss: SessionStart) -> &'static str {
    match ss {
        SessionStart::Required => "required",
        SessionStart::Preferred => "preferred",
        SessionStart::Optional => "optional",
        SessionStart::Never => "never",
        SessionStart::NA => "n/a",
    }
}

fn render_skill_md(opts: &ScaffoldOptions) -> String {
    let hints_yaml: String = opts
        .trigger_hints
        .iter()
        .map(|h| format!("  - \"{h}\""))
        .collect::<Vec<_>>()
        .join("\n");

    let short_desc = if opts.description.len() > 120 {
        format!("{}...", &opts.description[..117])
    } else {
        opts.description.clone()
    };

    format!(
        r#"---
name: {name}
description: |
  {description}
routing_layer: {layer}
routing_owner: {owner}
routing_gate: {gate}
routing_priority: {priority}
session_start: {session_start}
short_description: "{short_desc}"
trigger_hints:
{hints}
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: []
risk: low
source: local
---

# {name}

{description}

## When to use

- TODO: describe the activation conditions.

## Do not use

- TODO: describe the boundary conditions.

## Verification

- TODO: describe how to verify the skill completed successfully.
"#,
        name = opts.name,
        description = opts.description,
        layer = routing_layer_str(opts.routing_layer),
        owner = routing_owner_str(opts.routing_owner),
        gate = routing_gate_str(opts.routing_gate),
        priority = routing_priority_str(opts.routing_priority),
        session_start = session_start_str(opts.session_start),
        short_desc = short_desc,
        hints = hints_yaml,
    )
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate a new skill directory with SKILL.md.
///
/// Creates the directory and writes an initial SKILL.md with template content.
/// Call [`register_and_generate`] afterward to add a registry entry and
/// regenerate SKILL.md frontmatter from the registry (source-of-truth).
///
/// Fails if the directory already exists.
pub fn scaffold_skill(
    skills_root: &Path,
    opts: &ScaffoldOptions,
) -> Result<ScaffoldResult, ScaffoldError> {
    crate::validate::validate_skill_name(&opts.name).map_err(|e| {
        ScaffoldError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
    })?;
    let skill_dir = skills_root.join(&opts.name);
    if skill_dir.exists() {
        return Err(ScaffoldError::AlreadyExists(skill_dir));
    }

    fs::create_dir_all(&skill_dir)?;

    let skill_md_path = skill_dir.join("SKILL.md");
    let content = render_skill_md(opts);
    fs::write(&skill_md_path, content)?;

    Ok(ScaffoldResult {
        skill_dir: skill_dir.clone(),
        skill_md_path: skill_md_path.clone(),
        files_created: vec![skill_md_path],
        warnings: vec![],
    })
}

/// Register a scaffolded skill in the runtime registry, then regenerate
/// its SKILL.md frontmatter from the registry (source of truth).
///
/// Call this after `scaffold_skill` to ensure the registry owns all routing
/// metadata and the SKILL.md is a generated artifact.
pub fn register_and_generate(
    repo_root: &Path,
    slug: &str,
    opts: &ScaffoldOptions,
) -> Result<(), String> {
    use serde_json::json;
    use std::collections::HashMap;

    // ---- 1. Add row to SKILL_ROUTING_RUNTIME.json ----
    let runtime_path = crate::paths::runtime_json(repo_root);
    let mut doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&runtime_path).map_err(|e| e.to_string())?
    )
    .map_err(|e| e.to_string())?;

    let keys = doc["keys"]
        .as_array()
        .ok_or_else(|| "runtime JSON missing keys array".to_string())?;
    let col_idx: HashMap<&str, usize> = keys
        .iter()
        .enumerate()
        .filter_map(|(i, k)| k.as_str().map(|s| (s, i)))
        .collect();

    let mut row: Vec<serde_json::Value> = vec![serde_json::Value::Null; keys.len()];
    let skill_path = format!("skills/{slug}/SKILL.md");

    if let Some(&i) = col_idx.get("slug") {
        row[i] = json!(slug);
    }
    if let Some(&i) = col_idx.get("skill_path") {
        row[i] = json!(skill_path);
    }
    if let Some(&i) = col_idx.get("kind") {
        row[i] = json!("skill");
    }
    if let Some(&i) = col_idx.get("description") {
        row[i] = json!(opts.description);
    }
    if let Some(&i) = col_idx.get("layer") {
        row[i] = json!(crate::frontmatter::RoutingLayer::L3);
    }
    if let Some(&i) = col_idx.get("owner") {
        row[i] = json!("owner");
    }
    if let Some(&i) = col_idx.get("gate") {
        row[i] = json!("none");
    }
    if let Some(&i) = col_idx.get("priority") {
        row[i] = json!("P2");
    }
    if let Some(&i) = col_idx.get("session_start") {
        row[i] = json!("n/a");
    }
    if let Some(&i) = col_idx.get("trigger_hints") {
        row[i] = json!(opts.trigger_hints);
    }
    if let Some(&i) = col_idx.get("source") {
        row[i] = json!("local");
    }
    if let Some(&i) = col_idx.get("risk") {
        row[i] = json!("low");
    }
        if let Some(&i) = col_idx.get("metadata") {
        row[i] = json!({
            "version": "0.1.0",
            "platforms": ["supported"],
            "tags": []
        });
    }

    doc["skills"]
        .as_array_mut()
        .ok_or_else(|| "runtime JSON missing skills array".to_string())?
        .push(serde_json::Value::Array(row));

    core_state::utils::atomic_write::write_atomic_json(&runtime_path, &doc)
        .map_err(|e| e.to_string())?;

    // ---- 2. Also add row to SKILL_MANIFEST.json ----
    let manifest_path = crate::paths::manifest_json(repo_root);
    let mut manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?
    )
    .map_err(|e| e.to_string())?;

    let m_keys = manifest["keys"]
        .as_array()
        .ok_or_else(|| "manifest JSON missing keys array".to_string())?;
    let m_col_idx: HashMap<&str, usize> = m_keys
        .iter()
        .enumerate()
        .filter_map(|(i, k)| k.as_str().map(|s| (s, i)))
        .collect();

    let mut m_row: Vec<serde_json::Value> = vec![serde_json::Value::Null; m_keys.len()];
    if let Some(&i) = m_col_idx.get("slug") {
        m_row[i] = json!(slug);
    }
    if let Some(&i) = m_col_idx.get("skill_path") {
        m_row[i] = json!(skill_path);
    }
    if let Some(&i) = m_col_idx.get("description") {
        m_row[i] = json!(opts.description);
    }

    manifest["skills"]
        .as_array_mut()
        .ok_or_else(|| "manifest JSON missing skills array".to_string())?
        .push(serde_json::Value::Array(m_row));

    core_state::utils::atomic_write::write_atomic_json(&manifest_path, &manifest)
        .map_err(|e| e.to_string())?;

    // ---- 3. Regenerate SKILL.md from registry ----
    crate::generate::generate_frontmatter(repo_root, Some(slug), false)?;

    Ok(())
}

/// Dry-run: return what would be created without touching disk.
pub fn scaffold_dry_run(
    skills_root: &Path,
    opts: &ScaffoldOptions,
) -> Result<ScaffoldResult, ScaffoldError> {
    let skill_dir = skills_root.join(&opts.name);
    if skill_dir.exists() {
        return Err(ScaffoldError::AlreadyExists(skill_dir));
    }
    let skill_md_path = skill_dir.join("SKILL.md");
    Ok(ScaffoldResult {
        skill_dir,
        skill_md_path: skill_md_path.clone(),
        files_created: vec![skill_md_path],
        warnings: vec!["dry-run: no files were created".into()],
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_opts() -> ScaffoldOptions {
        ScaffoldOptions {
            name: "my-new-skill".into(),
            description: "Does something useful.".into(),
            routing_layer: RoutingLayer::L3,
            routing_owner: RoutingOwner::Owner,
            routing_gate: RoutingGate::None,
            routing_priority: RoutingPriority::P2,
            session_start: SessionStart::NA,
            trigger_hints: vec!["new skill".into(), "新 skill".into()],
        }
    }

    #[test]
    fn scaffold_creates_directory_and_file() {
        let tmp = tempfile::tempdir().unwrap();
        let result = scaffold_skill(tmp.path(), &test_opts()).unwrap();
        assert!(result.skill_md_path.exists());
        let content = fs::read_to_string(&result.skill_md_path).unwrap();
        assert!(content.contains("name: my-new-skill"));
        assert!(content.contains("routing_layer: L3"));
    }

    #[test]
    fn scaffold_fails_on_existing() {
        let tmp = tempfile::tempdir().unwrap();
        scaffold_skill(tmp.path(), &test_opts()).unwrap();
        let err = scaffold_skill(tmp.path(), &test_opts()).unwrap_err();
        assert!(matches!(err, ScaffoldError::AlreadyExists(_)));
    }

    #[test]
    fn dry_run_does_not_create() {
        let tmp = tempfile::tempdir().unwrap();
        let result = scaffold_dry_run(tmp.path(), &test_opts()).unwrap();
        assert!(!result.skill_md_path.exists());
    }
}
