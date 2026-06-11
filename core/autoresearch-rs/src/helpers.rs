//! Utility functions: time, paths, templates, JSON accessors, state management,
//! search plan building, and text processing.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local, Timelike, Utc};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
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

pub(super) fn slugify(text: &str) -> String {
    let lowered = text.trim().to_lowercase();
    let cleaned = Regex::new(r"[^a-z0-9]+")
        .unwrap()
        .replace_all(&lowered, "-")
        .to_string();
    let collapsed = Regex::new(r"-+")
        .unwrap()
        .replace_all(&cleaned, "-")
        .trim_matches('-')
        .to_string();
    if collapsed.is_empty() {
        "hypothesis".to_string()
    } else {
        collapsed
    }
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
    json!(values
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>())
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


pub(super) fn ensure_state_defaults(state: &Value) -> Value {
    let mut hydrated = state.clone();
    {
        let root = obj_mut(&mut hydrated);
        root.entry("schema_version")
            .or_insert(json!(SCHEMA_VERSION));
        root.entry("status").or_insert(json!("active"));
        root.entry("stage").or_insert(json!(STAGE_BOOTSTRAP));
        root.entry("mode").or_insert(json!("quick"));
        root.entry("current_direction").or_insert(Value::Null);
        root.entry("active_hypothesis").or_insert(Value::Null);
        root.entry("hypotheses").or_insert(json!([]));
        root.entry("hypothesis_backlog").or_insert(json!([]));
        root.entry("run_history").or_insert(json!([]));
        root.entry("external_research").or_insert(json!([]));
        root.entry("evidence_index").or_insert(json!([]));
        root.entry("blockers").or_insert(json!([]));
        root.entry("decisions").or_insert(json!([]));
        root.entry("environment").or_insert(Value::Null);
        root.entry("git").or_insert(Value::Null);
        root.entry("next_actions").or_insert(json!([]));
        let created_at = root
            .entry("created_at")
            .or_insert_with(|| json!(now_iso()))
            .clone();
        root.entry("updated_at").or_insert(created_at);
    }
    {
        let gate = novelty_gate_mut(&mut hydrated);
        gate.entry("status").or_insert(json!("pending"));
        gate.entry("claims").or_insert(json!([]));
        gate.entry("claim_records").or_insert(json!([]));
        gate.entry("draft_claims").or_insert(json!([]));
        gate.entry("overlap_summary").or_insert(Value::Null);
        gate.entry("differentiation_strategy")
            .or_insert(Value::Null);
        gate.entry("decision").or_insert(Value::Null);
    }
    let updated_at = str_key(&hydrated, "updated_at");
    for hypothesis in arr_mut(&mut hydrated, "hypotheses") {
        let item = hypothesis
            .as_object_mut()
            .expect("hypothesis must be object");
        item.entry("mechanism").or_insert(Value::Null);
        item.entry("falsifiable_prediction").or_insert(Value::Null);
        item.entry("success_threshold").or_insert(Value::Null);
        item.entry("stop_condition").or_insert(Value::Null);
        item.entry("baselines").or_insert(json!([]));
        item.entry("confounders").or_insert(json!([]));
        item.entry("negative_signals").or_insert(json!([]));
        item.entry("minimal_test").or_insert(Value::Null);
        let status = item
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("queued")
            .to_string();
        let valid = [
            "queued",
            "active",
            "needs_reflection",
            "parked",
            "concluded",
        ];
        if !valid.contains(&status.as_str()) {
            item.insert("status".into(), json!("queued"));
        } else {
            item.entry("status").or_insert(json!(status));
        }
        item.entry("status_reason").or_insert(Value::Null);
        let status_updated_at = item
            .get("created_at")
            .cloned()
            .unwrap_or_else(|| json!(updated_at.clone()));
        item.entry("status_updated_at").or_insert(status_updated_at);
    }
    for record in arr_mut(&mut hydrated, "run_history") {
        let item = record.as_object_mut().expect("run record must be object");
        item.entry("novelty_gate_status_at_run")
            .or_insert(Value::Null);
        item.entry("novelty_gate_override").or_insert(json!(false));
        item.entry("override_reason").or_insert(Value::Null);
        item.entry("environment_fingerprint").or_insert(Value::Null);
        item.entry("git_provenance").or_insert(Value::Null);
        item.entry("sanity_checks").or_insert(json!([]));
        item.entry("baseline_result").or_insert(Value::Null);
        item.entry("rules_in").or_insert(json!([]));
        item.entry("rules_out").or_insert(json!([]));
        item.entry("alternative_explanations").or_insert(json!([]));
        item.entry("threats").or_insert(json!([]));
        item.entry("interpretation").or_insert(Value::Null);
        item.entry("finding").or_insert(Value::Null);
        item.entry("decision_delta").or_insert(Value::Null);
        item.entry("reuse_note").or_insert(Value::Null);
        item.entry("applies_to").or_insert(json!([]));
        item.entry("does_not_apply_to").or_insert(json!([]));
    }
    for record in arr_mut(&mut hydrated, "external_research") {
        let item = record
            .as_object_mut()
            .expect("external research record must be object");
        item.entry("claim_id").or_insert(Value::Null);
        item.entry("source").or_insert(json!("all"));
        item.entry("results").or_insert(json!([]));
        item.entry("errors").or_insert(json!([]));
        item.entry("created_at")
            .or_insert_with(|| json!(updated_at.clone()));
    }
    set_key(&mut hydrated, "schema_version", json!(SCHEMA_VERSION));
    hydrated
}

pub(super) fn load_state(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path)?;
    let data: Value = serde_yml::from_str(&raw)
        .or_else(|_| serde_json::from_str(&raw))
        .with_context(|| format!("State file must be YAML/JSON: {}", path.display()))?;
    if !data.is_object() {
        bail!("State file must be a mapping: {}", path.display());
    }
    Ok(ensure_state_defaults(&migrate_state(&data)))
}

pub(super) fn migrate_state(state: &Value) -> Value {
    let mut migrated = state.clone();
    let version = migrated
        .get("schema_version")
        .and_then(Value::as_i64)
        .unwrap_or(2);
    if version >= SCHEMA_VERSION {
        return migrated;
    }
    let run_history = migrated
        .get("run_history")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let decisions = migrated
        .get("decisions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let updated_at = migrated
        .get("updated_at")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    for hypothesis in arr_mut(&mut migrated, "hypotheses") {
        let hypothesis_id = str_field(hypothesis, "id");
        let mut status = hypothesis
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("queued")
            .to_string();
        if ![
            "queued",
            "active",
            "needs_reflection",
            "parked",
            "concluded",
        ]
        .contains(&status.as_str())
        {
            status = "queued".to_string();
        }
        let latest_run = run_history.iter().rev().find(|item| {
            item.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id.as_str())
        });
        let latest_decision = decisions.iter().rev().find(|item| {
            item.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id.as_str())
        });
        let item = hypothesis.as_object_mut().unwrap();
        item.entry("mechanism").or_insert(Value::Null);
        item.entry("falsifiable_prediction").or_insert(Value::Null);
        item.entry("success_threshold").or_insert(Value::Null);
        item.entry("stop_condition").or_insert(Value::Null);
        item.entry("baselines").or_insert(json!([]));
        item.entry("confounders").or_insert(json!([]));
        item.entry("negative_signals").or_insert(json!([]));
        item.entry("minimal_test").or_insert(Value::Null);
        if status == "active"
            && latest_run.is_some()
            && (latest_decision.is_none()
                || latest_decision.and_then(|item| item.get("run_id"))
                    != latest_run.and_then(|item| item.get("run_id")))
        {
            status = "needs_reflection".to_string();
        }
        item.insert("status".into(), json!(status));
        item.entry("status_reason").or_insert(Value::Null);
        let status_updated_at = item
            .get("created_at")
            .cloned()
            .unwrap_or_else(|| json!(updated_at.clone()));
        item.entry("status_updated_at").or_insert(status_updated_at);
    }
    for record in arr_mut(&mut migrated, "run_history") {
        let item = record.as_object_mut().unwrap();
        item.entry("novelty_gate_status_at_run")
            .or_insert(Value::Null);
        item.entry("novelty_gate_override").or_insert(json!(false));
        item.entry("override_reason").or_insert(Value::Null);
        item.entry("environment_fingerprint").or_insert(Value::Null);
        item.entry("git_provenance").or_insert(Value::Null);
        item.entry("sanity_checks").or_insert(json!([]));
        item.entry("baseline_result").or_insert(Value::Null);
        item.entry("rules_in").or_insert(json!([]));
        item.entry("rules_out").or_insert(json!([]));
        item.entry("alternative_explanations").or_insert(json!([]));
        item.entry("threats").or_insert(json!([]));
        item.entry("interpretation").or_insert(Value::Null);
        item.entry("finding").or_insert(Value::Null);
        item.entry("decision_delta").or_insert(Value::Null);
        item.entry("reuse_note").or_insert(Value::Null);
        item.entry("applies_to").or_insert(json!([]));
        item.entry("does_not_apply_to").or_insert(json!([]));
    }
    obj_mut(&mut migrated)
        .entry("external_research")
        .or_insert(json!([]));
    set_key(&mut migrated, "schema_version", json!(SCHEMA_VERSION));
    migrated
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

pub(super) fn init_workspace(project: &str, question: &str, base_dir: &Path, mode: &str) -> Result<PathBuf> {
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

pub(super) fn append_research_log(workspace: &Path, heading: &str, bullets: Vec<String>) -> Result<()> {
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

pub(super) fn stopwords() -> HashSet<&'static str> {
    [
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "in", "into", "is",
        "it", "of", "on", "or", "reduce", "research", "that", "the", "this", "to", "use", "using",
        "with",
    ]
    .into_iter()
    .collect()
}

pub(super) fn compact_words(text: &str, limit: usize) -> Vec<String> {
    let re = Regex::new(r"[A-Za-z0-9][A-Za-z0-9_-]*").unwrap();
    let stops = stopwords();
    let mut filtered = Vec::new();
    for cap in re.find_iter(&text.to_lowercase()) {
        let word = cap.as_str();
        if word.len() <= 2 || stops.contains(word) {
            continue;
        }
        if !filtered.iter().any(|item| item == word) {
            filtered.push(word.to_string());
        }
        if filtered.len() >= limit {
            break;
        }
    }
    filtered
}

pub(super) fn default_required_evidence(axis: &str) -> Vec<String> {
    let axis_lower = axis.to_lowercase();
    if axis_lower.contains("method") || axis_lower.contains("workflow") {
        return vec![
            "Direct overlap papers using the same mechanism".into(),
            "Nearest baseline implementations or orchestration frameworks".into(),
            "Claims about what is structurally different".into(),
        ];
    }
    if axis_lower.contains("setting")
        || axis_lower.contains("domain")
        || axis_lower.contains("task")
    {
        return vec![
            "Prior work in the same domain or task".into(),
            "Recent competitors in the last 3 years".into(),
            "Evidence that the constraint or setting is materially different".into(),
        ];
    }
    if axis_lower.contains("combination") {
        return vec![
            "Papers combining the same building blocks".into(),
            "Closest papers combining two of the three components".into(),
            "Evidence that the composition order or objective is different".into(),
        ];
    }
    vec![
        "Closest prior work for the same core claim".into(),
        "Recent competitors from the last 3 years".into(),
        "Evidence for the exact differentiation sentence".into(),
    ]
}

pub(super) fn build_search_queries(claim: &str, axis: &str) -> Vec<Value> {
    let keywords = compact_words(claim, 6);
    let broad_terms = if keywords.is_empty() {
        claim.to_string()
    } else {
        keywords
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let focused_terms = if keywords.is_empty() {
        claim.to_string()
    } else {
        keywords
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let combination_terms = if keywords.len() >= 4 {
        [
            keywords[0].clone(),
            keywords[1].clone(),
            keywords[keywords.len() - 2].clone(),
            keywords[keywords.len() - 1].clone(),
        ]
        .join(" ")
    } else {
        focused_terms.clone()
    };
    let axis_hint = if axis.trim().is_empty() {
        "claim".to_string()
    } else {
        axis.trim().to_lowercase()
    };
    vec![
        json!({"label": "broad", "query": broad_terms}),
        json!({"label": "focused", "query": format!("{focused_terms} {axis_hint}").trim().to_string()}),
        json!({"label": "recent", "query": format!("{focused_terms} last 3 years").trim().to_string()}),
        json!({"label": "combination", "query": combination_terms}),
    ]
}

pub(super) fn axis_weights(axis: &str) -> (i64, i64, i64) {
    match axis {
        "method" => (5, 2, 3),
        "workflow" => (5, 2, 3),
        "task" => (4, 3, 4),
        "comparison" => (4, 1, 5),
        "setting" => (3, 4, 2),
        "framing" => (2, 1, 4),
        _ => (3, 2, 3),
    }
}

pub(super) fn score_claim_priority(record: &Value) -> Value {
    let axis = str_field_default(record, "axis", "claim").to_lowercase();
    let (mut novelty, mut cost, mut reviewer) = axis_weights(&axis);
    match record.get("overlap").and_then(Value::as_str) {
        Some("low") => novelty += 2,
        Some("medium") => novelty += 1,
        Some("high") => {
            novelty -= 1;
            reviewer += 1;
        }
        _ => {}
    }
    match record.get("confidence").and_then(Value::as_str) {
        Some("high") => cost -= 1,
        Some("low") => cost += 1,
        _ => {}
    }
    match record.get("verdict").and_then(Value::as_str) {
        Some("novel") => novelty += 2,
        Some("defensible") => novelty += 1,
        Some("risky") => {
            reviewer += 1;
            cost += 1;
        }
        Some("not-novel") => {
            novelty -= 2;
            cost += 1;
        }
        _ => {}
    }
    let specificity = str_field(record, "specificity").to_lowercase();
    if specificity.contains("testable") {
        cost -= 1;
    }
    if specificity.contains("paper-facing") {
        reviewer += 1;
    }
    let score = novelty * 3 + reviewer * 2 - cost * 2;
    let label = if score >= 18 {
        "first"
    } else if score >= 13 {
        "next"
    } else {
        "later"
    };
    let reason = if novelty >= reviewer && cost <= 2 {
        "high novelty upside with relatively cheap verification"
    } else if reviewer >= novelty && cost <= 3 {
        "reviewer pressure is high, so checking this early reduces risk"
    } else if cost >= 4 {
        "potentially useful, but verification is expensive"
    } else {
        "worth checking, but not the best first search target"
    };
    let mut out = record.clone();
    let map = out.as_object_mut().expect("claim record must be object");
    map.insert("priority_score".into(), json!(score));
    map.insert("priority_label".into(), json!(label));
    map.insert("priority_reason".into(), json!(reason));
    out
}

pub(super) fn prioritize_claims(claims: &[Value]) -> Vec<Value> {
    let mut scored: Vec<Value> = claims.iter().map(score_claim_priority).collect();
    scored.sort_by(|a, b| {
        let score_a = a.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        let score_b = b.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        score_b
            .cmp(&score_a)
            .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
    });
    for (index, item) in scored.iter_mut().enumerate() {
        item.as_object_mut()
            .unwrap()
            .insert("recommended_order".into(), json!(index + 1));
    }
    scored
}

pub(super) fn top_priority_claim(state: &Value) -> Option<Value> {
    for key in ["claim_records", "draft_claims"] {
        let entries = novelty_gate(state)
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if entries.is_empty() {
            continue;
        }
        let mut ranked = entries;
        ranked.sort_by(|a, b| {
            let order_a = a
                .get("recommended_order")
                .and_then(Value::as_i64)
                .unwrap_or(999);
            let order_b = b
                .get("recommended_order")
                .and_then(Value::as_i64)
                .unwrap_or(999);
            let score_a = a.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
            let score_b = b.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
            order_a
                .cmp(&order_b)
                .then_with(|| score_b.cmp(&score_a))
                .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
        });
        return ranked.into_iter().next();
    }
    None
}

pub(super) fn current_recommended_focus(state: &Value) -> Option<String> {
    let claim = top_priority_claim(state)?;
    Some(format!(
        "{}: {}",
        str_field_default(&claim, "claim_id", "C?"),
        str_field_default(&claim, "claim", "_No claim recorded._")
    ))
}

pub(super) fn build_search_plan_entry(record: &Value) -> Value {
    let claim = str_field(record, "claim");
    let axis = str_field_default(record, "axis", "claim");
    json!({
        "claim_id": str_field_default(record, "claim_id", "C?"),
        "claim": claim,
        "axis": axis,
        "priority_score": record.get("priority_score").cloned().unwrap_or(Value::Null),
        "priority_label": record.get("priority_label").cloned().unwrap_or(Value::Null),
        "priority_reason": record.get("priority_reason").cloned().unwrap_or(Value::Null),
        "recommended_order": record.get("recommended_order").cloned().unwrap_or(Value::Null),
        "keywords": compact_words(&claim, 6),
        "queries": build_search_queries(&claim, &axis),
        "sources": ["Semantic Scholar", "arXiv", "Google Scholar"],
        "required_evidence": default_required_evidence(&axis),
    })
}

pub(super) fn current_search_plan(state: &Value) -> Vec<Value> {
    let source_records = if !novelty_arr(state, "claim_records").is_empty() {
        novelty_arr(state, "claim_records").clone()
    } else {
        novelty_arr(state, "draft_claims").clone()
    };
    let mut plan: Vec<Value> = source_records.iter().map(build_search_plan_entry).collect();
    plan.sort_by(|a, b| {
        let order_a = a
            .get("recommended_order")
            .and_then(Value::as_i64)
            .unwrap_or(999);
        let order_b = b
            .get("recommended_order")
            .and_then(Value::as_i64)
            .unwrap_or(999);
        let score_a = a.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        let score_b = b.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        order_a
            .cmp(&order_b)
            .then_with(|| score_b.cmp(&score_a))
            .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
    });
    plan
}

pub(super) fn refresh_novelty_views(state: &mut Value) {
    let search_plan = current_search_plan(state);
    let recommended_focus = current_recommended_focus(state);
    let brief = current_brief(state);
    let gate = novelty_gate_mut(state);
    gate.insert("search_plan".into(), json!(search_plan));
    gate.insert(
        "recommended_focus".into(),
        recommended_focus.map_or(Value::Null, Value::String),
    );
    gate.insert("brief".into(), brief.unwrap_or(Value::Null));
}

pub(super) fn expected_baselines_for_axis(axis: &str) -> Vec<String> {
    match axis.to_lowercase().as_str() {
        "method" | "workflow" => vec![
            "Closest simple baseline implementation".into(),
            "Nearest orchestration or workflow framework baseline".into(),
            "A stripped-down version without the claimed mechanism".into(),
        ],
        "task" => vec![
            "Closest task-specific prior method".into(),
            "Simple transfer baseline without the claimed novelty".into(),
            "Recent strongest competitor from the last 3 years".into(),
        ],
        "setting" => vec![
            "Same method in an adjacent setting".into(),
            "Simple baseline in the same constraint".into(),
            "Closest unconstrained baseline to show what the setting changes".into(),
        ],
        "comparison" => vec![
            "Closest simple baseline the reviewer will ask about first".into(),
            "A stronger but obvious comparator".into(),
            "An ablated version removing the claimed differentiator".into(),
        ],
        "framing" => vec![
            "Closest paper making a similar framing claim".into(),
            "A simpler framing that could explain the same result".into(),
            "The baseline narrative a reviewer would default to".into(),
        ],
        _ => vec![
            "Closest prior work the reviewer will expect".into(),
            "A simple baseline explanation".into(),
            "The strongest recent competitor in the same area".into(),
        ],
    }
}

pub(super) fn verification_standard_for_priority(label: &str) -> &'static str {
    match label {
        "first" => "You should be able to decide proceed vs reframe after one focused search pass.",
        "next" => "This should be checked after the first claim is clarified, not before.",
        _ => "Useful later, but not strong enough to spend the first search budget on.",
    }
}

pub(super) fn current_brief(state: &Value) -> Option<Value> {
    let top = top_priority_claim(state)?;
    let plan = current_search_plan(state);
    let matching = plan
        .iter()
        .find(|entry| entry.get("claim_id") == top.get("claim_id"));
    let axis = str_field_default(&top, "axis", "claim");
    Some(json!({
        "claim_id": str_field_default(&top, "claim_id", "C?"),
        "claim": str_field_default(&top, "claim", "_No claim recorded._"),
        "axis": axis,
        "priority_label": str_field_default(&top, "priority_label", "later"),
        "priority_score": top.get("priority_score").cloned().unwrap_or(json!(0)),
        "priority_reason": str_field_default(&top, "priority_reason", "_No reason recorded._"),
        "decision_goal": "Decide whether this claim is safe to keep, should be reframed, or should be dropped.",
        "verification_standard": verification_standard_for_priority(&str_field_default(&top, "priority_label", "later")),
        "sources": matching.and_then(|item| item.get("sources")).cloned().unwrap_or(json!(["Semantic Scholar", "arXiv", "Google Scholar"])),
        "queries": matching.and_then(|item| item.get("queries")).cloned().unwrap_or_else(|| json!(build_search_queries(&str_field(&top, "claim"), &axis))),
        "required_evidence": matching.and_then(|item| item.get("required_evidence")).cloned().unwrap_or_else(|| json!(default_required_evidence(&axis))),
        "expected_baselines": expected_baselines_for_axis(&axis),
    }))
}

pub(super) fn normalize_limit(limit: usize) -> usize {
    limit.clamp(1, 20)
}

pub(super) fn xml_text_between(raw: &str, tag: &str) -> Option<String> {
    let pattern = Regex::new(&format!(r"(?s)<{tag}(?:\s[^>]*)?>(.*?)</{tag}>")).ok()?;
    let captures = pattern.captures(raw)?;
    Some(decode_xml_entities(captures.get(1)?.as_str().trim()))
}

pub(super) fn decode_xml_entities(raw: &str) -> String {
    raw.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn markdown_link(value: Option<&str>) -> String {
    value
        .filter(|item| !item.trim().is_empty())
        .map(|item| format!("[link]({})", item.trim()))
        .unwrap_or_else(|| "-".into())
}
