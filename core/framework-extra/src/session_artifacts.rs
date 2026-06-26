use fr_utils::constants::{
    CURRENT_ARTIFACT_DIR, EVIDENCE_INDEX_FILENAME, EVIDENCE_INDEX_SCHEMA_VERSION,
    NEXT_ACTIONS_FILENAME, NEXT_ACTIONS_SCHEMA_VERSION, SESSION_SUMMARY_FILENAME,
    SUPERVISOR_STATE_FILENAME, SUPERVISOR_STATE_SCHEMA_VERSION, TASK_POINTERS_FILENAME,
    TASK_POINTERS_SCHEMA_VERSION, TERMINAL_STORY_STATES, TERMINAL_VERIFICATION_STATUSES,
    TRACE_METADATA_FILENAME, TRACE_METADATA_SCHEMA_VERSION,
};
use fr_utils::json_io::read_json_strict;
use fr_utils::json_value::{
    build_task_id, nonempty_string, safe_slug, value_bool_or_none, value_string_list, value_text,
};
use fr_utils::util::{defaulted_payload_text, required_payload_text};
use fr_utils::types::{
    ArtifactPaths, ArtifactPayloads, SessionArtifactWritePlan, SupervisorStateInput,
    TaskRegistryEntry,
};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

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
    Ok(Some(framework_kernel::repo_roots::resolve_repo_root_arg(Some(path.as_path()))?))
}

pub fn write_framework_session_artifacts(payload: Value) -> Result<Value, String> {
    let run = || -> Result<Value, String> {
        let closeout_evaluation = super::closeout::enforce_closeout_for_session_payload(&payload)?;
        let mut plan = build_session_artifact_write_plan(&payload)?;
        let sync_repo = plan.repo_root.clone();
        let sync_tid = plan.task_id.clone();
        write_primary_session_artifacts(&mut plan)?;
        write_optional_session_mirror(&mut plan)?;
        write_repo_session_focus(&mut plan)?;
        let mut response = plan.into_response();
        if let Some(ref root) = sync_repo
            && let Ok(resolved) = framework_kernel::repo_roots::resolve_repo_root_arg(Some(root.as_path())) {
                core_state::task_state_aggregate::sync_task_state_aggregate_best_effort(
                    &resolved, &sync_tid,
                );
            }
        if let Some(eval) = closeout_evaluation
            && let Some(obj) = response.as_object_mut() {
                obj.insert("closeout_evaluation".to_string(), eval);
            }
        Ok(response)
    };
    match resolve_session_repo_root_for_task_ledger(&payload)? {
        Some(resolved) => core_state_utils::task_write_lock::apply_task_ledger_mutation(&resolved, run).map_err(|e| e.to_string()),
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
    let update_registry_only_if_known =
        value_bool_or_none(payload.get("update_registry_only_if_known")).unwrap_or(false);
    let repo_root = value_text(payload.get("repo_root"));
    let mirror_output_dir = value_text(payload.get("mirror_output_dir"));
    let output_root = PathBuf::from(&output_dir);
    let primary_dir = if payload.get("task_id").is_some() || !repo_root.is_empty() {
        output_root.join(&task_id)
    } else {
        output_root.clone()
    };
    let summary_path = primary_dir.join(SESSION_SUMMARY_FILENAME);
    let evidence_path = primary_dir.join(EVIDENCE_INDEX_FILENAME);
    let _summary_text = render_session_summary(&task, &phase, &status, &summary);
    let evidence_payload = if write_evidence {
        build_evidence_index_payload(&evidence)
    } else {
        read_json_strict(&evidence_path)?
    };
    let next_actions_path = primary_dir.join(NEXT_ACTIONS_FILENAME);
    let next_actions_payload = json!({
        "schema_version": NEXT_ACTIONS_SCHEMA_VERSION,
        "next_actions": &next_actions,
    });
    let trace_metadata_path = primary_dir.join(TRACE_METADATA_FILENAME);
    let trace_metadata_payload = json!({
        "schema_version": TRACE_METADATA_SCHEMA_VERSION,
        "task": &task,
        "matched_skills": payload.get("matched_skills").cloned().unwrap_or_else(|| json!([])),
    });
    let supervisor_state_payload = build_session_supervisor_state_payload(SupervisorStateInput {
        task_id: &task_id,
        task: &task,
        phase: &phase,
        status: &status,
        summary: summary.trim(),
        next_actions_payload: &next_actions_payload,
        evidence_payload: &evidence_payload,
        matched_skills: payload.get("matched_skills"),
        artifact_dir: &primary_dir,
        supervisor_state: payload.get("supervisor_state"),
        execution_contract: payload.get("execution_contract"),
        blockers: payload.get("blockers"),
        continuity: payload.get("continuity"),
    });
    Ok(SessionArtifactWritePlan {
        task,
        phase,
        status,
        summary,
        task_id,
        focus,
        update_registry_only_if_known,
        repo_root: (!repo_root.is_empty()).then(|| PathBuf::from(repo_root)),
        mirror_output_dir: (!mirror_output_dir.is_empty())
            .then(|| PathBuf::from(mirror_output_dir)),
        summary_path,
        next_actions_path,
        next_actions_payload,
        trace_metadata_path,
        trace_metadata_payload,
        evidence_path,
        write_evidence,
        evidence_payload,
        supervisor_state_payload,
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
    let evidence_payload = plan.write_evidence.then(|| plan.evidence_payload.clone());
    write_session_artifact_set(
        ArtifactPaths {
            summary: &plan.summary_path,
            evidence: &plan.evidence_path,
        },
        ArtifactPayloads {
            summary_text: &summary_text,
            evidence: evidence_payload.as_ref(),
        },
        &mut plan.changed_paths,
    )?;
    if write_json_if_changed(&plan.next_actions_path, &plan.next_actions_payload)? {
        plan.changed_paths
            .push(plan.next_actions_path.display().to_string());
    }
    if write_json_if_changed(&plan.trace_metadata_path, &plan.trace_metadata_payload)? {
        plan.changed_paths
            .push(plan.trace_metadata_path.display().to_string());
    }
    Ok(())
}

fn write_optional_session_mirror(plan: &mut SessionArtifactWritePlan) -> Result<(), String> {
    if plan.focus {
        let Some(mirror_root) = plan.mirror_output_dir.clone() else {
            return Ok(());
        };
        let mirror_summary = mirror_root.join(SESSION_SUMMARY_FILENAME);
        let mirror_evidence = mirror_root.join(EVIDENCE_INDEX_FILENAME);
        let summary_text =
            render_session_summary(&plan.task, &plan.phase, &plan.status, &plan.summary);
        let evidence_payload = plan.write_evidence.then(|| plan.evidence_payload.clone());
        write_session_artifact_set(
            ArtifactPaths {
                summary: &mirror_summary,
                evidence: &mirror_evidence,
            },
            ArtifactPayloads {
                summary_text: &summary_text,
                evidence: evidence_payload.as_ref(),
            },
            &mut plan.changed_paths,
        )?;
        let mirror_next_actions = mirror_root.join(NEXT_ACTIONS_FILENAME);
        if write_json_if_changed(&mirror_next_actions, &plan.next_actions_payload)? {
            plan.changed_paths
                .push(mirror_next_actions.display().to_string());
        }
        let mirror_trace = mirror_root.join(TRACE_METADATA_FILENAME);
        if write_json_if_changed(&mirror_trace, &plan.trace_metadata_payload)? {
            plan.changed_paths.push(mirror_trace.display().to_string());
        }
    }
    Ok(())
}

fn write_repo_session_focus(plan: &mut SessionArtifactWritePlan) -> Result<(), String> {
    let Some(repo_root) = plan.repo_root.clone() else {
        return Ok(());
    };
    let mirror_root = repo_root.join("artifacts").join(CURRENT_ARTIFACT_DIR);
    let updated_at = framework_kernel::time::current_local_timestamp();
    let registry_known = task_id_known_in_task_pointers(&mirror_root, &plan.task_id);
    let should_touch_registry = !plan.update_registry_only_if_known || registry_known;
    if should_touch_registry
        && write_task_pointers_entry(
            &mirror_root,
            TaskRegistryEntry {
                task_id: &plan.task_id,
                task: &plan.task,
                phase: &plan.phase,
                status: &plan.status,
                resume_allowed: Some(
                    !crate::util::is_terminal(&plan.status, TERMINAL_VERIFICATION_STATUSES)
                        && !crate::util::is_terminal(&plan.status, TERMINAL_STORY_STATES),
                ),
                updated_at: &updated_at,
                focus_task_id: if plan.focus {
                    Some(plan.task_id.as_str())
                } else {
                    None
                },
            },
        )?
    {
        plan.changed_paths.push(
            mirror_root
                .join(TASK_POINTERS_FILENAME)
                .display()
                .to_string(),
        );
    }
    if plan.focus {
        write_focused_repo_mirrors(plan, &repo_root, &mirror_root, &updated_at)?;
    } else {
        write_supervisor_state_for_non_focus_checkpoint(plan, &repo_root)?;
    }
    Ok(())
}

/// ADR-001: Stop/automatic checkpoint (`focus: false`) refreshes task artifacts and syncs
/// `.supervisor_state.json` to the checkpoint `task_id` without moving active/focus pointers.
fn write_supervisor_state_for_non_focus_checkpoint(
    plan: &mut SessionArtifactWritePlan,
    repo_root: &Path,
) -> Result<(), String> {
    let supervisor_state_path = repo_root.join(SUPERVISOR_STATE_FILENAME);
    if let Some(expected) = plan.expected_supervisor_state_hash.as_deref() {
        assert_expected_file_hash(&supervisor_state_path, Some(expected), "supervisor state")?;
    }
    if write_json_if_changed(&supervisor_state_path, &plan.supervisor_state_payload)? {
        plan.changed_paths
            .push(supervisor_state_path.display().to_string());
    }
    Ok(())
}

fn write_focused_repo_mirrors(
    plan: &mut SessionArtifactWritePlan,
    repo_root: &Path,
    mirror_root: &Path,
    updated_at: &str,
) -> Result<(), String> {
    let active_pointer = mirror_root.join("active_task.json");
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
            "session_summary": plan.summary_path.display().to_string(),
        }),
    )? {
        plan.changed_paths
            .push(active_pointer.display().to_string());
    }
    let focus_pointer = mirror_root.join("focus_task.json");
    assert_expected_file_hash(
        &focus_pointer,
        plan.expected_focus_task_hash.as_deref(),
        "focus task pointer",
    )?;
    if write_json_if_changed(
        &focus_pointer,
        &json!({
            "task_id": plan.task_id,
            "task": plan.task,
        }),
    )? {
        plan.changed_paths.push(focus_pointer.display().to_string());
    }
    let registry_path = mirror_root.join("task_registry.json");
    let existing_registry = read_json_strict(&registry_path).unwrap_or_else(|_| json!({}));
    let mut registry_rows = super::util::registry_rows_from_payload(&existing_registry);
    let mut found = false;
    for row in &mut registry_rows {
        if let Some(map) = row.as_object_mut()
            && safe_slug(&value_text(map.get("task_id"))) == plan.task_id {
                map.insert("task".to_string(), Value::String(plan.task.clone()));
                map.insert("phase".to_string(), Value::String(plan.phase.clone()));
                map.insert("status".to_string(), Value::String(plan.status.clone()));
                map.insert(
                    "updated_at".to_string(),
                    Value::String(updated_at.to_string()),
                );
                map.insert("resume_allowed".to_string(), Value::Bool(true));
                found = true;
                break;
            }
    }
    if !found {
        registry_rows.push(json!({
            "task_id": plan.task_id,
            "task": plan.task,
            "phase": plan.phase,
            "status": plan.status,
            "updated_at": updated_at,
            "resume_allowed": true,
        }));
    }
    let (normalized_registry, _, _) =
        super::util::normalize_task_registry_rows(plan.task_id.clone(), registry_rows);
    if write_json_if_changed(&registry_path, &normalized_registry)? {
        plan.changed_paths.push(registry_path.display().to_string());
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

fn task_id_known_in_task_pointers(mirror_root: &Path, task_id: &str) -> bool {
    let registry_path = mirror_root.join(TASK_POINTERS_FILENAME);
    let Ok(existing) = read_json_strict(&registry_path) else {
        return false;
    };
    super::util::registry_rows_from_payload(&existing)
        .iter()
        .any(|row| safe_slug(&value_text(row.get("task_id"))) == task_id)
}

fn write_task_pointers_entry(
    mirror_root: &Path,
    entry: TaskRegistryEntry<'_>,
) -> Result<bool, String> {
    let existing =
        read_json_strict(&mirror_root.join(TASK_POINTERS_FILENAME)).unwrap_or_else(|_| json!({}));
    let focus_task = entry.focus_task_id.map_or_else(
        || safe_slug(&value_text(existing.get("focus_task_id"))),
        ToString::to_string,
    );
    let mut rows = super::util::registry_rows_from_payload(&existing);
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
    let compacted = super::util::normalize_task_registry_rows(focus_task, rows).0;
    // Merge compacted registry back into TASK_POINTERS.json
    let mut out = json!({
        "schema_version": TASK_POINTERS_SCHEMA_VERSION,
    });
    if let Some(obj) = out.as_object_mut() {
        if let Some(tid) = existing.get("active_task_id") {
            obj.insert("active_task_id".to_string(), tid.clone());
        }
        if let Some(ftid) = compacted.get("focus_task_id") {
            obj.insert("focus_task_id".to_string(), ftid.clone());
        }
        if let Some(tasks) = compacted.get("tasks") {
            obj.insert("tasks".to_string(), tasks.clone());
        }
    }
    write_json_if_changed(&mirror_root.join(TASK_POINTERS_FILENAME), &out).map_err(|e| e.to_string())
}

fn write_session_artifact_set(
    paths: ArtifactPaths<'_>,
    payloads: ArtifactPayloads<'_>,
    changed_paths: &mut Vec<String>,
) -> Result<(), String> {
    // Lock both summary and evidence under a single runtime-path lock to prevent
    // partial-overwrite from concurrent writers.
    let _lock = rt_storage::acquire_runtime_path_lock(paths.summary)?;
    if write_text_if_changed(paths.summary, payloads.summary_text)? {
        changed_paths.push(paths.summary.display().to_string());
    }
    if let Some(evidence) = payloads.evidence
        && write_json_if_changed(paths.evidence, evidence)? {
            changed_paths.push(paths.evidence.display().to_string());
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

fn resolve_session_task_id(payload: &Value, task: &str) -> String {
    let direct = safe_slug(&value_text(payload.get("task_id")));
    let candidate = if direct.is_empty() {
        build_task_id(task, None)
    } else {
        direct
    };
    if is_unsafe_task_id_slug(&candidate) {
        build_task_id(task, None)
    } else {
        candidate
    }
}

fn is_unsafe_task_id_slug(slug: &str) -> bool {
    slug.is_empty()
        || slug.contains("..")
        || slug.contains('/')
        || slug.contains('\\')
        || slug.starts_with('.')
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

fn build_evidence_index_payload(entries: &[Value]) -> Value {
    json!({
        "schema_version": EVIDENCE_INDEX_SCHEMA_VERSION,
        "artifacts": entries,
    })
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
        Value::String(framework_kernel::time::current_local_timestamp()),
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
    let evidence_rows = super::evidence::normalize_evidence_index(input.evidence_payload);
    let evidence_success_count = evidence_rows
        .iter()
        .filter(|row| {
            row.get("success").and_then(Value::as_bool) == Some(true)
                || row.get("exit_code").and_then(Value::as_i64) == Some(0)
        })
        .count();
    payload.insert(
        "evidence_count".to_string(),
        Value::from(evidence_rows.len()),
    );
    payload.insert(
        "evidence_count_successful".to_string(),
        Value::from(evidence_success_count),
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
    // matched_skills merged into supervisor_state.trace_metadata
    if let Some(skills) = input.matched_skills {
        let mut trace = serde_json::Map::new();
        trace.insert("matched_skills".to_string(), skills.clone());
        trace.insert(
            "updated_at".to_string(),
            Value::String(framework_kernel::time::now_iso()),
        );
        payload.insert("trace_metadata".to_string(), Value::Object(trace));
    }
    payload.insert(
        "artifact_refs".to_string(),
        json!({
            "task_root": input.artifact_dir.display().to_string(),
            "session_summary": input.artifact_dir.join(SESSION_SUMMARY_FILENAME).display().to_string(),
            "next_actions": input.artifact_dir.join(NEXT_ACTIONS_FILENAME).display().to_string(),
            "evidence_index": input.artifact_dir.join(EVIDENCE_INDEX_FILENAME).display().to_string(),
        }),
    );
    Value::Object(payload)
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
        Value::String(framework_kernel::time::current_local_timestamp()),
    );
    Value::Object(payload)
}

fn normalized_continuity(existing: Option<&Value>, status: &str) -> Value {
    let mut payload = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let terminal = crate::util::is_terminal(status, fr_utils::constants::TERMINAL_VERIFICATION_STATUSES)
        || crate::util::is_terminal(status, fr_utils::constants::TERMINAL_STORY_STATES);
    payload.insert(
        "story_state".to_string(),
        Value::String(if terminal { "completed" } else { "active" }.to_string()),
    );
    payload.insert("resume_allowed".to_string(), Value::Bool(!terminal));
    payload.insert(
        "last_updated_at".to_string(),
        Value::String(framework_kernel::time::current_local_timestamp()),
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

pub(super) use core_state_utils::json_io::{write_json_if_changed, write_text_if_changed};

pub(super) fn current_file_hash(path: &Path) -> Result<Option<String>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(trace_runtime::sha256_hex(&bytes))),
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
