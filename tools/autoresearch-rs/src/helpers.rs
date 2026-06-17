//! Utility functions: time, paths, templates, JSON accessors, workspace init,
//! and audit logging.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, Timelike, Utc};
use serde_json::{Map, Value, json};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::*;

pub(super) fn now_iso() -> String {
    Utc::now().with_nanosecond_zero().to_rfc3339()
}

pub(super) trait NanosecondZero {
    fn with_nanosecond_zero(self) -> Self;
}

impl NanosecondZero for DateTime<Utc> {
    fn with_nanosecond_zero(self) -> Self {
        self.with_nanosecond(0).unwrap_or(self)
    }
}

pub(super) fn parse_iso_timestamp(value: &str) -> Option<DateTime<Utc>> {
    if value.trim().is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(&value.replace('Z', "+00:00"))
        .ok()
        .map(|ts| ts.with_timezone(&Utc))
}

pub(super) fn days_since(value: &str) -> Option<i64> {
    parse_iso_timestamp(value).map(|ts| (Utc::now() - ts).num_days().max(0))
}

pub(super) fn repo_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("CARGO_MANIFEST_DIR") {
        return Ok(PathBuf::from(root)
            .parent()
            .and_then(Path::parent)
            .unwrap_or(Path::new("."))
            .to_path_buf());
    }
    let current = std::env::current_dir()?;
    for candidate in current.ancestors() {
        if candidate.join("AGENTS.md").exists() && candidate.join("skills").exists() {
            return Ok(candidate.to_path_buf());
        }
    }
    Ok(current)
}

pub(super) fn templates_dir() -> Result<PathBuf> {
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        return Ok(PathBuf::from(manifest).join("templates"));
    }
    Ok(repo_root()?.join(TEMPLATES_RELATIVE))
}

pub(super) fn template_path(name: &str) -> Result<PathBuf> {
    Ok(templates_dir()?.join(name))
}

pub(super) fn load_template(name: &str) -> Result<String> {
    let path = template_path(name)?;
    fs::read_to_string(&path).with_context(|| format!("Missing template: {}", path.display()))
}

pub(super) fn replace_placeholders(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in pairs {
        rendered = rendered.replace(&format!("{{{key}}}"), value);
    }
    rendered
}

pub(super) fn obj_mut(value: &mut Value) -> &mut Map<String, Value> {
    value.as_object_mut().expect("state must be an object")
}

pub(super) fn arr<'a>(value: &'a Value, key: &str) -> &'a Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .expect("expected array after defaults")
}

pub(super) fn arr_mut<'a>(value: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
    obj_mut(value)
        .entry(key.to_string())
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .expect("expected array")
}

pub(super) fn str_key(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string()
}

pub(super) fn str_field(value: &Value, key: &str) -> String {
    str_field_default(value, key, "-")
}

pub(super) fn str_field_default(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

pub(super) fn set_key(value: &mut Value, key: &str, child: Value) {
    obj_mut(value).insert(key.to_string(), child);
}

pub(super) fn string_vec(values: &[String]) -> Value {
    json!(
        values
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
    )
}

pub(super) fn optional_string(value: Option<&str>) -> Value {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(Value::from)
        .unwrap_or(Value::Null)
}

pub(super) fn value_as_string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|item| !item.trim().is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn novelty_gate(value: &Value) -> &Map<String, Value> {
    value
        .get("novelty_gate")
        .and_then(Value::as_object)
        .expect("novelty gate defaults must exist")
}

pub(super) fn novelty_gate_mut(value: &mut Value) -> &mut Map<String, Value> {
    obj_mut(value)
        .entry("novelty_gate".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("novelty_gate must be object")
}

pub(super) fn novelty_arr<'a>(value: &'a Value, key: &str) -> &'a Vec<Value> {
    novelty_gate(value)
        .get(key)
        .and_then(Value::as_array)
        .expect("novelty array default missing")
}

pub(super) fn novelty_value(value: &Value, key: &str) -> Value {
    novelty_gate(value).get(key).cloned().unwrap_or(Value::Null)
}

pub(super) fn novelty_str(value: &Value, key: &str, default: &str) -> String {
    novelty_gate(value)
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

pub(super) fn resolve_workspace(path: &Path) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let candidate = candidate.canonicalize().unwrap_or(candidate);
    if candidate.is_file() {
        if candidate.file_name().and_then(|name| name.to_str()) != Some("research-state.yaml") {
            bail!("Workspace path must be a project directory or research-state.yaml");
        }
        Ok(candidate.parent().unwrap_or(Path::new(".")).to_path_buf())
    } else {
        Ok(candidate)
    }
}

pub(super) fn ensure_workspace(path: &Path) -> Result<(PathBuf, PathBuf)> {
    let workspace = resolve_workspace(path)?;
    let state_path = workspace.join("research-state.yaml");
    if !state_path.exists() {
        bail!("Missing state file: {}", state_path.display());
    }
    Ok((workspace, state_path))
}

pub(super) fn init_workspace(
    project: &str,
    question: &str,
    base_dir: &Path,
    mode: &str,
) -> Result<PathBuf> {
    let base = if base_dir.is_absolute() {
        base_dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(base_dir)
    };
    let root = base.join(project);
    for directory in [
        root.clone(),
        root.join("literature"),
        root.join("src"),
        root.join("data"),
        root.join("experiments"),
        root.join("experiments/_templates"),
        root.join("to_human"),
        root.join("paper"),
    ] {
        fs::create_dir_all(directory)?;
    }
    let state_path = root.join("research-state.yaml");
    if state_path.exists() {
        bail!(
            "Refusing to overwrite existing workspace: {}",
            root.display()
        );
    }
    let state = default_state(project, question, mode);
    dump_state(&state_path, &state)?;
    let date = Local::now().format("%Y-%m-%d").to_string();
    fs::write(
        root.join("research-log.md"),
        replace_placeholders(
            &load_template("research-log.md")?,
            &[
                ("project", project),
                ("question", question),
                ("date", &date),
            ],
        ),
    )?;
    fs::write(
        root.join("findings.md"),
        replace_placeholders(
            &load_template("findings.md")?,
            &[("project", project), ("question", question)],
        ),
    )?;
    fs::write(
        root.join("BOOTSTRAP_BRIEF.md"),
        replace_placeholders(
            &load_template("bootstrap-brief.md")?,
            &[("project", project), ("question", question)],
        ),
    )?;
    fs::write(
        root.join("literature/NOVELTY_GATE.md"),
        replace_placeholders(
            &load_template("novelty-gate.md")?,
            &[("project", project), ("question", question)],
        ),
    )?;
    fs::write(
        root.join("experiments/README.md"),
        replace_placeholders(
            &load_template("experiments-readme.md")?,
            &[("project", project)],
        ),
    )?;
    for (output_name, template_name) in [
        ("HYPOTHESIS_CARD.md", "hypothesis-card.md"),
        ("PROTOCOL_TEMPLATE.md", "protocol-template.md"),
        ("RUN_RECORD_TEMPLATE.md", "run-record-template.md"),
        ("REFLECTION_TEMPLATE.md", "reflection-template.md"),
    ] {
        fs::write(
            root.join("experiments/_templates").join(output_name),
            load_template(template_name)?,
        )?;
    }
    sync_workspace_files(&root, &state)?;
    Ok(root)
}

pub(super) fn append_ledger_event(workspace: &Path, kind: &str, payload: Value) -> Result<()> {
    let event = json!({
        "schema_version": "autoresearch-ledger-v1",
        "event_id": format!("evt_{}", Uuid::new_v4().simple().to_string().chars().take(12).collect::<String>()),
        "ts": now_iso(),
        "kind": kind,
        "workspace": workspace.display().to_string(),
        "project": workspace.file_name().and_then(|n| n.to_str()).unwrap_or("-"),
        "payload": payload,
    });
    let target = workspace.join("run-ledger.jsonl");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut handle = OpenOptions::new().create(true).append(true).open(target)?;
    writeln!(handle, "{}", serde_json::to_string(&event)?)?;
    Ok(())
}

pub(super) fn append_research_log(
    workspace: &Path,
    heading: &str,
    bullets: Vec<String>,
) -> Result<()> {
    let log_path = workspace.join("research-log.md");
    let mut lines = vec![
        String::new(),
        format!("## {} — {}", Local::now().format("%Y-%m-%d"), heading),
        String::new(),
    ];
    for bullet in bullets {
        lines.push(format!("- {bullet}"));
    }
    lines.push(String::new());
    let mut handle = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    write!(handle, "{}", lines.join("\n"))?;
    Ok(())
}

pub(super) fn normalize_limit(limit: usize) -> usize {
    limit.clamp(1, 20)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn str_field_returns_value() {
        let v = json!({"title": "hello"});
        assert_eq!(str_field(&v, "title"), "hello");
    }

    #[test]
    fn str_field_returns_dash_for_missing() {
        let v = json!({});
        assert_eq!(str_field(&v, "title"), "-");
    }

    #[test]
    fn str_field_default_custom_default() {
        let v = json!({});
        assert_eq!(str_field_default(&v, "key", "N/A"), "N/A");
    }

    #[test]
    fn str_key_returns_value() {
        let v = json!({"name": "test"});
        assert_eq!(str_key(&v, "name"), "test");
    }

    #[test]
    fn str_key_returns_dash_for_missing() {
        let v = json!({});
        assert_eq!(str_key(&v, "name"), "-");
    }

    #[test]
    fn arr_returns_array() {
        let v = json!({"items": [1, 2, 3]});
        assert_eq!(arr(&v, "items").len(), 3);
    }

    #[test]
    fn arr_mut_creates_empty_array_if_missing() {
        let mut v = json!({});
        let a = arr_mut(&mut v, "new_key");
        assert!(a.is_empty());
    }

    #[test]
    fn set_key_inserts_value() {
        let mut v = json!({});
        set_key(&mut v, "name", json!("test"));
        assert_eq!(v["name"], "test");
    }

    #[test]
    fn string_vec_creates_json_array() {
        let result = string_vec(&["a".to_string(), "b".to_string()]);
        assert_eq!(result, json!(["a", "b"]));
    }

    #[test]
    fn optional_string_some() {
        assert_eq!(optional_string(Some("hello")), json!("hello"));
    }

    #[test]
    fn optional_string_none() {
        assert_eq!(optional_string(None), json!(null));
    }
}
