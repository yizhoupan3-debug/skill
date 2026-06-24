//! Skill dependency graph: cycle detection, transitive deps, loadout validation.
//!
//! Skills declare optional `dependencies` in their SKILL.md frontmatter.
//! This module builds a directed graph and provides analysis functions.

use crate::frontmatter_parser;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Dependency graph for all skills.
#[derive(Debug, Clone)]
pub struct SkillDependencyGraph {
    /// slug → list of skills it requires.
    requires: HashMap<String, Vec<String>>,
    /// Pairs of skills that conflict with each other.
    conflicts: Vec<(String, String)>,
    /// slug → list of skills it can overlay onto.
    overlays: HashMap<String, Vec<String>>,
}

/// Errors during graph construction.
#[derive(Debug)]
pub enum DependencyError {
    Io(std::io::Error),
    CycleDetected(Vec<String>),
}

impl fmt::Display for DependencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::CycleDetected(path) => write!(f, "dependency cycle detected: {:?}", path),
        }
    }
}

impl std::error::Error for DependencyError {}

impl From<std::io::Error> for DependencyError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Error when a proposed loadout is invalid.
#[derive(Debug)]
pub enum LoadoutValidationError {
    /// A required dependency is missing from the loadout.
    MissingDependency {
        slug: String,
        requires: String,
    },
    /// Two skills in the loadout conflict.
    Conflict {
        skill_a: String,
        skill_b: String,
    },
}

impl fmt::Display for LoadoutValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDependency { slug, requires } => {
                write!(
                    f,
                    "`{slug}` requires `{requires}` which is not in the loadout"
                )
            }
            Self::Conflict { skill_a, skill_b } => {
                write!(
                    f,
                    "`{skill_a}` and `{skill_b}` conflict and cannot coexist in a loadout"
                )
            }
        }
    }
}

impl std::error::Error for LoadoutValidationError {}

// ---------------------------------------------------------------------------
// SkillDependencyGraph
// ---------------------------------------------------------------------------

impl SkillDependencyGraph {
    /// Build the dependency graph from all SKILL.md files in `skills_root`.
    pub fn from_skills_dir(skills_root: &Path) -> Result<Self, DependencyError> {
        let mut requires = HashMap::new();
        let mut conflicts = Vec::new();
        let mut overlays = HashMap::new();

        let entries = fs::read_dir(skills_root)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            let slug = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();

            let text = fs::read_to_string(&skill_md)?;
            if let Ok(fm) = frontmatter_parser::parse_frontmatter(&text) {
                if let Some(deps) = fm.dependencies {
                    if !deps.requires.is_empty() {
                        requires.insert(slug.clone(), deps.requires);
                    }
                    for conflict in &deps.conflicts_with {
                        conflicts.push((slug.clone(), conflict.clone()));
                    }
                    if !deps.provides_overlay_for.is_empty() {
                        overlays.insert(slug.clone(), deps.provides_overlay_for);
                    }
                }
            }
        }

        Ok(Self {
            requires,
            conflicts,
            overlays,
        })
    }

    /// Detect cycles in the requires graph using DFS.
    ///
    /// Returns all cycles found (each as a list of slugs forming the cycle).
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        let mut path = Vec::new();

        for slug in self.requires.keys() {
            if !visited.contains(slug) {
                self.dfs_cycle(slug, &mut visited, &mut stack, &mut path, &mut cycles);
            }
        }
        cycles
    }

    fn dfs_cycle(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        if stack.contains(node) {
            // Found a cycle — extract the cycle from path
            if let Some(start) = path.iter().position(|s| s == node) {
                let mut cycle: Vec<String> = path[start..].to_vec();
                cycle.push(node.to_string());
                cycles.push(cycle);
            }
            return;
        }
        if visited.contains(node) {
            return;
        }
        visited.insert(node.to_string());
        stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(deps) = self.requires.get(node) {
            for dep in deps {
                self.dfs_cycle(dep, visited, stack, path, cycles);
            }
        }

        path.pop();
        stack.remove(node);
    }

    /// Get all transitive dependencies of a skill (including itself).
    pub fn transitive_deps(&self, slug: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(slug.to_string());

        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());
            result.push(current.clone());

            if let Some(deps) = self.requires.get(&current) {
                for dep in deps {
                    if !visited.contains(dep) {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }
        result
    }

    /// Validate that a proposed loadout has no missing deps or conflicts.
    pub fn validate_loadout(
        &self,
        slugs: &[String],
    ) -> Result<(), Vec<LoadoutValidationError>> {
        let loadout_set: HashSet<&String> = slugs.iter().collect();
        let mut errors = Vec::new();

        // Check for missing dependencies
        for slug in slugs {
            if let Some(deps) = self.requires.get(slug) {
                for dep in deps {
                    if !loadout_set.contains(dep) {
                        errors.push(LoadoutValidationError::MissingDependency {
                            slug: slug.clone(),
                            requires: dep.clone(),
                        });
                    }
                }
            }
        }

        // Check for conflicts
        for (a, b) in &self.conflicts {
            if loadout_set.contains(a) && loadout_set.contains(b) {
                errors.push(LoadoutValidationError::Conflict {
                    skill_a: a.clone(),
                    skill_b: b.clone(),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn create_skill_with_deps(skills_root: &Path, slug: &str, requires: &[&str]) {
        let dir = skills_root.join(slug);
        fs::create_dir_all(&dir).unwrap();
        let deps_yaml = if requires.is_empty() {
            String::new()
        } else {
            let items: Vec<String> = requires.iter().map(|r| format!("  - {r}")).collect();
            format!("dependencies:\n  requires:\n{}\n", items.join("\n"))
        };
        let content = format!(
            r#"---
name: {slug}
description: Test
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
trigger_hints: [test]
{deps_yaml}
---
# {slug}
"#,
        );
        fs::write(dir.join("SKILL.md"), content).unwrap();
    }

    #[test]
    fn no_cycles_in_simple_chain() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_skill_with_deps(root, "a", &["b"]);
        create_skill_with_deps(root, "b", &["c"]);
        create_skill_with_deps(root, "c", &[]);

        let graph = SkillDependencyGraph::from_skills_dir(root).unwrap();
        assert!(graph.detect_cycles().is_empty());
    }

    #[test]
    fn detects_cycle() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_skill_with_deps(root, "a", &["b"]);
        create_skill_with_deps(root, "b", &["c"]);
        create_skill_with_deps(root, "c", &["a"]);

        let graph = SkillDependencyGraph::from_skills_dir(root).unwrap();
        let cycles = graph.detect_cycles();
        assert!(!cycles.is_empty());
    }

    #[test]
    fn transitive_deps() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_skill_with_deps(root, "a", &["b"]);
        create_skill_with_deps(root, "b", &["c"]);
        create_skill_with_deps(root, "c", &[]);

        let graph = SkillDependencyGraph::from_skills_dir(root).unwrap();
        let deps = graph.transitive_deps("a");
        assert!(deps.contains(&"a".to_string()));
        assert!(deps.contains(&"b".to_string()));
        assert!(deps.contains(&"c".to_string()));
    }

    #[test]
    fn validate_loadout_catches_missing_dep() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_skill_with_deps(root, "a", &["b"]);
        create_skill_with_deps(root, "b", &[]);

        let graph = SkillDependencyGraph::from_skills_dir(root).unwrap();
        let result = graph.validate_loadout(&["a".into()]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, LoadoutValidationError::MissingDependency { .. })));
    }

    #[test]
    fn validate_loadout_ok_when_deps_satisfied() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_skill_with_deps(root, "a", &["b"]);
        create_skill_with_deps(root, "b", &[]);

        let graph = SkillDependencyGraph::from_skills_dir(root).unwrap();
        let result = graph.validate_loadout(&["a".into(), "b".into()]);
        assert!(result.is_ok());
    }
}
