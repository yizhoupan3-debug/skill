//! Environment fingerprint and Git provenance capture/summarization.
//!
//! Migrated from `tools/autoresearch-rs/src/provenance.rs`.

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Run a shell command and return stdout on success.
pub fn command_output(args: &[&str], cwd: &Path) -> Option<String> {
    let (program, rest) = args.split_first()?;
    let output = Command::new(program)
        .args(rest)
        .current_dir(cwd)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Capture a snapshot of the runtime environment (Rust version, platform, arch).
pub fn capture_environment_fingerprint(workspace: &Path) -> Value {
    json!({
        "rust_version": command_output(&["rustc", "--version"], workspace)
            .unwrap_or_else(|| "unknown".to_string()),
        "platform": std::env::consts::OS,
        "machine": std::env::consts::ARCH,
        "yaml_available": true,
        "external_research_http": true,
        "workspace": workspace.display().to_string(),
    })
}

/// Capture git HEAD, branch, dirty state, and change counts.
///
/// If the workspace already has `research-state.yaml` with a valid `git` field,
/// the inherited provenance is reused to avoid redundant git calls.
pub fn capture_git_provenance(workspace: &Path) -> Value {
    let head = command_output(&["git", "rev-parse", "HEAD"], workspace);
    if head.is_none() {
        return json!({
            "available": false,
            "workspace": workspace.display().to_string(),
            "head": Value::Null,
            "branch": Value::Null,
            "dirty": Value::Null,
            "tracked_changes": 0,
            "untracked_changes": 0,
        });
    }
    // Reuse existing provenance from state file if available
    let inherited = fs::read_to_string(workspace.join("research-state.yaml"))
        .ok()
        .and_then(|raw| serde_yml::from_str::<Value>(&raw).ok())
        .and_then(|state| state.get("git").cloned())
        .filter(|git| {
            git.get("available")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });
    if let Some(inherited) = inherited {
        return inherited;
    }
    let branch = command_output(&["git", "rev-parse", "--abbrev-ref", "HEAD"], workspace);
    let status = command_output(&["git", "status", "--porcelain"], workspace).unwrap_or_default();
    let mut tracked_changes = 0i64;
    let mut untracked_changes = 0i64;
    let mut dirty = false;
    for line in status.lines().filter(|line| !line.trim().is_empty()) {
        dirty = true;
        if line.starts_with("??") {
            untracked_changes += 1;
        } else {
            tracked_changes += 1;
        }
    }
    json!({
        "available": true,
        "workspace": workspace.display().to_string(),
        "head": head,
        "branch": branch,
        "dirty": dirty,
        "tracked_changes": tracked_changes,
        "untracked_changes": untracked_changes,
    })
}

/// Summarize environment fingerprint as a single-line string.
pub fn summarize_environment_fingerprint(fingerprint: Option<&Value>) -> String {
    let Some(fingerprint) = fingerprint else {
        return "rust=- platform=- machine=-".to_string();
    };
    let rust_version = fingerprint
        .get("rust_version")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let platform = fingerprint
        .get("platform")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let machine = fingerprint
        .get("machine")
        .and_then(Value::as_str)
        .unwrap_or("-");
    format!("rust={rust_version} platform={platform} machine={machine}")
}

/// Summarize git provenance as a single-line string.
pub fn summarize_git_provenance(provenance: Option<&Value>) -> String {
    let Some(provenance) = provenance else {
        return "unavailable".to_string();
    };
    if !provenance
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return "unavailable".to_string();
    }
    let head = provenance
        .get("head")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let short_head: String = head.chars().take(7).collect();
    let branch = provenance
        .get("branch")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let dirty = if provenance
        .get("dirty")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "dirty"
    } else {
        "clean"
    };
    let tracked = provenance
        .get("tracked_changes")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let untracked = provenance
        .get("untracked_changes")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    format!("{short_head} {branch} {dirty} tracked={tracked} untracked={untracked}")
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_environment_fingerprint_none() {
        assert_eq!(
            summarize_environment_fingerprint(None),
            "rust=- platform=- machine=-"
        );
    }

    #[test]
    fn summarize_environment_fingerprint_some() {
        let fp = json!({
            "rust_version": "rustc 1.80.0",
            "platform": "macos",
            "machine": "aarch64",
        });
        let summary = summarize_environment_fingerprint(Some(&fp));
        assert!(summary.contains("rustc 1.80.0"));
        assert!(summary.contains("macos"));
        assert!(summary.contains("aarch64"));
    }

    #[test]
    fn summarize_git_provenance_none() {
        assert_eq!(summarize_git_provenance(None), "unavailable");
    }

    #[test]
    fn summarize_git_provenance_unavailable() {
        let git = json!({"available": false});
        assert_eq!(summarize_git_provenance(Some(&git)), "unavailable");
    }

    #[test]
    fn summarize_git_provenance_available() {
        let git = json!({
            "available": true,
            "head": "abc1234567890",
            "branch": "main",
            "dirty": true,
            "tracked_changes": 3,
            "untracked_changes": 1,
        });
        let summary = summarize_git_provenance(Some(&git));
        assert!(summary.contains("abc1234"));
        assert!(summary.contains("main"));
        assert!(summary.contains("dirty"));
        assert!(summary.contains("tracked=3"));
        assert!(summary.contains("untracked=1"));
    }

    #[test]
    fn summarize_git_provenance_clean() {
        let git = json!({
            "available": true,
            "head": "def5678",
            "branch": "feature",
            "dirty": false,
            "tracked_changes": 0,
            "untracked_changes": 0,
        });
        let summary = summarize_git_provenance(Some(&git));
        assert!(summary.contains("clean"));
    }
}
