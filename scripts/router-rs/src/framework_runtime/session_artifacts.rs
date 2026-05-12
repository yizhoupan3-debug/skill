use super::constants::{
    ACTIVE_TASK_POINTER_NAME, CONTINUITY_JOURNAL_FILENAME, CONTINUITY_JOURNAL_SCHEMA_VERSION,
    CURRENT_ARTIFACT_DIR, EVIDENCE_INDEX_FILENAME, EVIDENCE_INDEX_SCHEMA_VERSION,
    FOCUS_TASK_POINTER_NAME, FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
    FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION, NEXT_ACTIONS_FILENAME,
    NEXT_ACTIONS_SCHEMA_VERSION, SESSION_SUMMARY_FILENAME, SUPERVISOR_STATE_FILENAME,
    SUPERVISOR_STATE_SCHEMA_VERSION, TASK_REGISTRY_NAME, TERMINAL_VERIFICATION_STATUSES,
    TRACE_METADATA_FILENAME, TRACE_METADATA_SCHEMA_VERSION,
};
use super::json_io::{read_json_strict, read_text_if_exists};
use super::json_value::{
    nonempty_string, safe_slug, value_bool_or_none, value_string_list, value_text,
};
use super::types::{
    ArtifactPaths, ArtifactPayloads, ContinuityJournalInput, SessionArtifactWritePlan,
    SupervisorStateInput, TaskRegistryEntry,
};
use chrono::{Local, SecondsFormat};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

impl SessionArtifactWritePlan {
    fn into_response(self) -> Value {
        json!({
            "ok": true,
            "schema_version": FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION,
            "authority": FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
            "task_id": self.task_id,
            "focus": self.focus,
            "task": self.task,
            "phase": self.phase,
            "status": self.status,
            "summary": self.summary,
            "paths": {
                "session_summary": self.summary_path.display().to_string(),
                "next_actions": self.next_actions_path.display().to_string(),
                "evidence_index": self.evidence_path.display().to_string(),
                "trace_metadata": self.trace_metadata_path.display().to_string(),
                "continuity_journal": self.journal_path.display().to_string(),
            },
            "changed_paths": self.changed_paths,
        })
    }
}

fn resolve_session_repo_root_for_task_ledger(payload: &Value) -> Result<Option<PathBuf>, String> {
    let rr = value_text(payload.get("repo_root"));
    if rr.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(&rr);
    if !path.is_dir() {
        fs::create_dir_all(&path).map_err(|e| {
            format!(
                "framework session artifact writer: repo_root create_dir_all {} failed: {e}",
                path.display()
            )
        })?;
    }
    Ok(Some(super::resolve_repo_root_arg(Some(path.as_path()))?))
}

pub fn write_framework_session_artifacts(payload: Value) -> Result<Value, String> {
    let run = || -> Result<Value, String> {
        let closeout_evaluation = super::enforce_closeout_for_session_payload(&payload)?;
        let mut plan = build_session_artifact_write_plan(&payload)?;
        let sync_repo = plan.repo_root.clone();
        let sync_tid = plan.task_id.clone();
        write_primary_session_artifacts(&mut plan)?;
        write_optional_session_mirror(&mut plan)?;
        write_repo_session_focus(&mut plan)?;
        let mut response = plan.into_response();
        if let Some(ref root) = sync_repo {
            if let Ok(resolved) = super::resolve_repo_root_arg(Some(root.as_path())) {
                crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                    &resolved, &sync_tid,
                );
            }
        }
        if let Some(eval) = closeout_evaluation {
            if let Some(obj) = response.as_object_mut() {
                obj.insert("closeout_evaluation".to_string(), eval);
            }
        }
        Ok(response)
    };
    match resolve_session_repo_root_for_task_ledger(&payload)? {
        Some(resolved) => crate::task_write_lock::apply_task_ledger_mutation(&resolved, run),
        None => run(),
    }
}

fn build_session_artifact_write_plan(payload: &Value) -> Result<SessionArtifactWritePlan, String> {
    let output_dir = value_text(payload.get("output_dir"));
    if output_dir.is_empty() {
        return Err("framework session artifact writer requires output_dir".to_string());
    }
    let task = required_payload_text(payload, "task", "framework session artifact writer")?;
    let phase = defaulted_payload_text(payload, "phase", "implementation");
    let status = defaulted_payload_text(payload, "status", "in_progress");
    let summary = value_text(payload.get("summary"));
    let (next_actions, evidence) = session_artifact_payloads(payload);
    let write_evidence = payload.get("evidence").is_some();
    let task_id = resolve_session_task_id(payload, &task);
    let focus = value_bool_or_none(payload.get("focus")).unwrap_or(false);
    let repo_root = value_text(payload.get("repo_root"));
    let mirror_output_dir = value_text(payload.get("mirror_output_dir"));
    let output_root = PathBuf::from(&output_dir);
    let primary_dir = if payload.get("task_id").is_some() || !repo_root.is_empty() {
        output_root.join(&task_id)
    } else {
        output_root.clone()
    };
    let summary_path = primary_dir.join(SESSION_SUMMARY_FILENAME);
    let next_actions_path = primary_dir.join(NEXT_ACTIONS_FILENAME);
    let evidence_path = primary_dir.join(EVIDENCE_INDEX_FILENAME);
    let trace_metadata_path = primary_dir.join(TRACE_METADATA_FILENAME);
    let journal_path = primary_dir.join(CONTINUITY_JOURNAL_FILENAME);
    let summary_text = render_session_summary(&task, &phase, &status, &summary);
    let next_actions_payload = build_next_actions_payload(&next_actions);
    let evidence_payload = if write_evidence {
        build_evidence_index_payload(&evidence)
    } else {
        read_json_strict(&evidence_path)?
    };
    let trace_metadata_payload = build_trace_metadata_payload(
        &task,
        &phase,
        &status,
        payload.get("trace_metadata"),
        payload.get("matched_skills"),
    );
    let supervisor_state_payload = build_session_supervisor_state_payload(SupervisorStateInput {
        task_id: &task_id,
        task: &task,
        phase: &phase,
        status: &status,
        summary: summary.trim(),
        next_actions_payload: &next_actions_payload,
        evidence_payload: &evidence_payload,
        trace_metadata_payload: &trace_metadata_payload,
        artifact_dir: &primary_dir,
        supervisor_state: payload.get("supervisor_state"),
        execution_contract: payload.get("execution_contract"),
        blockers: payload.get("blockers"),
        continuity: payload.get("continuity"),
    });
    let journal_payload = build_continuity_journal_payload(ContinuityJournalInput {
        task_id: &task_id,
        task: &task,
        phase: &phase,
        status: &status,
        artifact_dir: &primary_dir,
        summary_text: &summary_text,
        next_actions_payload: &next_actions_payload,
        evidence_payload: &evidence_payload,
        trace_metadata_payload: &trace_metadata_payload,
        supervisor_state_payload: &supervisor_state_payload,
        existing_journal: read_json_strict(&journal_path)?,
    });
    Ok(SessionArtifactWritePlan {
        task,
        phase,
        status,
        summary,
        task_id,
        focus,
        repo_root: (!repo_root.is_empty()).then(|| PathBuf::from(repo_root)),
        mirror_output_dir: (!mirror_output_dir.is_empty())
            .then(|| PathBuf::from(mirror_output_dir)),
        summary_path,
        next_actions_path,
        evidence_path,
        trace_metadata_path,
        journal_path,
        write_evidence,
        next_actions_payload,
        evidence_payload,
        trace_metadata_payload,
        supervisor_state_payload,
        journal_payload,
        expected_active_task_hash: nonempty_string(payload.get("expected_active_task_hash")),
        expected_focus_task_hash: nonempty_string(payload.get("expected_focus_task_hash")),
        expected_supervisor_state_hash: nonempty_string(
            payload.get("expected_supervisor_state_hash"),
        ),
        changed_paths: Vec::new(),
    })
}

fn write_primary_session_artifacts(plan: &mut SessionArtifactWritePlan) -> Result<(), String> {
    let summary_text = render_session_summary(&plan.task, &plan.phase, &plan.status, &plan.summary);
    let next_actions_payload = plan.next_actions_payload.clone();
    let evidence_payload = plan.write_evidence.then(|| plan.evidence_payload.clone());
    let trace_metadata_payload = plan.trace_metadata_payload.clone();
    let journal_payload = plan.journal_payload.clone();
    write_session_artifact_set(
        ArtifactPaths {
            summary: &plan.summary_path,
            next_actions: &plan.next_actions_path,
            evidence: &plan.evidence_path,
            trace_metadata: Some(&plan.trace_metadata_path),
            journal: Some(&plan.journal_path),
        },
        ArtifactPayloads {
            summary_text: &summary_text,
            next_actions: &next_actions_payload,
            evidence: evidence_payload.as_ref(),
            trace_metadata: &trace_metadata_payload,
            journal: Some(&journal_payload),
        },
        &mut plan.changed_paths,
    )
}

fn write_optional_session_mirror(plan: &mut SessionArtifactWritePlan) -> Result<(), String> {
    if plan.focus {
        let Some(mirror_root) = plan.mirror_output_dir.clone() else {
            return Ok(());
        };
        let mirror_summary = mirror_root.join(SESSION_SUMMARY_FILENAME);
        let mirror_next_actions = mirror_root.join(NEXT_ACTIONS_FILENAME);
        let mirror_evidence = mirror_root.join(EVIDENCE_INDEX_FILENAME);
        let mirror_trace = mirror_root.join(TRACE_METADATA_FILENAME);
        let mirror_journal = mirror_root.join(CONTINUITY_JOURNAL_FILENAME);
        let summary_text =
            render_session_summary(&plan.task, &plan.phase, &plan.status, &plan.summary);
        let next_actions_payload = plan.next_actions_payload.clone();
        let evidence_payload = plan.write_evidence.then(|| plan.evidence_payload.clone());
        let trace_metadata_payload = plan.trace_metadata_payload.clone();
        let journal_payload = plan.journal_payload.clone();
        write_session_artifact_set(
            ArtifactPaths {
                summary: &mirror_summary,
                next_actions: &mirror_next_actions,
                evidence: &mirror_evidence,
                trace_metadata: Some(&mirror_trace),
                journal: Some(&mirror_journal),
            },
            ArtifactPayloads {
                summary_text: &summary_text,
                next_actions: &next_actions_payload,
                evidence: evidence_payload.as_ref(),
                trace_metadata: &trace_metadata_payload,
                journal: Some(&journal_payload),
            },
            &mut plan.changed_paths,
        )?;
    }
    Ok(())
}

fn write_repo_session_focus(plan: &mut SessionArtifactWritePlan) -> Result<(), String> {
    let Some(repo_root) = plan.repo_root.clone() else {
        return Ok(());
    };
    let mirror_root = repo_root.join("artifacts").join(CURRENT_ARTIFACT_DIR);
    let updated_at = current_local_timestamp();
    if write_task_registry_entry(
        &mirror_root,
        TaskRegistryEntry {
            task_id: &plan.task_id,
            task: &plan.task,
            phase: &plan.phase,
            status: &plan.status,
            resume_allowed: Some(!super::is_terminal(
                &plan.status,
                TERMINAL_VERIFICATION_STATUSES,
            )),
            updated_at: &updated_at,
            focus_task_id: if plan.focus {
                Some(plan.task_id.as_str())
            } else {
                None
            },
        },
    )? {
        plan.changed_paths
            .push(mirror_root.join(TASK_REGISTRY_NAME).display().to_string());
    }
    if plan.focus {
        write_focused_repo_mirrors(plan, &repo_root, &mirror_root, &updated_at)?;
    }
    Ok(())
}

fn write_focused_repo_mirrors(
    plan: &mut SessionArtifactWritePlan,
    repo_root: &Path,
    mirror_root: &Path,
    updated_at: &str,
) -> Result<(), String> {
    let active_pointer = mirror_root.join(ACTIVE_TASK_POINTER_NAME);
    assert_expected_file_hash(
        &active_pointer,
        plan.expected_active_task_hash.as_deref(),
        "active task pointer",
    )?;
    if write_json_if_changed(
        &active_pointer,
        &json!({
            "task_id": plan.task_id,
            "task": plan.task,
            "updated_at": updated_at,
            "task_root": plan.summary_path.parent().map(|path| path.display().to_string()).unwrap_or_default(),
            "session_summary": plan.summary_path.display().to_string(),
            "next_actions": plan.next_actions_path.display().to_string(),
            "evidence_index": plan.evidence_path.display().to_string(),
            "trace_metadata": plan.trace_metadata_path.display().to_string(),
            "continuity_journal": plan.journal_path.display().to_string(),
        }),
    )? {
        plan.changed_paths
            .push(active_pointer.display().to_string());
    }
    let focus_pointer = mirror_root.join(FOCUS_TASK_POINTER_NAME);
    assert_expected_file_hash(
        &focus_pointer,
        plan.expected_focus_task_hash.as_deref(),
        "focus task pointer",
    )?;
    if write_focus_task_pointer(mirror_root, &plan.task_id, &plan.task, updated_at)? {
        plan.changed_paths.push(focus_pointer.display().to_string());
    }
    let supervisor_state_path = repo_root.join(SUPERVISOR_STATE_FILENAME);
    assert_expected_file_hash(
        &supervisor_state_path,
        plan.expected_supervisor_state_hash.as_deref(),
        "supervisor state",
    )?;
    if write_json_if_changed(&supervisor_state_path, &plan.supervisor_state_payload)? {
        plan.changed_paths
            .push(supervisor_state_path.display().to_string());
    }
    Ok(())
}

fn write_focus_task_pointer(
    mirror_root: &Path,
    task_id: &str,
    task: &str,
    updated_at: &str,
) -> Result<bool, String> {
    write_json_if_changed(
        &mirror_root.join(FOCUS_TASK_POINTER_NAME),
        &json!({
            "task_id": task_id,
            "task": task,
            "updated_at": updated_at,
        }),
    )
}

fn write_task_registry_entry(
    mirror_root: &Path,
    entry: TaskRegistryEntry<'_>,
) -> Result<bool, String> {
    let existing = read_json_strict(&mirror_root.join(TASK_REGISTRY_NAME))?;
    let focus_task = entry.focus_task_id.map_or_else(
        || safe_slug(&value_text(existing.get("focus_task_id"))),
        ToString::to_string,
    );
    let mut rows = super::registry_rows_from_payload(&existing);
    let mut replaced = false;
    for row in &mut rows {
        let Some(map) = row.as_object_mut() else {
            continue;
        };
        if safe_slug(&value_text(map.get("task_id"))) != entry.task_id {
            continue;
        }
        map.insert(
            "task_id".to_string(),
            Value::String(entry.task_id.to_string()),
        );
        map.insert("task".to_string(), Value::String(entry.task.to_string()));
        map.insert(
            "updated_at".to_string(),
            Value::String(entry.updated_at.to_string()),
        );
        map.insert(
            "status".to_string(),
            Value::String(entry.status.to_string()),
        );
        map.insert("phase".to_string(), Value::String(entry.phase.to_string()));
        map.insert(
            "resume_allowed".to_string(),
            entry.resume_allowed.map_or(Value::Null, Value::Bool),
        );
        replaced = true;
        break;
    }
    if !replaced {
        rows.push(json!({
            "task_id": entry.task_id,
            "task": entry.task,
            "updated_at": entry.updated_at,
            "status": entry.status,
            "phase": entry.phase,
            "resume_allowed": entry.resume_allowed,
        }));
    }
    let compacted = super::normalize_task_registry_rows(focus_task, rows).0;
    write_json_if_changed(&mirror_root.join(TASK_REGISTRY_NAME), &compacted)
}

fn write_session_artifact_set(
    paths: ArtifactPaths<'_>,
    payloads: ArtifactPayloads<'_>,
    changed_paths: &mut Vec<String>,
) -> Result<(), String> {
    if write_text_if_changed(paths.summary, payloads.summary_text)? {
        changed_paths.push(paths.summary.display().to_string());
    }
    if write_json_if_changed(paths.next_actions, payloads.next_actions)? {
        changed_paths.push(paths.next_actions.display().to_string());
    }
    if let Some(evidence) = payloads.evidence {
        let _lock = crate::runtime_storage::acquire_runtime_path_lock(paths.evidence)?;
        if write_json_if_changed(paths.evidence, evidence)? {
            changed_paths.push(paths.evidence.display().to_string());
        }
    }
    if let Some(path) = paths.trace_metadata {
        write_json_artifact_if_changed(path, payloads.trace_metadata, changed_paths)?;
    }
    if let (Some(path), Some(payload)) = (paths.journal, payloads.journal) {
        write_json_artifact_if_changed(path, payload, changed_paths)?;
    }
    Ok(())
}

fn write_json_artifact_if_changed(
    path: &Path,
    payload: &Value,
    changed_paths: &mut Vec<String>,
) -> Result<(), String> {
    if write_json_if_changed(path, payload)? {
        changed_paths.push(path.display().to_string());
    }
    Ok(())
}

fn session_artifact_payloads(payload: &Value) -> (Vec<String>, Vec<Value>) {
    let next_actions = payload
        .get("next_actions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|item| value_text(Some(&item)))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    let evidence = payload
        .get("evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(Value::is_object)
        .collect::<Vec<_>>();
    (next_actions, evidence)
}

fn required_payload_text(payload: &Value, key: &str, context: &str) -> Result<String, String> {
    let value = value_text(payload.get(key));
    if value.is_empty() {
        Err(format!("{context} requires {key}"))
    } else {
        Ok(value)
    }
}

fn defaulted_payload_text(payload: &Value, key: &str, fallback: &str) -> String {
    let value = value_text(payload.get(key));
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn resolve_session_task_id(payload: &Value, task: &str) -> String {
    let direct = safe_slug(&value_text(payload.get("task_id")));
    if direct.is_empty() {
        build_task_id(task, None)
    } else {
        direct
    }
}

fn render_session_summary(task: &str, phase: &str, status: &str, summary: &str) -> String {
    [
        "# SESSION_SUMMARY".to_string(),
        String::new(),
        format!("- task: {task}"),
        format!("- phase: {phase}"),
        format!("- status: {status}"),
        String::new(),
        "## Summary".to_string(),
        if summary.trim().is_empty() {
            "No summary provided.".to_string()
        } else {
            summary.trim().to_string()
        },
        String::new(),
    ]
    .join("\n")
}

fn build_next_actions_payload(actions: &[String]) -> Value {
    json!({
        "schema_version": NEXT_ACTIONS_SCHEMA_VERSION,
        "next_actions": actions,
    })
}

fn build_evidence_index_payload(entries: &[Value]) -> Value {
    json!({
        "schema_version": EVIDENCE_INDEX_SCHEMA_VERSION,
        "artifacts": entries,
    })
}

fn build_trace_metadata_payload(
    task: &str,
    phase: &str,
    status: &str,
    trace_metadata: Option<&Value>,
    matched_skills: Option<&Value>,
) -> Value {
    let mut payload = trace_metadata
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    payload.insert(
        "schema_version".to_string(),
        Value::String(TRACE_METADATA_SCHEMA_VERSION.to_string()),
    );
    payload.insert("task".to_string(), Value::String(task.to_string()));
    payload.insert("phase".to_string(), Value::String(phase.to_string()));
    payload.insert(
        "verification_status".to_string(),
        Value::String(status.to_string()),
    );
    payload
        .entry("updated_at".to_string())
        .or_insert_with(|| Value::String(current_local_timestamp()));
    if let Some(skills) = normalized_string_array(matched_skills)
        .or_else(|| normalized_string_array(payload.get("matched_skills")))
    {
        payload.insert("matched_skills".to_string(), Value::Array(skills));
    } else {
        payload.insert("matched_skills".to_string(), Value::Array(Vec::new()));
    }
    Value::Object(payload)
}

fn build_session_supervisor_state_payload(input: SupervisorStateInput<'_>) -> Value {
    let mut payload = input
        .supervisor_state
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    payload.insert(
        "schema_version".to_string(),
        Value::String(SUPERVISOR_STATE_SCHEMA_VERSION.to_string()),
    );
    payload.insert(
        "task_id".to_string(),
        Value::String(input.task_id.to_string()),
    );
    payload.insert(
        "task_summary".to_string(),
        Value::String(input.task.to_string()),
    );
    payload.insert(
        "active_phase".to_string(),
        Value::String(input.phase.to_string()),
    );
    payload.insert(
        "updated_at".to_string(),
        Value::String(current_local_timestamp()),
    );
    if !input.summary.is_empty() {
        payload.insert(
            "last_summary".to_string(),
            Value::String(input.summary.to_string()),
        );
    }
    payload.insert(
        "verification".to_string(),
        normalized_verification(payload.get("verification"), input.status),
    );
    payload.insert(
        "continuity".to_string(),
        normalized_continuity(
            input.continuity.or_else(|| payload.get("continuity")),
            input.status,
        ),
    );
    payload.insert(
        "next_actions".to_string(),
        input
            .next_actions_payload
            .get("next_actions")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    payload.insert(
        "evidence_count".to_string(),
        Value::from(super::normalize_evidence_index(input.evidence_payload).len()),
    );
    if let Some(contract) = input
        .execution_contract
        .or_else(|| payload.get("execution_contract"))
    {
        payload.insert("execution_contract".to_string(), contract.clone());
    }
    payload.insert(
        "blockers".to_string(),
        normalized_blockers(input.blockers.or_else(|| payload.get("blockers"))),
    );
    payload.insert(
        "trace_metadata".to_string(),
        input.trace_metadata_payload.clone(),
    );
    payload.insert(
        "artifact_refs".to_string(),
        json!({
            "task_root": input.artifact_dir.display().to_string(),
            "session_summary": input.artifact_dir.join(SESSION_SUMMARY_FILENAME).display().to_string(),
            "next_actions": input.artifact_dir.join(NEXT_ACTIONS_FILENAME).display().to_string(),
            "evidence_index": input.artifact_dir.join(EVIDENCE_INDEX_FILENAME).display().to_string(),
            "trace_metadata": input.artifact_dir.join(TRACE_METADATA_FILENAME).display().to_string(),
            "continuity_journal": input.artifact_dir.join(CONTINUITY_JOURNAL_FILENAME).display().to_string(),
        }),
    );
    Value::Object(payload)
}

fn build_continuity_journal_payload(input: ContinuityJournalInput<'_>) -> Value {
    let summary_sha = sha256_hex(input.summary_text.as_bytes());
    let next_actions_sha = sha256_json(input.next_actions_payload);
    let evidence_sha = sha256_json(input.evidence_payload);
    let trace_sha = sha256_json(input.trace_metadata_payload);
    let supervisor_sha = sha256_json(input.supervisor_state_payload);
    let checkpoint_hash = sha256_hex(
        [
            summary_sha.as_str(),
            next_actions_sha.as_str(),
            evidence_sha.as_str(),
            trace_sha.as_str(),
            supervisor_sha.as_str(),
        ]
        .join(":")
        .as_bytes(),
    );
    let checkpoint = json!({
        "checkpoint_id": checkpoint_hash,
        "task_id": input.task_id,
        "task": input.task,
        "phase": input.phase,
        "status": input.status,
        "created_at": current_local_timestamp(),
        "artifact_hashes": {
            "session_summary": summary_sha,
            "next_actions": next_actions_sha,
            "evidence_index": evidence_sha,
            "trace_metadata": trace_sha,
            "supervisor_state": supervisor_sha,
        },
        "artifact_refs": {
            "task_root": input.artifact_dir.display().to_string(),
            "session_summary": input.artifact_dir.join(SESSION_SUMMARY_FILENAME).display().to_string(),
            "next_actions": input.artifact_dir.join(NEXT_ACTIONS_FILENAME).display().to_string(),
            "evidence_index": input.artifact_dir.join(EVIDENCE_INDEX_FILENAME).display().to_string(),
            "trace_metadata": input.artifact_dir.join(TRACE_METADATA_FILENAME).display().to_string(),
        }
    });
    let mut checkpoints = input
        .existing_journal
        .get("checkpoints")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|item| item.get("checkpoint_id").and_then(Value::as_str) != Some(&checkpoint_hash))
        .collect::<Vec<_>>();
    checkpoints.push(checkpoint);
    while checkpoints.len() > 20 {
        checkpoints.remove(0);
    }
    json!({
        "schema_version": CONTINUITY_JOURNAL_SCHEMA_VERSION,
        "task_id": input.task_id,
        "task": input.task,
        "latest_checkpoint_id": checkpoint_hash,
        "checkpoint_count": checkpoints.len(),
        "checkpoints": checkpoints,
    })
}

fn normalized_verification(existing: Option<&Value>, status: &str) -> Value {
    let mut payload = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    payload.insert(
        "verification_status".to_string(),
        Value::String(status.to_string()),
    );
    payload.insert(
        "updated_at".to_string(),
        Value::String(current_local_timestamp()),
    );
    Value::Object(payload)
}

fn normalized_continuity(existing: Option<&Value>, status: &str) -> Value {
    let mut payload = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let terminal = super::is_terminal(status, super::constants::TERMINAL_VERIFICATION_STATUSES)
        || super::is_terminal(status, super::constants::TERMINAL_STORY_STATES);
    payload.insert(
        "story_state".to_string(),
        Value::String(if terminal { "completed" } else { "active" }.to_string()),
    );
    payload.insert("resume_allowed".to_string(), Value::Bool(!terminal));
    payload.insert(
        "last_updated_at".to_string(),
        Value::String(current_local_timestamp()),
    );
    Value::Object(payload)
}

fn normalized_blockers(existing: Option<&Value>) -> Value {
    let Some(value) = existing else {
        return json!({"open_blockers": []});
    };
    if value.is_object() {
        return value.clone();
    }
    if let Some(items) = normalized_string_array(Some(value)) {
        return json!({"open_blockers": items});
    }
    json!({"open_blockers": []})
}

fn normalized_string_array(value: Option<&Value>) -> Option<Vec<Value>> {
    let values = value_string_list(value);
    if values.is_empty() {
        None
    } else {
        Some(values.into_iter().map(Value::String).collect())
    }
}

fn current_local_timestamp() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn sha256_json(value: &Value) -> String {
    sha256_hex(
        serde_json::to_string(value)
            .unwrap_or_else(|_| "null".to_string())
            .as_bytes(),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(super) fn write_text_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    let existing = read_text_if_exists(path);
    if existing == content {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    crate::atomic_write::write_atomic_text(path, content)?;
    Ok(true)
}

pub(super) fn write_json_if_changed(path: &Path, payload: &Value) -> Result<bool, String> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|err| format!("serialize JSON payload failed: {err}"))?
    );
    write_text_if_changed(path, &serialized)
}

pub(super) fn current_file_hash(path: &Path) -> Result<Option<String>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(sha256_hex(&bytes))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!(
            "read file hash failed for {}: {err}",
            path.display()
        )),
    }
}

pub(super) fn assert_expected_file_hash(
    path: &Path,
    expected_hash: Option<&str>,
    label: &str,
) -> Result<(), String> {
    let Some(expected_hash) = expected_hash else {
        return Ok(());
    };
    let current = current_file_hash(path)?;
    if current.as_deref() == Some(expected_hash) {
        return Ok(());
    }
    Err(format!(
        "stale {label} update rejected for {}; expected hash {expected_hash}, current hash {}",
        path.display(),
        current.unwrap_or_else(|| "<missing>".to_string())
    ))
}

fn build_task_id(task: &str, created_at: Option<&str>) -> String {
    let stamp = created_at
        .unwrap_or(&current_local_timestamp())
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    let slug = safe_slug(task);
    if stamp.is_empty() {
        slug
    } else {
        let suffix = if stamp.len() > 14 {
            &stamp[stamp.len() - 14..]
        } else {
            &stamp
        };
        format!("{slug}-{suffix}")
    }
}
