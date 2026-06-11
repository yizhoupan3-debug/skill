use anyhow::{bail, Context, Result};
use chrono::Local;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::constants::*;
use crate::helpers::*;

// ── state I/O ─────────────────────────────────────────────────────────

pub(crate) fn load_state(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path)?;
    let data: Value = serde_yml::from_str(&raw)
        .or_else(|_| serde_json::from_str(&raw))
        .with_context(|| format!("State file must be YAML/JSON: {}", path.display()))?;
    if !data.is_object() {
        bail!("State file must be a mapping: {}", path.display());
    }
    Ok(ensure_state_defaults(&migrate_state(&data)))
}

pub(crate) fn dump_state(path: &Path, state: &Value) -> Result<()> {
    let mut state_to_write = ensure_state_defaults(state);
    crate::engine::refresh_novelty_views(&mut state_to_write);
    set_key(&mut state_to_write, "schema_version", json!(SCHEMA_VERSION));
    set_key(&mut state_to_write, "updated_at", json!(now_iso()));
    let actions = crate::engine::recommend_next_actions(&state_to_write);
    set_key(&mut state_to_write, "next_actions", json!(actions));
    let rendered = serde_yml::to_string(&state_to_write)?;
    fs::write(path, rendered)?;
    Ok(())
}

pub(crate) fn migrate_state(state: &Value) -> Value {
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

pub(crate) fn default_state(project: &str, question: &str, mode: &str) -> Value {
    let timestamp = now_iso();
    let mut state = json!({
        "schema_version": SCHEMA_VERSION,
        "project": project,
        "question": question,
        "mode": mode,
        "status": "active",
        "stage": STAGE_BOOTSTRAP,
        "current_direction": Value::Null,
        "active_hypothesis": Value::Null,
        "novelty_gate": {
            "status": "pending",
            "claims": [],
            "claim_records": [],
            "draft_claims": [],
            "overlap_summary": Value::Null,
            "differentiation_strategy": Value::Null,
            "decision": Value::Null
        },
        "hypotheses": [],
        "hypothesis_backlog": [],
        "run_history": [],
        "external_research": [],
        "evidence_index": [],
        "blockers": [],
        "decisions": [],
        "environment": Value::Null,
        "git": Value::Null,
        "next_actions": [],
        "created_at": timestamp,
        "updated_at": timestamp
    });
    let actions = crate::engine::recommend_next_actions(&state);
    set_key(&mut state, "next_actions", json!(actions));
    state
}

pub(crate) fn ensure_state_defaults(state: &Value) -> Value {
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

// ── workspace operations ──────────────────────────────────────────────

pub(crate) fn resolve_workspace(path: &Path) -> Result<std::path::PathBuf> {
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

pub(crate) fn ensure_workspace(path: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let workspace = resolve_workspace(path)?;
    let state_path = workspace.join("research-state.yaml");
    if !state_path.exists() {
        bail!("Missing state file: {}", state_path.display());
    }
    Ok((workspace, state_path))
}

pub(crate) fn init_workspace(
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
    crate::render::sync_workspace_files(&root, &state)?;
    Ok(root)
}

// ── environment / git provenance ──────────────────────────────────────

pub(crate) fn capture_environment_fingerprint(workspace: &Path) -> Value {
    json!({
        "rust_version": command_output(&["rustc", "--version"], workspace).unwrap_or_else(|| "unknown".to_string()),
        "platform": std::env::consts::OS,
        "machine": std::env::consts::ARCH,
        "yaml_available": true,
        "external_research_http": true,
        "workspace": workspace.display().to_string(),
    })
}

pub(crate) fn capture_git_provenance(workspace: &Path) -> Value {
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

pub(crate) fn summarize_environment_fingerprint(fingerprint: Option<&Value>) -> String {
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

pub(crate) fn summarize_git_provenance(provenance: Option<&Value>) -> String {
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

// ── ledger / research log ─────────────────────────────────────────────

pub(crate) fn append_ledger_event(workspace: &Path, kind: &str, payload: Value) -> Result<()> {
    use uuid::Uuid;
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

pub(crate) fn append_research_log(
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
