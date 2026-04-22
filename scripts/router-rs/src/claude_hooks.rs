use chrono::Local;
use regex::Regex;
use rusqlite::{params, types::ValueRef, Connection};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::framework_runtime::{
    build_framework_recap_projection, build_framework_runtime_snapshot_envelope,
};

const CLAUDE_HOOK_SCHEMA_VERSION: &str = "router-rs-claude-hook-response-v1";
const CLAUDE_HOOK_AUDIT_SCHEMA_VERSION: &str = "router-rs-claude-hook-audit-response-v1";
const CLAUDE_HOOK_AUTHORITY: &str = "rust-claude-hook";
const CLAUDE_HOOK_AUDIT_AUTHORITY: &str = "rust-claude-hook-audit";
const MEMORY_STATE_SCHEMA_VERSION: &str = "memory-state-v1";
const MEMORY_STORE_SCHEMA_VERSION: &str = "1";
const CLAUDE_MEMORY_PATH: &str = ".codex/memory/CLAUDE_MEMORY.md";
const MEMORY_AUTO_FILENAME: &str = "MEMORY_AUTO.md";
const MEMORY_DB_FILENAME: &str = "memory.sqlite3";
const MEMORY_STATE_FILENAME: &str = "state.json";
const SQLITE_DUMP_FILENAME: &str = "sqlite_legacy_dump.json";
const STABLE_DOCUMENTS: [&str; 5] = [
    "MEMORY.md",
    "preferences.md",
    "decisions.md",
    "lessons.md",
    "runbooks.md",
];
const GENERATED_PATHS: [&str; 9] = [
    ".claude/settings.json",
    ".claude/hooks/README.md",
    ".claude/hooks/session_start.sh",
    ".claude/hooks/stop.sh",
    ".claude/hooks/pre_compact.sh",
    ".claude/hooks/subagent_stop.sh",
    ".claude/hooks/session_end.sh",
    ".claude/hooks/config_change.sh",
    ".claude/hooks/stop_failure.sh",
];
const SHARED_CONTINUITY_PATHS: [&str; 5] = [
    "SESSION_SUMMARY.md",
    "NEXT_ACTIONS.json",
    "EVIDENCE_INDEX.json",
    "TRACE_METADATA.json",
    ".supervisor_state.json",
];
const TERMINAL_STORY_STATES: [&str; 6] = [
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
];
const TERMINAL_PHASES: [&str; 7] = [
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
    "done",
];
const TERMINAL_VERIFICATION_STATUSES: [&str; 6] = [
    "completed",
    "passed",
    "verified",
    "cancelled",
    "abandoned",
    "failed",
];

pub fn run_claude_lifecycle_hook(
    command: &str,
    repo_root: &Path,
    max_lines: usize,
) -> Result<Value, String> {
    let canonical = canonical_lifecycle_command(command)?;
    let contract = lifecycle_contract(canonical);
    let mut response = json!({
        "schema_version": CLAUDE_HOOK_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUTHORITY,
        "wrapper_command": command,
        "canonical_command": canonical,
        "command": canonical,
        "repo_root": repo_root.display().to_string(),
        "contract": contract,
    });

    if contract
        .get("consolidates_shared_memory")
        .and_then(Value::as_bool)
        == Some(true)
    {
        response["consolidation"] = consolidate_shared_memory(repo_root)?;
    }

    response["projection"] = sync_claude_memory_projection(repo_root, max_lines)?;
    Ok(response)
}

pub fn run_claude_audit_hook(command: &str, repo_root: &Path) -> Result<Value, String> {
    let canonical = canonical_audit_command(command)?;
    let payload = read_stdin_payload()?;
    match canonical {
        "config-change" => run_config_change(repo_root, &payload),
        "stop-failure" => run_stop_failure(repo_root, &payload),
        _ => Err(format!("Unsupported Claude audit command: {command}")),
    }
}

fn canonical_lifecycle_command(command: &str) -> Result<&'static str, String> {
    match command {
        "refresh-projection" | "sync" => Ok("refresh-projection"),
        "session-start" | "start-session" => Ok("session-start"),
        "session-stop" | "stop-session" => Ok("session-stop"),
        "pre-compact" => Ok("pre-compact"),
        "subagent-stop" => Ok("subagent-stop"),
        "session-end" | "end-session" => Ok("session-end"),
        _ => Err(format!("Unsupported Claude lifecycle command: {command}")),
    }
}

fn canonical_audit_command(command: &str) -> Result<&'static str, String> {
    match command {
        "config-change" => Ok("config-change"),
        "stop-failure" => Ok("stop-failure"),
        _ => Err(format!("Unsupported Claude audit command: {command}")),
    }
}

fn lifecycle_contract(command: &str) -> Value {
    match command {
        "refresh-projection" => json!({
            "writes": ["project-local Claude memory projection"],
            "forbidden_writes": SHARED_CONTINUITY_PATHS,
            "consolidates_shared_memory": false,
            "summary": "Refresh the imported Claude projection without touching shared continuity artifacts."
        }),
        "session-start" => json!({
            "writes": ["project-local Claude memory projection"],
            "forbidden_writes": SHARED_CONTINUITY_PATHS,
            "consolidates_shared_memory": false,
            "summary": "Refresh the imported Claude projection at session start."
        }),
        "session-stop" => json!({
            "writes": ["project-local Claude memory projection"],
            "forbidden_writes": SHARED_CONTINUITY_PATHS,
            "consolidates_shared_memory": false,
            "summary": "Perform a lightweight post-turn projection refresh only."
        }),
        "pre-compact" => json!({
            "writes": ["project-local Claude memory projection"],
            "forbidden_writes": SHARED_CONTINUITY_PATHS,
            "consolidates_shared_memory": false,
            "summary": "Refresh the projection before compaction without running consolidation."
        }),
        "subagent-stop" => json!({
            "writes": ["project-local Claude memory projection"],
            "forbidden_writes": SHARED_CONTINUITY_PATHS,
            "consolidates_shared_memory": false,
            "summary": "Refresh the projection after subagent completion without taking over subagent orchestration."
        }),
        "session-end" => json!({
            "writes": [
                "project-local shared memory bundle",
                "project-local Claude memory projection",
                "terminal-session continuity repair when resume_allowed is stale"
            ],
            "forbidden_writes": [
                "SESSION_SUMMARY.md",
                "NEXT_ACTIONS.json",
                "EVIDENCE_INDEX.json",
                "TRACE_METADATA.json"
            ],
            "conditional_writes": [".supervisor_state.json"],
            "consolidates_shared_memory": true,
            "summary": "Consolidate the project-local memory bundle, refresh the imported Claude projection, and only repair terminal resume state when needed."
        }),
        _ => Value::Null,
    }
}

fn sync_claude_memory_projection(repo_root: &Path, max_lines: usize) -> Result<Value, String> {
    let target = repo_root.join(CLAUDE_MEMORY_PATH);
    let content = build_claude_memory_projection(repo_root, max_lines)?;
    let changed = write_text_if_changed(&target, &content)?;
    Ok(json!({
        "status": if changed { "updated" } else { "unchanged" },
        "target_path": target.display().to_string(),
        "changed": changed,
    }))
}

fn build_claude_memory_projection(repo_root: &Path, max_lines: usize) -> Result<String, String> {
    build_framework_recap_projection(repo_root, max_lines)
}

fn consolidate_shared_memory(repo_root: &Path) -> Result<Value, String> {
    repair_terminal_resume_allowed(repo_root)?;
    let runtime_snapshot = build_framework_runtime_snapshot_envelope(repo_root)?
        .get("runtime_snapshot")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| "framework runtime snapshot is missing runtime_snapshot".to_string())?;
    let workspace = repo_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workspace")
        .to_string();
    let resolved_dir = repo_root.join(".codex/memory");
    fs::create_dir_all(&resolved_dir)
        .map_err(|err| format!("create memory directory failed: {err}"))?;
    let archive = archive_legacy_memory_bundle(&workspace, &resolved_dir)?;
    let documents = load_stable_documents(repo_root, &resolved_dir);
    let changed_files = write_documents(&documents, &resolved_dir)?;
    let mut changed_files_with_state = changed_files;
    if let Some(state_path) = write_memory_state(&runtime_snapshot, &resolved_dir)? {
        changed_files_with_state.push(state_path);
    }
    let sqlite_result = persist_memory_bundle(&workspace, &documents, &resolved_dir)?;
    Ok(json!({
        "memory_root": resolved_dir.display().to_string(),
        "changed_files": changed_files_with_state,
        "archive": archive,
        "sqlite_result": sqlite_result,
    }))
}

fn repair_terminal_resume_allowed(repo_root: &Path) -> Result<(), String> {
    let state_path = repo_root.join(".supervisor_state.json");
    let mut supervisor_state = read_json_if_exists(&state_path);
    let needs_repair = supervisor_state
        .as_object()
        .and_then(|state| {
            state
                .get("continuity")
                .and_then(Value::as_object)
                .map(|continuity| (state, continuity))
        })
        .map(|(state, continuity)| {
            continuity.get("resume_allowed").and_then(Value::as_bool) == Some(true)
                && (is_terminal_token(state.get("active_phase"), &TERMINAL_PHASES)
                    || is_terminal_token(
                        state
                            .get("verification")
                            .and_then(Value::as_object)
                            .and_then(|verification| verification.get("verification_status")),
                        &TERMINAL_VERIFICATION_STATUSES,
                    )
                    || is_terminal_token(continuity.get("story_state"), &TERMINAL_STORY_STATES))
        })
        .unwrap_or(false);
    if !needs_repair {
        return Ok(());
    }
    if let Some(state) = supervisor_state.as_object_mut() {
        if let Some(continuity) = state.get_mut("continuity").and_then(Value::as_object_mut) {
            continuity.insert("resume_allowed".to_string(), Value::Bool(false));
        }
    }
    write_json_if_changed(&state_path, &supervisor_state)?;
    Ok(())
}

fn load_stable_documents(repo_root: &Path, resolved_dir: &Path) -> Vec<(String, String)> {
    vec![
        (
            "MEMORY.md".to_string(),
            read_text_if_exists(&resolved_dir.join("MEMORY.md"))
                .if_empty_then(default_memory_md(repo_root)),
        ),
        (
            "preferences.md".to_string(),
            read_text_if_exists(&resolved_dir.join("preferences.md"))
                .if_empty_then("# preferences\n".to_string()),
        ),
        (
            "decisions.md".to_string(),
            read_text_if_exists(&resolved_dir.join("decisions.md"))
                .if_empty_then("# decisions\n".to_string()),
        ),
        (
            "lessons.md".to_string(),
            read_text_if_exists(&resolved_dir.join("lessons.md"))
                .if_empty_then("# lessons\n".to_string()),
        ),
        (
            "runbooks.md".to_string(),
            read_text_if_exists(&resolved_dir.join("runbooks.md"))
                .if_empty_then(default_runbooks()),
        ),
    ]
}

fn default_memory_md(repo_root: &Path) -> String {
    format!(
        "# 项目长期记忆\n\n_本文件沉淀跨会话稳定的项目事实、决策与约定。当前任务态以 continuity artifacts 为准；历史/debug 归档到 `memory/archive/`。_\n\n## 项目身份\n\n- **仓库**: `{}`\n- **闭环事实源**: `artifacts/current/<task_id>/` + `artifacts/current/active_task.json` + `.supervisor_state.json`\n- **默认召回策略**: 稳定层优先，仅在 query 明确命中 active task 且 freshness gate 通过时追加当前任务态\n- **Artifact 分层**: `artifacts/bootstrap/` / `artifacts/ops/memory_automation/` / `artifacts/evidence/` / `artifacts/scratch/`\n",
        repo_root.display()
    )
}

fn default_runbooks() -> String {
    "# runbooks\n\n## 标准操作\n\n- 统一维护入口：python3 scripts/run_memory_automation.py --workspace <workspace>\n- 需要迁移旧 artifact 布局时显式执行：python3 scripts/run_memory_automation.py --workspace <workspace> --apply-artifact-migrations\n- 合并稳定记忆：python3 scripts/consolidate_memory.py --workspace <workspace>\n- 召回上下文：python3 scripts/retrieve_memory.py --workspace <workspace> --mode stable|active|history|debug --topic <关键词>\n- 生命周期收口：python3 scripts/router_rs_runner.py --claude-hook-command session-end --repo-root <repo_root> --claude-hook-max-lines 4\n- 诊断快照与存储审计查看 `artifacts/ops/memory_automation/<run_id>/`，不再从 MEMORY_AUTO 或 sessions 读取。\n"
        .to_string()
}

fn archive_legacy_memory_bundle(workspace: &str, resolved_dir: &Path) -> Result<Value, String> {
    let archive_root = resolved_dir
        .join("archive")
        .join(format!("pre-cutover-{}", current_local_date()));
    let mut archived_paths = Vec::new();

    let legacy_path = resolved_dir.join(MEMORY_AUTO_FILENAME);
    if legacy_path.exists() {
        archived_paths.push(move_to_archive(
            &legacy_path,
            &archive_root.join(MEMORY_AUTO_FILENAME),
        )?);
    }
    let sessions_dir = resolved_dir.join("sessions");
    if sessions_dir.exists() {
        archived_paths.push(move_to_archive(
            &sessions_dir,
            &archive_root.join("sessions"),
        )?);
    }

    let db_path = resolved_dir.join(MEMORY_DB_FILENAME);
    let conn = open_memory_store(&db_path)?;
    let legacy_rows = export_rows(
        &conn,
        "SELECT * FROM session_notes WHERE workspace = ? ORDER BY updated_at DESC, session_key DESC, position ASC",
        &[workspace],
    )?;
    let evidence_rows = export_rows(
        &conn,
        "SELECT * FROM evidence_records WHERE workspace = ? ORDER BY updated_at DESC",
        &[workspace],
    )?;
    let memory_items = export_rows(
        &conn,
        "SELECT * FROM memory_items WHERE workspace = ? AND source NOT IN (?, ?, ?, ?, ?) ORDER BY updated_at DESC",
        &[
            workspace,
            STABLE_DOCUMENTS[0],
            STABLE_DOCUMENTS[1],
            STABLE_DOCUMENTS[2],
            STABLE_DOCUMENTS[3],
            STABLE_DOCUMENTS[4],
        ],
    )?;
    let legacy_row_count = legacy_rows.len() + evidence_rows.len();
    let legacy_memory_item_count = memory_items.len();
    if legacy_row_count > 0 || legacy_memory_item_count > 0 {
        fs::create_dir_all(&archive_root)
            .map_err(|err| format!("create archive directory failed: {err}"))?;
        let dump_path = archive_root.join(SQLITE_DUMP_FILENAME);
        let dump_payload = json!({
            "schema_version": "memory-legacy-dump-v1",
            "exported_at": current_local_timestamp(),
            "workspace": workspace,
            "memory_items": memory_items,
            "session_notes": legacy_rows,
            "evidence_records": evidence_rows,
        });
        write_json_if_changed(&dump_path, &dump_payload)?;
        archived_paths.push(dump_path.display().to_string());
        conn.execute(
            "DELETE FROM session_notes WHERE workspace = ?",
            params![workspace],
        )
        .map_err(|err| format!("delete legacy session notes failed: {err}"))?;
        conn.execute(
            "DELETE FROM evidence_records WHERE workspace = ?",
            params![workspace],
        )
        .map_err(|err| format!("delete legacy evidence failed: {err}"))?;
        conn.execute(
            "DELETE FROM memory_items WHERE workspace = ? AND source NOT IN (?, ?, ?, ?, ?)",
            params![
                workspace,
                STABLE_DOCUMENTS[0],
                STABLE_DOCUMENTS[1],
                STABLE_DOCUMENTS[2],
                STABLE_DOCUMENTS[3],
                STABLE_DOCUMENTS[4]
            ],
        )
        .map_err(|err| format!("delete non-authoritative memory items failed: {err}"))?;
    }

    Ok(json!({
        "archive_root": archive_root.display().to_string(),
        "archived_paths": archived_paths,
        "legacy_row_count": legacy_row_count,
        "legacy_memory_item_count": legacy_memory_item_count,
    }))
}

fn persist_memory_bundle(
    workspace: &str,
    documents: &[(String, String)],
    resolved_dir: &Path,
) -> Result<Value, String> {
    let db_path = resolved_dir.join(MEMORY_DB_FILENAME);
    let conn = open_memory_store(&db_path)?;
    let sources = documents
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    delete_memory_items_not_in_sources(&conn, workspace, &sources)?;
    delete_memory_items_by_sources(&conn, workspace, &sources)?;

    let mut persisted_items = 0usize;
    for (file_name, text) in documents {
        let category = memory_category_for_file(file_name);
        let segments = extract_memory_segments(text);
        for (index, (headings, summary)) in segments.iter().enumerate() {
            let heading_context = headings.join(" / ");
            let item_id = memory_item_id(workspace, category, index + 1, summary, file_name);
            let metadata = json!({
                "document": file_name,
                "headings": headings,
            });
            let keywords = json!([summary, file_name, headings]).to_string();
            let now = current_local_timestamp();
            conn.execute(
                "INSERT INTO memory_items (item_id, workspace, category, source, confidence, status, summary, notes, evidence_json, metadata_json, keywords_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(item_id) DO UPDATE SET workspace=excluded.workspace, category=excluded.category, source=excluded.source, confidence=excluded.confidence, status=excluded.status, summary=excluded.summary, notes=excluded.notes, evidence_json=excluded.evidence_json, metadata_json=excluded.metadata_json, keywords_json=excluded.keywords_json, updated_at=excluded.updated_at",
                params![
                    item_id,
                    workspace,
                    category,
                    file_name,
                    0.8f64,
                    "active",
                    summary,
                    heading_context,
                    "[]",
                    metadata.to_string(),
                    keywords,
                    now,
                    current_local_timestamp(),
                ],
            )
            .map_err(|err| format!("upsert memory item failed: {err}"))?;
            persisted_items += 1;
        }
    }

    let memory_items_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memory_items WHERE workspace = ?",
            params![workspace],
            |row| row.get(0),
        )
        .map_err(|err| format!("count memory items failed: {err}"))?;

    Ok(json!({
        "db_path": db_path.display().to_string(),
        "memory_items": memory_items_count,
        "persisted_items": persisted_items,
        "legacy_tables_authoritative": false,
    }))
}

fn write_documents(
    documents: &[(String, String)],
    resolved_dir: &Path,
) -> Result<Vec<String>, String> {
    fs::create_dir_all(resolved_dir)
        .map_err(|err| format!("create memory directory failed: {err}"))?;
    let mut changed_files = Vec::new();
    for (file_name, text) in documents {
        let path = resolved_dir.join(file_name);
        if write_text_if_changed(&path, text)? {
            changed_files.push(path.canonicalize().unwrap_or(path).display().to_string());
        }
    }
    Ok(changed_files)
}

fn write_memory_state(
    runtime_snapshot: &Map<String, Value>,
    resolved_dir: &Path,
) -> Result<Option<String>, String> {
    let path = resolved_dir.join(MEMORY_STATE_FILENAME);
    let payload = build_memory_state(runtime_snapshot)?;
    if write_json_if_changed(&path, &payload)? {
        let resolved = path.canonicalize().unwrap_or(path);
        return Ok(Some(resolved.display().to_string()));
    }
    Ok(None)
}

fn build_memory_state(runtime_snapshot: &Map<String, Value>) -> Result<Value, String> {
    let continuity = runtime_snapshot
        .get("continuity")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let paths = runtime_snapshot
        .get("paths")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let source_updated_at = continuity
        .get("continuity")
        .and_then(Value::as_object)
        .and_then(|inner| inner.get("last_updated_at"))
        .and_then(Value::as_str)
        .or_else(|| runtime_snapshot.get("collected_at").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let source_hash = build_runtime_source_hash(&paths, runtime_snapshot.get("active_task_id"))?;
    Ok(json!({
        "schema_version": MEMORY_STATE_SCHEMA_VERSION,
        "source_task_id": runtime_snapshot.get("active_task_id").cloned().unwrap_or(Value::Null),
        "source_task": continuity.get("task").cloned().unwrap_or(Value::Null),
        "source_phase": continuity.get("phase").cloned().unwrap_or(Value::Null),
        "source_status": continuity.get("status").cloned().unwrap_or(Value::Null),
        "continuity_state": continuity.get("state").cloned().unwrap_or(Value::Null),
        "artifact_root": runtime_snapshot.get("current_root").cloned().unwrap_or(Value::Null),
        "source_updated_at": source_updated_at,
        "content_hash": source_hash,
        "last_consolidated_at": current_local_timestamp(),
    }))
}

fn build_runtime_source_hash(
    paths: &Map<String, Value>,
    active_task_id: Option<&Value>,
) -> Result<String, String> {
    let payload = json!({
        "active_task_id": active_task_id.cloned().unwrap_or(Value::Null),
        "session_summary_text": read_text_if_exists(Path::new(&value_text(paths.get("session_summary")))),
        "next_actions": read_json_if_exists(Path::new(&value_text(paths.get("next_actions")))),
        "evidence_index": read_json_if_exists(Path::new(&value_text(paths.get("evidence_index")))),
        "trace_metadata": read_json_if_exists(Path::new(&value_text(paths.get("trace_metadata")))),
        "supervisor_state": read_json_if_exists(Path::new(&value_text(paths.get("supervisor_state")))),
    });
    let encoded =
        serde_json::to_vec(&payload).map_err(|err| format!("encode hash payload failed: {err}"))?;
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    Ok(format!("{:x}", hasher.finalize()))
}

fn run_config_change(repo_root: &Path, payload: &Value) -> Result<Value, String> {
    let scope = payload
        .get("source")
        .or_else(|| payload.get("scope"))
        .or_else(|| payload.get("matcher"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut rel_paths = HashSet::new();
    for path in iter_candidate_paths(payload) {
        rel_paths.insert(relative_candidate_path(&path, repo_root));
    }
    let mentions_continuity = payload_mentions_continuity(payload);
    let mut notices = Vec::new();
    if mentions_continuity {
        let message = "[claude-config-change] payload referenced shared continuity artifacts; leaving them untouched and keeping audit host-private.";
        eprintln!("{message}");
        notices.push(message.to_string());
    }
    if scope == "project_settings" {
        let hits = rel_paths
            .iter()
            .filter(|path| GENERATED_PATHS.contains(&path.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if hits.is_empty() {
            let message =
                "[claude-config-change] project settings changed outside generated Claude host surfaces; no action taken.";
            eprintln!("{message}");
            notices.push(message.to_string());
        } else {
            let message = format!(
                "[claude-config-change] detected edits on generated Claude host surfaces: {}; regenerate via scripts/materialize_cli_host_entrypoints.py instead of hand-editing outputs.",
                hits.join(", ")
            );
            eprintln!("{message}");
            notices.push(message);
        }
    }
    Ok(json!({
        "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
        "command": "config-change",
        "repo_root": repo_root.display().to_string(),
        "scope": scope,
        "notices": notices,
    }))
}

fn run_stop_failure(_repo_root: &Path, payload: &Value) -> Result<Value, String> {
    let failure_type = payload
        .get("error")
        .or_else(|| payload.get("failure_type"))
        .or_else(|| payload.get("matcher"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let continuity_note = if payload_mentions_continuity(payload) {
        " Shared continuity remains untouched."
    } else {
        ""
    };
    let message = format!(
        "[claude-stop-failure] Claude stop failure classified as {failure_type}; inspect /hooks, generated host files, and host-private projection drift before retrying.{continuity_note}"
    );
    eprintln!("{message}");
    Ok(json!({
        "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
        "command": "stop-failure",
        "failure_type": failure_type,
        "message": message,
    }))
}

fn read_stdin_payload() -> Result<Value, String> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|err| format!("read stdin payload failed: {err}"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(trimmed).or_else(|_| Ok(json!({ "raw": trimmed })))
}

fn iter_candidate_paths(payload: &Value) -> Vec<String> {
    let mut candidates = Vec::new();
    for key in [
        "file_path",
        "changed_path",
        "path",
        "config_path",
        "target_path",
    ] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            let normalized = text.replace('\\', "/");
            if !normalized.is_empty() {
                candidates.push(normalized);
            }
        }
    }
    if let Some(items) = payload.get("changed_files").and_then(Value::as_array) {
        for item in items {
            if let Some(text) = item.as_str() {
                let normalized = text.replace('\\', "/");
                if !normalized.is_empty() {
                    candidates.push(normalized);
                }
            }
        }
    }
    candidates
}

fn relative_candidate_path(path: &str, repo_root: &Path) -> String {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        if let Ok(rel) = candidate
            .canonicalize()
            .unwrap_or(candidate.clone())
            .strip_prefix(
                repo_root
                    .canonicalize()
                    .unwrap_or_else(|_| repo_root.to_path_buf()),
            )
        {
            return rel.to_string_lossy().replace('\\', "/");
        }
    }
    path.replace('\\', "/")
}

fn payload_mentions_continuity(payload: &Value) -> bool {
    let serialized = serde_json::to_string(payload).unwrap_or_default();
    SHARED_CONTINUITY_PATHS
        .iter()
        .any(|needle| serialized.contains(needle))
}

fn open_memory_store(db_path: &Path) -> Result<Connection, String> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create sqlite parent failed: {err}"))?;
    }
    let conn =
        Connection::open(db_path).map_err(|err| format!("open sqlite store failed: {err}"))?;
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        CREATE TABLE IF NOT EXISTS schema_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS memory_items (
            item_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            category TEXT NOT NULL,
            source TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.5,
            status TEXT NOT NULL DEFAULT 'active',
            summary TEXT NOT NULL,
            notes TEXT NOT NULL DEFAULT '',
            evidence_json TEXT NOT NULL DEFAULT '[]',
            metadata_json TEXT NOT NULL DEFAULT '{}',
            keywords_json TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_memory_items_workspace_updated
        ON memory_items(workspace, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_memory_items_workspace_category_status
        ON memory_items(workspace, category, status, updated_at DESC);
        CREATE TABLE IF NOT EXISTS session_notes (
            note_id INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace TEXT NOT NULL,
            session_key TEXT NOT NULL,
            position INTEGER NOT NULL,
            note TEXT NOT NULL,
            note_type TEXT NOT NULL DEFAULT 'append',
            metadata_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE (workspace, session_key, position)
        );
        CREATE INDEX IF NOT EXISTS idx_session_notes_workspace_session_position
        ON session_notes(workspace, session_key, position);
        CREATE TABLE IF NOT EXISTS evidence_records (
            evidence_id INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace TEXT NOT NULL,
            kind TEXT NOT NULL,
            path TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            artifact_id TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_evidence_records_workspace_updated
        ON evidence_records(workspace, updated_at DESC);
        ",
    )
    .map_err(|err| format!("ensure memory schema failed: {err}"))?;
    conn.execute(
        "INSERT INTO schema_meta(key, value, updated_at) VALUES ('schema_version', ?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
        params![MEMORY_STORE_SCHEMA_VERSION, current_local_timestamp()],
    )
    .map_err(|err| format!("update schema version failed: {err}"))?;
    Ok(conn)
}

fn export_rows(conn: &Connection, query: &str, params_list: &[&str]) -> Result<Vec<Value>, String> {
    let mut statement = conn
        .prepare(query)
        .map_err(|err| format!("prepare sqlite export failed: {err}"))?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(params_list.iter()), |row| {
            row_to_json(row)
        })
        .map_err(|err| format!("query sqlite export failed: {err}"))?;
    let mut values = Vec::new();
    for row in rows {
        values.push(Value::Object(
            row.map_err(|err| format!("read sqlite row failed: {err}"))?,
        ));
    }
    Ok(values)
}

fn row_to_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<Map<String, Value>> {
    let row_ref = row.as_ref();
    let mut map = Map::new();
    for index in 0..row_ref.column_count() {
        let name = row_ref.column_name(index)?.to_string();
        let value = match row.get_ref(index)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(value) => Value::from(value),
            ValueRef::Real(value) => Value::from(value),
            ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).to_string()),
            ValueRef::Blob(value) => Value::String(String::from_utf8_lossy(value).to_string()),
        };
        map.insert(name, value);
    }
    Ok(map)
}

fn delete_memory_items_not_in_sources(
    conn: &Connection,
    workspace: &str,
    sources: &[String],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM memory_items WHERE workspace = ? AND source NOT IN (?, ?, ?, ?, ?)",
        params![
            workspace,
            sources[0].as_str(),
            sources[1].as_str(),
            sources[2].as_str(),
            sources[3].as_str(),
            sources[4].as_str()
        ],
    )
    .map_err(|err| format!("delete memory items outside sources failed: {err}"))?;
    Ok(())
}

fn delete_memory_items_by_sources(
    conn: &Connection,
    workspace: &str,
    sources: &[String],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM memory_items WHERE workspace = ? AND source IN (?, ?, ?, ?, ?)",
        params![
            workspace,
            sources[0].as_str(),
            sources[1].as_str(),
            sources[2].as_str(),
            sources[3].as_str(),
            sources[4].as_str()
        ],
    )
    .map_err(|err| format!("delete authoritative memory items before resync failed: {err}"))?;
    Ok(())
}

fn move_to_archive(source: &Path, destination: &Path) -> Result<String, String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create archive parent failed: {err}"))?;
    }
    let target = if destination.exists() {
        let suffix = current_local_timestamp().replace(':', "").replace('+', "_");
        let stem = destination
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("archive");
        let ext = destination
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default();
        destination.with_file_name(format!("{stem}-{suffix}{ext}"))
    } else {
        destination.to_path_buf()
    };
    fs::rename(source, &target)
        .map_err(|err| format!("move {} failed: {err}", source.display()))?;
    Ok(target.display().to_string())
}

fn read_text_if_exists(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn read_json_if_exists(path: &Path) -> Value {
    let text = read_text_if_exists(path);
    serde_json::from_str(&text).unwrap_or(Value::Object(Map::new()))
}

fn write_text_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    let existing = read_text_if_exists(path);
    if existing == content {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    fs::write(path, content).map_err(|err| format!("write {} failed: {err}", path.display()))?;
    Ok(true)
}

fn write_json_if_changed(path: &Path, payload: &Value) -> Result<bool, String> {
    let text = serde_json::to_string_pretty(payload)
        .map_err(|err| format!("serialize {} failed: {err}", path.display()))?;
    write_text_if_changed(path, &(text + "\n"))
}

fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn is_terminal_token(value: Option<&Value>, terminal_values: &[&str]) -> bool {
    let token = value_text(value).to_lowercase();
    !token.is_empty() && terminal_values.contains(&token.as_str())
}

fn current_local_timestamp() -> String {
    Local::now().to_rfc3339()
}

fn current_local_date() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn safe_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            slug.push(normalized);
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn memory_category_for_file(file_name: &str) -> &'static str {
    match file_name {
        "MEMORY.md" => "invariant",
        "preferences.md" => "preference",
        "decisions.md" => "decision",
        "lessons.md" => "lesson",
        "runbooks.md" => "runbook",
        _ => "general",
    }
}

fn memory_item_id(
    workspace: &str,
    category: &str,
    index: usize,
    summary: &str,
    fallback: &str,
) -> String {
    let summary_slug = {
        let slug = safe_slug(&summary.chars().take(80).collect::<String>());
        if slug.is_empty() {
            safe_slug(fallback)
        } else {
            slug
        }
    };
    format!("{}:{category}:{index}:{summary_slug}", safe_slug(workspace))
}

fn extract_memory_segments(raw: &str) -> Vec<(Vec<String>, String)> {
    let mut segments = Vec::new();
    let mut heading_stack: Vec<String> = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();

    let flush_paragraph = |segments: &mut Vec<(Vec<String>, String)>,
                           heading_stack: &Vec<String>,
                           paragraph: &mut Vec<String>| {
        if paragraph.is_empty() {
            return;
        }
        let body = paragraph
            .iter()
            .map(|part| part.trim())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        paragraph.clear();
        if body.is_empty() || (body.starts_with('_') && body.ends_with('_')) {
            return;
        }
        segments.push((heading_stack.clone(), body));
    };

    for raw_line in raw.lines() {
        let stripped = raw_line.trim();
        if stripped.is_empty() {
            flush_paragraph(&mut segments, &heading_stack, &mut paragraph);
            continue;
        }
        if let Some(captures) = Regex::new(r"^(#{1,6})\s+(.*)$")
            .ok()
            .and_then(|regex| regex.captures(stripped))
        {
            flush_paragraph(&mut segments, &heading_stack, &mut paragraph);
            let level = captures
                .get(1)
                .map(|value| value.as_str().len())
                .unwrap_or(1);
            let title = captures
                .get(2)
                .map(|value| value.as_str().trim().to_string())
                .unwrap_or_default();
            if level == 1 {
                heading_stack.clear();
                continue;
            }
            let depth = level.saturating_sub(2);
            heading_stack.truncate(depth);
            heading_stack.push(title);
            continue;
        }
        if let Some(captures) = Regex::new(r"^(?:[-*]|\d+[.)])\s+(.*)$")
            .ok()
            .and_then(|regex| regex.captures(stripped))
        {
            flush_paragraph(&mut segments, &heading_stack, &mut paragraph);
            let body = captures
                .get(1)
                .map(|value| value.as_str().trim().to_string())
                .unwrap_or_default();
            if !body.is_empty() {
                segments.push((heading_stack.clone(), body));
            }
            continue;
        }
        paragraph.push(stripped.to_string());
    }
    flush_paragraph(&mut segments, &heading_stack, &mut paragraph);
    segments
}

trait StringFallback {
    fn if_empty_then(self, fallback: String) -> String;
}

impl StringFallback for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.trim().is_empty() {
            fallback
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo_root(label: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "router-rs-claude-hooks-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(base.join("artifacts/current/task-1")).expect("create temp repo");
        fs::create_dir_all(base.join(".codex/memory")).expect("create memory dir");
        fs::write(
            base.join(".codex/memory/MEMORY.md"),
            "# 项目长期记忆\n\n## Active Patterns\n\n- AP-1: Sync skills after skill edits\n\n## 稳定决策\n\n- SD-1: Shared CLI memory root lives under `./.codex/memory/`\n\n## Lessons\n\n- L-1: Do not let generated host files drift from runtime truth\n",
        )
        .expect("write shared memory");
        fs::write(
            base.join(".supervisor_state.json"),
            serde_json::to_string_pretty(&json!({
                "task_id": "task-1",
                "task_summary": "repair claude hook",
                "active_phase": "implementing",
                "verification": {"verification_status": "in_progress"},
                "continuity": {"story_state": "active", "resume_allowed": true},
                "next_actions": ["finish rust hook"],
                "execution_contract": {"scope": ["hooks"], "acceptance_criteria": ["smoke passes"]},
                "blockers": {"open_blockers": []},
                "controller": {"primary_owner": "claude-hook", "gate": "none"}
            }))
            .expect("serialize state"),
        )
        .expect("write state");
        fs::write(
            base.join("artifacts/current/active_task.json"),
            serde_json::to_string_pretty(&json!({"task_id": "task-1"})).expect("serialize pointer"),
        )
        .expect("write pointer");
        fs::write(
            base.join("artifacts/current/task-1/SESSION_SUMMARY.md"),
            "- task: repair claude hook\n- phase: implementing\n- status: in_progress\n",
        )
        .expect("write session summary");
        fs::write(
            base.join("artifacts/current/task-1/NEXT_ACTIONS.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "next-actions-v2",
                "next_actions": ["finish rust hook"]
            }))
            .expect("serialize next actions"),
        )
        .expect("write next actions");
        fs::write(
            base.join("artifacts/current/task-1/EVIDENCE_INDEX.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "evidence-index-v2",
                "artifacts": []
            }))
            .expect("serialize evidence"),
        )
        .expect("write evidence");
        fs::write(
            base.join("artifacts/current/task-1/TRACE_METADATA.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "trace-metadata-v2",
                "task": "repair claude hook",
                "matched_skills": ["claude-hooks"]
            }))
            .expect("serialize trace"),
        )
        .expect("write trace");
        base
    }

    #[test]
    fn session_stop_writes_projection() {
        let repo_root = temp_repo_root("projection");
        let response = run_claude_lifecycle_hook("session-stop", &repo_root, 6).expect("hook ok");
        assert_eq!(
            response["canonical_command"],
            Value::String("session-stop".to_string())
        );
        assert_eq!(
            response["projection"]["target_path"],
            Value::String(repo_root.join(CLAUDE_MEMORY_PATH).display().to_string())
        );
        let projection =
            fs::read_to_string(repo_root.join(CLAUDE_MEMORY_PATH)).expect("projection");
        assert!(projection.contains("Claude Startup Projection"));
        assert!(projection.contains("## Startup Rules"));
        assert!(projection.contains("不要用 Got it / Understood"));
        assert!(projection.contains("目标明确时直接执行可验证的小步"));
        assert!(projection.contains("OpenAI/GPT"));
        assert!(projection.contains("repair claude hook"));
        assert!(projection.contains("AP-1: Sync skills after skill edits"));
        assert!(projection.contains("SD-1: Shared CLI memory root lives under `./.codex/memory/`"));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn sync_alias_maps_to_refresh_projection() {
        let repo_root = temp_repo_root("sync-alias");
        let response = run_claude_lifecycle_hook("sync", &repo_root, 6).expect("hook ok");
        assert_eq!(
            response["canonical_command"],
            Value::String("refresh-projection".to_string())
        );
        assert_eq!(
            response["contract"]["summary"],
            Value::String(
                "Refresh the imported Claude projection without touching shared continuity artifacts."
                    .to_string()
            )
        );
        assert_eq!(
            response["projection"]["target_path"],
            Value::String(repo_root.join(CLAUDE_MEMORY_PATH).display().to_string())
        );
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn session_end_repairs_terminal_resume_allowed_before_consolidation() {
        let repo_root = temp_repo_root("session-end");
        fs::write(
            repo_root.join(".supervisor_state.json"),
            serde_json::to_string_pretty(&json!({
                "task_id": "task-1",
                "task_summary": "repair claude hook",
                "active_phase": "finalized",
                "verification": {"verification_status": "completed"},
                "continuity": {"story_state": "completed", "resume_allowed": true},
                "next_actions": ["finish rust hook"],
                "execution_contract": {"scope": ["hooks"], "acceptance_criteria": ["smoke passes"]},
                "blockers": {"open_blockers": []}
            }))
            .expect("serialize state"),
        )
        .expect("write repaired state seed");

        let response = run_claude_lifecycle_hook("session-end", &repo_root, 6).expect("hook ok");

        assert_eq!(
            response["canonical_command"],
            Value::String("session-end".to_string())
        );
        assert!(response.get("consolidation").is_some());
        let repaired_state = read_json_if_exists(&repo_root.join(".supervisor_state.json"));
        assert_eq!(
            repaired_state["continuity"]["resume_allowed"],
            Value::Bool(false)
        );
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn config_change_audit_detects_generated_surfaces() {
        let repo_root = temp_repo_root("audit");
        let payload = json!({
            "source": "project_settings",
            "file_path": ".claude/settings.json"
        });
        let result = run_config_change(&repo_root, &payload).expect("audit ok");
        assert_eq!(
            result["command"],
            Value::String("config-change".to_string())
        );
        assert_eq!(
            result["scope"],
            Value::String("project_settings".to_string())
        );
        assert!(result["notices"]
            .as_array()
            .expect("notices")
            .iter()
            .any(|item| item
                .as_str()
                .unwrap_or("")
                .contains("generated Claude host surfaces")));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn stop_failure_audit_prefers_official_error_field() {
        let payload = json!({
            "error": "rate_limit",
            "error_details": "too many requests"
        });
        let result = run_stop_failure(Path::new("."), &payload).expect("audit ok");
        assert_eq!(
            result["failure_type"],
            Value::String("rate_limit".to_string())
        );
    }
}
