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
    pub user_invocable: bool,
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
user-invocable: {user_invocable}
disable-model-invocation: false
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
        user_invocable = opts.user_invocable,
        short_desc = short_desc,
        hints = hints_yaml,
    )
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate a new skill directory with SKILL.md.
///
/// Fails if the directory already exists.
pub fn scaffold_skill(
    skills_root: &Path,
    opts: &ScaffoldOptions,
) -> Result<ScaffoldResult, ScaffoldError> {
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
            user_invocable: true,
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
