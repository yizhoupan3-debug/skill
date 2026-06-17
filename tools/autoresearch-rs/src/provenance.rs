//! Environment fingerprint and Git provenance capture/summarization.

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::Command;

pub(super) fn command_output(args: &[&str], cwd: &Path) -> Option<String> {
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

pub(super) fn capture_environment_fingerprint(workspace: &Path) -> Value {
    json!({
        "rust_version": command_output(&["rustc", "--version"], workspace).unwrap_or_else(|| "unknown".to_string()),
        "platform": std::env::consts::OS,
        "machine": std::env::consts::ARCH,
        "yaml_available": true,
        "external_research_http": true,
        "workspace": workspace.display().to_string(),
    })
}

pub(super) fn capture_git_provenance(workspace: &Path) -> Value {
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
    let mut tracked_changes = 0;
    let mut untracked_changes = 0;
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

pub(super) fn summarize_environment_fingerprint(fingerprint: Option<&Value>) -> String {
    let Some(fingerprint) = fingerprint else {
        return "rust=- platform=- machine=-".to_string();
    };
    let runtime_version = fingerprint
        .get("rust_version")
        .and_then(Value::as_str)
        .unwrap_or("-");
    format!(
        "rust={} platform={} machine={}",
        runtime_version,
        fingerprint
            .get("platform")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        fingerprint
            .get("machine")
            .and_then(Value::as_str)
            .unwrap_or("-")
    )
}

pub(super) fn summarize_git_provenance(provenance: Option<&Value>) -> String {
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
    let short_head = head.chars().take(7).collect::<String>();
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
    format!(
        "{} {} {} tracked={} untracked={}",
        short_head,
        branch,
        dirty,
        provenance
            .get("tracked_changes")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        provenance
            .get("untracked_changes")
            .and_then(Value::as_i64)
            .unwrap_or(0)
    )
}
