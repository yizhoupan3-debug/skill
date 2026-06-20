//! Workspace management: initialization, file sync, ledger events.
//!
//! Migrated from `tools/autoresearch-rs/src/workspace.rs`.
//!
//! TODO: Full implementation pending. This is a stub for compilation.

use anyhow::Result;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

/// Write content to a file only if it doesn't already exist.
pub fn write_if_missing(path: &Path, content: String) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        fs::write(path, content)?;
    }
    Ok(())
}

/// Append a JSONL ledger event to `run-ledger.jsonl`.
pub fn append_ledger_event(workspace: &Path, kind: &str, payload: Value) -> Result<()> {
    use std::io::Write;
    let event = json!({
        "schema_version": "autoresearch-ledger-v1",
        "event_id": format!("evt_{}", chrono::Utc::now().timestamp_millis()),
        "ts": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "kind": kind,
        "workspace": workspace.display().to_string(),
        "project": workspace.file_name().and_then(|n| n.to_str()).unwrap_or("-"),
        "payload": payload,
    });
    let target = workspace.join("run-ledger.jsonl");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut handle = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(target)?;
    writeln!(handle, "{}", serde_json::to_string(&event)?)?;
    Ok(())
}

/// Append a section to `research-log.md`.
pub fn append_research_log(
    workspace: &Path,
    heading: &str,
    bullets: Vec<String>,
) -> Result<()> {
    use std::io::Write;
    let log_path = workspace.join("research-log.md");
    let date = chrono::Local::now().format("%Y-%m-%d");
    let mut lines = vec![
        String::new(),
        format!("## {date} — {heading}"),
        String::new(),
    ];
    for bullet in bullets {
        lines.push(format!("- {bullet}"));
    }
    lines.push(String::new());
    let mut handle = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    write!(handle, "{}", lines.join("\n"))?;
    Ok(())
}

/// Initialize a new research workspace with default directory structure and state file.
pub fn init_workspace(
    project: &str,
    question: &str,
    base_dir: &Path,
    mode: &str,
) -> Result<std::path::PathBuf> {
    let base = if base_dir.is_absolute() {
        base_dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(base_dir)
    };
    let root = base.join(project);
    for dir in [
        root.clone(),
        root.join("literature"),
        root.join("src"),
        root.join("data"),
        root.join("experiments"),
        root.join("experiments/_templates"),
        root.join("to_human"),
        root.join("paper"),
    ] {
        fs::create_dir_all(dir)?;
    }
    let state_path = root.join("research-state.yaml");
    if state_path.exists() {
        anyhow::bail!(
            "Refusing to overwrite existing workspace: {}",
            root.display()
        );
    }
    let state = crate::claims::lifecycle::default_state(project, question, mode);
    crate::state::dump_state(&state_path, &state)?;
    // Write initial files
    let date = chrono::Local::now().format("%Y-%m-%d");
    fs::write(
        root.join("research-log.md"),
        format!("# Research Log — {project}\n\nQuestion: {question}\n\n## {date} — Workspace initialized\n\n"),
    )?;
    fs::write(
        root.join("findings.md"),
        format!("# Findings — {project}\n\nQuestion: {question}\n\n"),
    )?;
    fs::write(
        root.join("BOOTSTRAP_BRIEF.md"),
        format!("# Bootstrap Brief — {project}\n\nQuestion: {question}\n\n"),
    )?;
    fs::write(
        root.join("literature/NOVELTY_GATE.md"),
        format!("# Novelty Gate — {project}\n\n"),
    )?;
    fs::write(
        root.join("experiments/README.md"),
        format!("# Experiments — {project}\n\n"),
    )?;
    Ok(root)
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn write_if_missing_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        write_if_missing(&path, "content".to_string()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "content");
    }

    #[test]
    fn write_if_missing_does_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "original").unwrap();
        write_if_missing(&path, "new".to_string()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    }

    #[test]
    fn append_ledger_event_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        append_ledger_event(dir.path(), "test", json!({"key": "value"})).unwrap();
        let content = fs::read_to_string(dir.path().join("run-ledger.jsonl")).unwrap();
        assert!(content.contains("test"));
        assert!(content.contains("value"));
    }
}
