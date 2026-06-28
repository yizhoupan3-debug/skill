//! Bootstrap, artifact migration, and file utilities.
//!
//! Extracted from projection.rs to keep file size ≤2000 lines.

use super::*;
use framework_kernel::json_value::{build_task_id, safe_slug};

pub fn default_home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn default_bootstrap_output_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("artifacts").join("bootstrap")
}

pub fn default_bootstrap_mirror_path(output_dir: &Path) -> PathBuf {
    output_dir.join("framework_default_bootstrap.json")
}

pub fn workspace_name_from_root(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

pub fn compact_evolution_proposals(payload: &Value) -> Value {
    json!({
        "proposal_count": payload.get("proposal_count").and_then(Value::as_u64).unwrap_or(0),
        "proposals": payload
            .get("proposals")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    })
}

pub fn build_default_bootstrap_payload(
    repo_root: &Path,
    output_dir: Option<&Path>,
    query: &str,
    artifact_source_dir: Option<&Path>,
    workspace_override: Option<&str>,
    _top: usize,
) -> Result<Value> {
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let resolved_output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_bootstrap_output_dir(&repo_root));
    fs::create_dir_all(&resolved_output_dir)?;
    let workspace = workspace_override
        .map(str::to_owned)
        .unwrap_or_else(|| workspace_name_from_root(&repo_root));
    let created_at = framework_kernel::time::current_local_timestamp();
    let task_id = build_task_id(
        if query.trim().is_empty() {
            &workspace
        } else {
            query
        },
        None,
    );
    let continuity_bootstrap =
        build_default_continuity_bootstrap(&repo_root, artifact_source_dir, Some(&task_id))?;
    let runtime = json!({
        "skills": [],
        "count": 0,
        "source": "skills/SKILL_ROUTING_RUNTIME.json",
    });
    let proposals = compact_evolution_proposals(&json!({
        "proposal_count": 0,
        "proposals": [],
    }));
    let payload = json!({
        "skills-export": runtime,
        "continuity-bootstrap": continuity_bootstrap,
        "evolution-proposals": proposals,
        "bootstrap": {
            "query": query,
            "workspace": workspace,
            "repo_root": repo_root.to_string_lossy(),
            "task_id": task_id,
            "created_at": created_at,
        }
    });
    let task_output_dir = resolved_output_dir.join(&task_id);
    fs::create_dir_all(&task_output_dir)?;
    let bootstrap_path = task_output_dir.join("framework_default_bootstrap.json");
    let mirror_bootstrap_path = default_bootstrap_mirror_path(&resolved_output_dir);
    write_json_if_changed(&bootstrap_path, &payload)?;
    write_json_if_changed(&mirror_bootstrap_path, &payload)?;
    Ok(json!({
        "bootstrap_path": bootstrap_path.to_string_lossy(),
        "paths": {
            "output_dir": resolved_output_dir.to_string_lossy(),
            "task_output_dir": task_output_dir.to_string_lossy(),
            "repo_root": repo_root.to_string_lossy(),
            "mirror_bootstrap_path": mirror_bootstrap_path.to_string_lossy(),
        },
        "proposal_count": payload
            .get("evolution-proposals")
            .and_then(Value::as_object)
            .and_then(|entry| entry.get("proposal_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "payload": payload,
    }))
}

pub fn build_default_continuity_bootstrap(
    repo_root: &Path,
    artifact_source_dir: Option<&Path>,
    task_id: Option<&str>,
) -> Result<Value> {
    let mut args = vec!["framework".to_string(), "snapshot".to_string()];
    if let Some(path) = artifact_source_dir {
        args.push("--artifact-source-dir".to_string());
        args.push(path.to_string_lossy().into_owned());
    }
    if let Some(task_id) = task_id {
        args.push("--task-id".to_string());
        args.push(task_id.to_string());
    }
    let snapshot = run_router_rs_json(repo_root, &args).map_err(FrameworkError::validation)?;
    Ok(json!({
        "schema_version": "framework-continuity-bootstrap-v1",
        "source": "framework-runtime-snapshot",
        "snapshot": snapshot.get("runtime_snapshot").cloned().unwrap_or_else(|| json!({})),
    }))
}

#[derive(Clone)]
pub struct MigrationPlan {
    source: String,
    destination: String,
}

pub fn migration_plan_values(plans: &[MigrationPlan]) -> Value {
    Value::Array(
        plans
            .iter()
            .map(|plan| {
                json!({
                    "source": plan.source,
                    "destination": plan.destination,
                })
            })
            .collect(),
    )
}

pub fn evidence_artifact_root(repo_root: &Path, task_id: Option<&str>) -> PathBuf {
    let root = repo_root.join("artifacts").join("evidence");
    task_id
        .map(|value| root.join(safe_slug(value)))
        .unwrap_or(root)
}

pub fn scratch_artifact_root(repo_root: &Path, run_id: Option<&str>) -> PathBuf {
    let root = repo_root.join("artifacts").join("scratch");
    run_id
        .map(|value| root.join(safe_slug(value)))
        .unwrap_or(root)
}

pub fn move_path(source: &Path, destination: &Path) -> Result<String> {
    let mut resolved_destination = destination.to_path_buf();
    if resolved_destination.exists() {
        let suffix = framework_kernel::time::current_local_timestamp()
            .replace(':', "")
            .replace('+', "_");
        let stem = resolved_destination
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("moved");
        let extension = resolved_destination
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default();
        resolved_destination =
            resolved_destination.with_file_name(format!("{stem}-{suffix}{extension}"));
    }
    if let Some(parent) = resolved_destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(source, &resolved_destination)?;
    Ok(resolved_destination.to_string_lossy().into_owned())
}

pub fn destination_for_current_artifact(
    repo_root: &Path,
    path: &Path,
    active_task_id: &str,
) -> Option<PathBuf> {
    let current_root = repo_root.join("artifacts").join("current");
    let task_root = current_root.join(active_task_id);
    if !path.exists()
        || (path.parent() != Some(current_root.as_path())
            && path.parent() != Some(task_root.as_path()))
    {
        return None;
    }
    if CURRENT_ALLOWED_ARTIFACT_NAMES.contains(&path.file_name()?.to_str()?)
        || path.file_name()?.to_str()? == active_task_id
    {
        return None;
    }
    if path.parent() == Some(task_root.as_path())
        && TASK_ALLOWED_ARTIFACT_NAMES.contains(&path.file_name()?.to_str()?)
    {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    let suffix = || {
        if path.parent() == Some(current_root.as_path()) {
            PathBuf::from(name)
        } else {
            PathBuf::from(active_task_id).join(name)
        }
    };
    if name == "framework_default_bootstrap.json" || name == "hermes_default_bootstrap.json" {
        return Some(
            repo_root
                .join("artifacts")
                .join("bootstrap")
                .join("archived-current")
                .join(suffix()),
        );
    }
    if name == "run_summary.json"
        || name == "storage_audit.json"
        || name == "snapshot.json"
        || name == "snapshot.md"
    {
        return Some(
            repo_root
                .join("artifacts")
                .join("ops")
                .join("archived-current")
                .join(suffix()),
        );
    }
    if name.starts_with("tmp-") {
        return Some(if path.parent() == Some(current_root.as_path()) {
            scratch_artifact_root(repo_root, None).join(name)
        } else {
            scratch_artifact_root(repo_root, Some("archived-current"))
                .join(active_task_id)
                .join(name)
        });
    }
    Some(evidence_artifact_root(repo_root, Some("archived-current")).join(suffix()))
}

pub fn plan_current_artifact_clutter_migrations(
    repo_root: &Path,
    active_task_id: &str,
) -> Result<Vec<MigrationPlan>> {
    let current_root = repo_root.join("artifacts").join("current");
    if !current_root.exists() {
        return Ok(Vec::new());
    }
    let mut plans = Vec::new();
    for entry in fs::read_dir(&current_root)? {
        let path = entry?.path();
        if let Some(destination) =
            destination_for_current_artifact(repo_root, &path, active_task_id)
        {
            plans.push(MigrationPlan {
                source: path.to_string_lossy().into_owned(),
                destination: destination.to_string_lossy().into_owned(),
            });
        }
    }
    let task_root = current_root.join(active_task_id);
    if task_root.is_dir() {
        for entry in fs::read_dir(&task_root)? {
            let path = entry?.path();
            if let Some(destination) =
                destination_for_current_artifact(repo_root, &path, active_task_id)
            {
                plans.push(MigrationPlan {
                    source: path.to_string_lossy().into_owned(),
                    destination: destination.to_string_lossy().into_owned(),
                });
            }
        }
    }
    plans.sort_by(|left, right| left.source.cmp(&right.source));
    Ok(plans)
}

pub fn migrate_current_artifact_clutter(
    repo_root: &Path,
    active_task_id: &str,
) -> Result<Vec<String>> {
    let plans = plan_current_artifact_clutter_migrations(repo_root, active_task_id)?;
    let mut moved = Vec::new();
    for plan in plans {
        moved.push(move_path(
            Path::new(&plan.source),
            Path::new(&plan.destination),
        )?);
    }
    Ok(moved)
}

pub fn bootstrap_payload_matches_contract(payload: &Value, repo_root: &Path) -> bool {
    let normalized_repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf())
        .to_string_lossy()
        .to_string();
    payload
        .get("bootstrap")
        .and_then(Value::as_object)
        .zip(
            payload
                .get("continuity-bootstrap")
                .and_then(Value::as_object),
        )
        .zip(payload.get("skills-export").and_then(Value::as_object))
        .zip(
            payload
                .get("evolution-proposals")
                .and_then(Value::as_object),
        )
        .map(|(((bootstrap, _continuity), skills), _proposals)| {
            bootstrap
                .get("repo_root")
                .and_then(Value::as_str)
                .map(|value| value == normalized_repo_root)
                .unwrap_or(false)
                && skills.get("source").and_then(Value::as_str)
                    == Some("skills/SKILL_ROUTING_RUNTIME.json")
        })
        .unwrap_or(false)
}

pub fn ensure_default_bootstrap(repo_root: &Path, output_dir: Option<&Path>) -> Result<Value> {
    let resolved_output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_bootstrap_output_dir(repo_root));
    fs::create_dir_all(&resolved_output_dir)?;
    let mirror_bootstrap_path = default_bootstrap_mirror_path(&resolved_output_dir);
    let had_existing_file = mirror_bootstrap_path.exists();
    let existing_payload = read_text_if_exists(&mirror_bootstrap_path)?
        .and_then(|content| serde_json::from_str::<Value>(&content).ok());
    if mirror_bootstrap_path.is_file()
        && existing_payload
            .as_ref()
            .is_some_and(|payload| bootstrap_payload_matches_contract(payload, repo_root))
    {
        return Ok(json!({
            "success": true,
            "changed": false,
            "status": "already-present",
            "output_dir": resolved_output_dir.to_string_lossy(),
            "bootstrap_path": mirror_bootstrap_path.to_string_lossy(),
            "mirror_bootstrap_path": mirror_bootstrap_path.to_string_lossy(),
        }));
    }

    let parsed =
        build_default_bootstrap_payload(repo_root, Some(&resolved_output_dir), "", None, None, 8)?;
    let output_dir_value = parsed
        .get("paths")
        .and_then(|value| value.get("output_dir"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| resolved_output_dir.to_string_lossy().into_owned());
    let mirror_bootstrap_value = parsed
        .get("paths")
        .and_then(|value| value.get("mirror_bootstrap_path"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| mirror_bootstrap_path.to_string_lossy().into_owned());
    let task_output_dir_value = parsed
        .get("paths")
        .and_then(|value| value.get("task_output_dir"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let bootstrap_path_value = parsed
        .get("bootstrap_path")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let task_id_value = parsed
        .get("payload")
        .and_then(|value| value.get("bootstrap"))
        .and_then(|value| value.get("task_id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(json!({
        "success": true,
        "changed": true,
        "status": if had_existing_file { "repaired-stale" } else { "materialized" },
        "output_dir": output_dir_value,
        "task_output_dir": task_output_dir_value,
        "bootstrap_path": bootstrap_path_value,
        "mirror_bootstrap_path": mirror_bootstrap_value,
        "task_id": task_id_value,
        "proposal_count": parsed.get("proposal_count").and_then(Value::as_u64),
    }))
}

pub fn validate_default_bootstrap(bootstrap_path: &Path, repo_root: &Path) -> Result<bool> {
    let path = normalize_path(bootstrap_path)?;
    let repo_root = normalize_path(repo_root)?;
    let Some(content) = read_text_if_exists(&path)? else {
        return Ok(false);
    };
    let payload = serde_json::from_str::<Value>(&content)?;
    Ok(bootstrap_payload_matches_contract(&payload, &repo_root))
}

pub fn ensure_config_file(config_path: &Path) -> Result<bool> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if config_path.exists() {
        return Ok(false);
    }
    let header = framework_kernel::runtime_registry::host_config_schema_url("codex");
    let header = if header.is_empty() {
        String::new()
    } else {
        format!("#:schema {header}\n")
    };
    fs::write(config_path, &header)?;
    Ok(true)
}

pub fn ensure_hooks_feature_disabled(config_path: &Path) -> Result<bool> {
    const HOOKS_DISABLED_LINE: &str = "hooks = false";
    let content = read_text_if_exists(config_path)?.unwrap_or_default();
    if let Some((start, end)) = find_named_block_bounds(&content, "[features]") {
        let block = content[start..end].trim_end_matches('\n');
        let mut has_hooks = false;
        let mut hooks_already_disabled = false;
        let mut updated_lines = Vec::new();
        for line in block.lines() {
            if is_named_setting(line, "hooks") {
                hooks_already_disabled |= line.trim() == HOOKS_DISABLED_LINE;
                if !has_hooks {
                    updated_lines.push(HOOKS_DISABLED_LINE.to_string());
                    has_hooks = true;
                }
            } else {
                updated_lines.push(line.to_string());
            }
        }
        if has_hooks && hooks_already_disabled {
            return Ok(false);
        }
        if !has_hooks {
            updated_lines.push(HOOKS_DISABLED_LINE.to_string());
        }
        let new_block = format!("{}\n", updated_lines.join("\n"));
        let updated = format!("{}{}{}", &content[..start], new_block, &content[end..]);
        write_text_if_changed(config_path, &updated)
    } else {
        let mut updated = content.trim_end().to_string();
        if !updated.is_empty() {
            updated.push_str("\n\n");
        }
        updated.push_str("[features]\n");
        updated.push_str(HOOKS_DISABLED_LINE);
        updated.push('\n');
        write_text_if_changed(config_path, &updated)
    }
}

pub fn find_named_block_bounds(content: &str, marker: &str) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    let mut start: Option<usize> = None;
    for line in content.split_inclusive('\n') {
        let normalized = line.trim_end_matches('\n');
        if start.is_none() {
            if normalized == marker {
                start = Some(offset);
            }
        } else if normalized.starts_with('[') {
            return Some((start.unwrap_or(0), offset));
        }
        offset += line.len();
    }
    start.map(|value| (value, content.len()))
}

pub fn ensure_tui_status_line(config_path: &Path) -> Result<bool> {
    let content = read_text_if_exists(config_path)?.unwrap_or_default();
    let status_line = format_status_line();
    if let Some((start, end)) = find_tui_block_bounds(&content) {
        let block = content[start..end].trim_end_matches('\n');
        let mut replaced = false;
        let mut updated_lines = Vec::new();
        for line in block.lines() {
            if is_status_line(line) {
                updated_lines.push(status_line.clone());
                replaced = true;
            } else {
                updated_lines.push(line.to_string());
            }
        }
        if !replaced {
            updated_lines.push(status_line);
        }
        let new_block = format!("{}\n", updated_lines.join("\n"));
        let updated = format!("{}{}{}", &content[..start], new_block, &content[end..]);
        return write_text_if_changed(config_path, &updated);
    }

    let mut updated = content.trim_end().to_string();
    if !updated.is_empty() {
        updated.push_str("\n\n");
    }
    updated.push_str("[tui]\n");
    updated.push_str(&format_status_line());
    updated.push('\n');
    write_text_if_changed(config_path, &updated)
}

pub fn find_tui_block_bounds(content: &str) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    let mut start: Option<usize> = None;
    for line in content.split_inclusive('\n') {
        let normalized = line.trim_end_matches('\n');
        if start.is_none() {
            if normalized == "[tui]" {
                start = Some(offset);
            }
        } else if normalized.starts_with('[') {
            return Some((start.unwrap_or(0), offset));
        }
        offset += line.len();
    }
    start.map(|value| (value, content.len()))
}

pub fn is_status_line(line: &str) -> bool {
    is_named_setting(line, "status_line")
}

pub fn is_named_setting(line: &str, key: &str) -> bool {
    line.split_once('=')
        .map(|(name, _)| name.trim() == key)
        .unwrap_or(false)
}

pub fn format_status_line() -> String {
    let items = DEFAULT_TUI_STATUS_ITEMS
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("status_line = [{items}]")
}

pub fn read_text_if_exists(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub use core_state_utils::json_io::{write_json_if_changed, write_text_if_changed};

/// Read JSON from file if it exists. Returns `Ok(None)` when file is absent.
pub fn read_json_if_exists(path: &Path) -> Result<Option<Value>> {
    Ok(Some(core_state_utils::json_io::read_json_if_exists(path)))
}
