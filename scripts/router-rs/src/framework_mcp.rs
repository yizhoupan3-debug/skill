use crate::{
    build_framework_contract_summary_envelope, build_framework_memory_recall_envelope,
    build_framework_runtime_snapshot_envelope, load_records, resolve_repo_root_arg, MatchRow,
    SkillRecord,
};
use chrono::{Local, SecondsFormat};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "skill-framework-mcp";
const SERVER_VERSION: &str = "0.1.0";
const BOOTSTRAP_FILENAME: &str = "framework_default_bootstrap.json";
const STABLE_MEMORY_FILENAMES: &[&str] = &[
    "MEMORY.md",
    "preferences.md",
    "decisions.md",
    "lessons.md",
    "runbooks.md",
];

pub fn run_framework_mcp_stdio_loop(
    repo_root: Option<&Path>,
    output_dir: Option<&Path>,
) -> Result<(), String> {
    let repo_root = resolve_repo_root_arg(repo_root)?;
    let output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo_root.join("artifacts").join("bootstrap"));
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_framework_mcp_stdio(stdin.lock(), stdout.lock(), &repo_root, &output_dir)
}

pub fn run_framework_mcp_stdio<R: BufRead, W: Write>(
    input: R,
    mut output: W,
    repo_root: &Path,
    output_dir: &Path,
) -> Result<(), String> {
    for line_result in input.lines() {
        let line =
            line_result.map_err(|err| format!("read framework MCP request failed: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_framework_mcp_line(&line, repo_root, output_dir) {
            let encoded = serde_json::to_string(&response)
                .map_err(|err| format!("serialize framework MCP response failed: {err}"))?;
            writeln!(output, "{encoded}")
                .map_err(|err| format!("write framework MCP response failed: {err}"))?;
            output
                .flush()
                .map_err(|err| format!("flush framework MCP response failed: {err}"))?;
        }
    }
    Ok(())
}

fn handle_framework_mcp_line(line: &str, repo_root: &Path, output_dir: &Path) -> Option<Value> {
    let request = match serde_json::from_str::<Value>(line) {
        Ok(value) => value,
        Err(err) => {
            return Some(error_response(
                Value::Null,
                framework_error(
                    "INVALID_INPUT",
                    &format!("Invalid JSON input: {}", err.to_string()),
                    &["send one JSON-RPC object per line"],
                    true,
                ),
            ))
        }
    };
    handle_framework_mcp_request(&request, repo_root, output_dir)
}

fn handle_framework_mcp_request(
    request: &Value,
    repo_root: &Path,
    output_dir: &Path,
) -> Option<Value> {
    let request_id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    if method == "notifications/initialized" {
        return None;
    }
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
            "capabilities": {
                "tools": {"listChanged": false},
                "resources": {"subscribe": false, "listChanged": false},
            },
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tool_definitions()})),
        "tools/call" => handle_tools_call(&params, repo_root, output_dir),
        "resources/list" => Ok(json!({"resources": resource_definitions()})),
        "resources/read" => handle_resources_read(&params, repo_root, output_dir),
        _ => Err(framework_error(
            "UNSUPPORTED_OPERATION",
            &format!("Unsupported JSON-RPC method: {method}"),
            &["call initialize", "call tools/list", "call resources/list"],
            true,
        )),
    };
    Some(match result {
        Ok(payload) => success_response(request_id, payload),
        Err(error) => error_response(request_id, error),
    })
}

fn handle_tools_call(params: &Value, repo_root: &Path, output_dir: &Path) -> Result<Value, Value> {
    let tool_name = require_string(params, "name")?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let structured = match tool_name.as_str() {
        "framework_bootstrap_refresh" => bootstrap_refresh(
            repo_root,
            output_dir,
            optional_string(&arguments, "query", "")?,
            optional_usize(&arguments, "top", 8, 1)?,
        ),
        "framework_memory_recall" => memory_recall(
            repo_root,
            optional_string(&arguments, "query", "")?,
            optional_usize(&arguments, "top", 8, 1)?,
            optional_string(&arguments, "mode", "stable")?,
        ),
        "framework_skill_search" => skill_search(
            repo_root,
            optional_string(&arguments, "query", "")?,
            optional_usize(&arguments, "limit", 10, 1)?,
        ),
        "framework_runtime_snapshot" => runtime_snapshot(repo_root),
        "framework_contract_summary" => contract_summary(repo_root),
        _ => Err(framework_error(
            "INVALID_INPUT",
            &format!("Unknown tool name: {tool_name}"),
            &["call tools/list to inspect available framework tools"],
            true,
        )),
    };
    match structured {
        Ok(payload) => Ok(json!({
            "structuredContent": payload,
            "content": [{"type": "text", "text": serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())}],
            "isError": false,
        })),
        Err(error) => {
            let payload = json!({"ok": false, "error": error});
            Ok(json!({
                "structuredContent": payload,
                "content": [{"type": "text", "text": serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())}],
                "isError": true,
            }))
        }
    }
}

fn handle_resources_read(
    params: &Value,
    repo_root: &Path,
    output_dir: &Path,
) -> Result<Value, Value> {
    let uri = require_string(params, "uri")?;
    let resource = read_resource(&uri, repo_root, output_dir)?;
    Ok(json!({"contents": [resource]}))
}

fn bootstrap_refresh(
    repo_root: &Path,
    output_dir: &Path,
    query: String,
    top: usize,
) -> Result<Value, Value> {
    let result =
        build_default_bootstrap_payload(repo_root, output_dir, &query, top).map_err(|err| {
            framework_error(
                "BOOTSTRAP_REFRESH_FAILED",
                &err,
                &["inspect framework artifacts"],
                true,
            )
        })?;
    Ok(json!({
        "ok": true,
        "workspace": workspace_name_from_root(repo_root),
        "query": query,
        "bootstrap_path": result.get("bootstrap_path").cloned().unwrap_or(Value::Null),
        "task_id": result
            .get("payload")
            .and_then(|value| value.get("bootstrap"))
            .and_then(|value| value.get("task_id"))
            .cloned()
            .unwrap_or(Value::Null),
        "paths": result.get("paths").cloned().unwrap_or_else(|| json!({})),
        "memory_items": result.get("memory_items").cloned().unwrap_or_else(|| json!(0)),
        "proposal_count": result.get("proposal_count").cloned().unwrap_or_else(|| json!(0)),
    }))
}

fn memory_recall(
    repo_root: &Path,
    query: String,
    top: usize,
    mode: String,
) -> Result<Value, Value> {
    let envelope =
        build_framework_memory_recall_envelope(repo_root, &query, top, &mode, None, None, None)
            .map_err(|err| {
                framework_error(
            "RUST_FRAMEWORK_MEMORY_RECALL_FAILED",
            &err,
            &[
                "verify scripts/router-rs builds cleanly",
                "inspect .supervisor_state.json, artifacts/current, and .codex/memory for drift",
            ],
            true,
        )
            })?;
    let payload = envelope.get("memory_recall").cloned().ok_or_else(|| {
        framework_error(
            "RUST_FRAMEWORK_MEMORY_RECALL_FAILED",
            "Missing memory_recall payload.",
            &[],
            true,
        )
    })?;
    Ok(compact_memory_recall_payload(payload))
}

fn skill_search(repo_root: &Path, query: String, limit: usize) -> Result<Value, Value> {
    let exported = export_framework_skills(repo_root).map_err(|err| {
        framework_error(
            "SKILL_SEARCH_FAILED",
            &err,
            &["verify skills routing runtime exists"],
            true,
        )
    })?;
    let mut rows = exported
        .get("skills")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if query.trim().is_empty() {
        rows.sort_by(|left, right| {
            value_string(left.get("slug")).cmp(&value_string(right.get("slug")))
        });
    } else {
        let tokens = query
            .split_whitespace()
            .map(|token| token.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let mut scored = rows
            .into_iter()
            .filter_map(|row| {
                let haystack = skill_export_search_text(&row).to_ascii_lowercase();
                let score = tokens
                    .iter()
                    .filter(|token| haystack.contains(token.as_str()))
                    .count();
                (score > 0).then_some((score, row))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right.0.cmp(&left.0).then_with(|| {
                value_string(left.1.get("slug")).cmp(&value_string(right.1.get("slug")))
            })
        });
        rows = scored.into_iter().map(|(_, row)| row).collect();
    }
    rows.truncate(limit);
    Ok(json!({
        "ok": true,
        "query": query,
        "match_count": rows.len(),
        "matches": rows,
        "source": exported.get("source").cloned().unwrap_or(Value::Null),
    }))
}

fn runtime_snapshot(repo_root: &Path) -> Result<Value, Value> {
    let envelope =
        build_framework_runtime_snapshot_envelope(repo_root, None, None).map_err(|err| {
            framework_error(
                "RUST_RUNTIME_SNAPSHOT_FAILED",
                &err,
                &[
                    "verify scripts/router-rs builds cleanly",
                    "inspect active continuity artifacts under artifacts/current",
                ],
                true,
            )
        })?;
    envelope.get("runtime_snapshot").cloned().ok_or_else(|| {
        framework_error(
            "RUST_RUNTIME_SNAPSHOT_FAILED",
            "Missing runtime_snapshot payload.",
            &[],
            true,
        )
    })
}

fn contract_summary(repo_root: &Path) -> Result<Value, Value> {
    let envelope = build_framework_contract_summary_envelope(repo_root).map_err(|err| {
        framework_error(
            "RUST_CONTRACT_SUMMARY_FAILED",
            &err,
            &[
                "verify scripts/router-rs builds cleanly",
                "inspect .supervisor_state.json and artifacts/current for drift",
            ],
            true,
        )
    })?;
    envelope.get("contract_summary").cloned().ok_or_else(|| {
        framework_error(
            "RUST_CONTRACT_SUMMARY_FAILED",
            "Missing contract_summary payload.",
            &[],
            true,
        )
    })
}

fn read_resource(uri: &str, repo_root: &Path, output_dir: &Path) -> Result<Value, Value> {
    match uri {
        "framework://memory/project" => {
            let text = read_project_memory_bundle(repo_root)?;
            Ok(json!({"uri": uri, "mimeType": "text/markdown", "text": text}))
        }
        "framework://routing/runtime" => {
            let text = read_text_file(
                &repo_root.join("skills").join("SKILL_ROUTING_RUNTIME.json"),
                "Routing runtime file not found.",
            )?;
            Ok(json!({"uri": uri, "mimeType": "application/json", "text": text}))
        }
        "framework://bootstrap/default" => {
            let path = output_dir.join(BOOTSTRAP_FILENAME);
            if !path.is_file() {
                let _ = bootstrap_refresh(repo_root, output_dir, String::new(), 8)?;
            }
            let text = read_text_file(&path, "Bootstrap payload not found after refresh.")?;
            Ok(json!({"uri": uri, "mimeType": "application/json", "text": text}))
        }
        "framework://supervisor/state" => {
            let text = read_text_file(
                &repo_root.join(".supervisor_state.json"),
                "Supervisor state file not found.",
            )?;
            Ok(json!({"uri": uri, "mimeType": "application/json", "text": text}))
        }
        "framework://artifacts/index" => {
            let snapshot = runtime_snapshot(repo_root)?;
            let contract = contract_summary(repo_root)?;
            let payload = json!({
                "workspace": workspace_name_from_root(repo_root),
                "collected_at": snapshot.get("collected_at").cloned().unwrap_or(Value::Null),
                "current_root": snapshot.get("current_root").cloned().unwrap_or(Value::Null),
                "continuity": snapshot.get("continuity").cloned().unwrap_or_else(|| json!({})),
                "next_actions": contract.get("next_actions").cloned().unwrap_or_else(|| json!([])),
                "trace_skills": contract.get("trace_skills").cloned().unwrap_or_else(|| json!([])),
                "evidence_count": snapshot.get("evidence_count").cloned().unwrap_or_else(|| json!(0)),
                "paths": snapshot.get("paths").cloned().unwrap_or_else(|| json!({})),
            });
            let text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string());
            Ok(json!({"uri": uri, "mimeType": "application/json", "text": text}))
        }
        _ => Err(framework_error(
            "INVALID_INPUT",
            &format!("Unknown resource URI: {uri}"),
            &["call resources/list to inspect available framework resources"],
            true,
        )),
    }
}

fn compact_memory_recall_payload(mut payload: Value) -> Value {
    let retrieval = payload
        .get("retrieval")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let continuity = payload
        .get("continuity")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(map) = payload.as_object_mut() {
        map.insert(
            "retrieval".to_string(),
            json!({
                "workspace": retrieval.get("workspace").cloned().unwrap_or(Value::Null),
                "topic": retrieval.get("topic").cloned().unwrap_or(Value::Null),
                "mode": retrieval.get("mode").cloned().unwrap_or(Value::Null),
                "memory_root": retrieval.get("memory_root").cloned().unwrap_or(Value::Null),
                "sqlite_path": retrieval.get("sqlite_path").cloned().unwrap_or(Value::Null),
                "active_task_id": retrieval.get("active_task_id").cloned().unwrap_or(Value::Null),
                "active_task_included": retrieval.get("active_task_included").cloned().unwrap_or_else(|| json!(false)),
                "freshness": retrieval.get("freshness").cloned().unwrap_or_else(|| json!({})),
                "items": retrieval.get("items").cloned().unwrap_or_else(|| json!([])),
            }),
        );
        map.insert(
            "continuity".to_string(),
            json!({
                "state": continuity.get("state").cloned().unwrap_or(Value::Null),
                "can_resume": continuity.get("can_resume").cloned().unwrap_or_else(|| json!(false)),
                "task": continuity.get("task").cloned().unwrap_or(Value::Null),
                "phase": continuity.get("phase").cloned().unwrap_or(Value::Null),
                "status": continuity.get("status").cloned().unwrap_or(Value::Null),
                "next_actions": continuity.get("next_actions").cloned().unwrap_or_else(|| json!([])),
                "blockers": continuity.get("blockers").cloned().unwrap_or_else(|| json!([])),
                "recovery_hints": continuity.get("recovery_hints").cloned().unwrap_or_else(|| json!([])),
                "current_execution": continuity.get("current_execution").cloned().unwrap_or(Value::Null),
                "recent_completed_execution": continuity.get("recent_completed_execution").cloned().unwrap_or(Value::Null),
            }),
        );
        map.remove("prompt_payload");
        map.remove("active_task");
        map.remove("focused_task");
    }
    payload
}

fn build_default_bootstrap_payload(
    repo_root: &Path,
    output_dir: &Path,
    query: &str,
    top: usize,
) -> Result<Value, String> {
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let memory =
        build_framework_memory_recall_envelope(&repo_root, query, top, "active", None, None, None)?;
    let memory_recall = memory
        .get("memory_recall")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "router-rs memory recall payload missing memory_recall object".to_string()
        })?;
    let prompt_payload = memory_recall
        .get("prompt_payload")
        .cloned()
        .unwrap_or_else(|| Value::Object(memory_recall.clone()));
    let continuity_decision = prompt_payload
        .get("continuity_decision")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let workspace = prompt_payload
        .get("workspace")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| workspace_name_from_root(&repo_root));
    let created_at = current_local_timestamp();
    let task_id = continuity_decision
        .get("task_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| build_framework_task_id(query, &workspace, &created_at));
    let runtime = export_framework_skills(&repo_root)?;
    let proposals = json!({"proposal_count": 0, "proposals": []});
    let payload = json!({
        "skills-export": runtime,
        "memory-bootstrap": prompt_payload,
        "evolution-proposals": proposals,
        "bootstrap": {
            "query": query,
            "workspace": workspace,
            "repo_root": repo_root.to_string_lossy(),
            "task_id": task_id,
            "created_at": created_at,
            "source_task": continuity_decision.get("source_task").cloned().unwrap_or(Value::Null),
            "query_matches_active_task": continuity_decision
                .get("query_matches_active_task")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "ignored_root_continuity": continuity_decision
                .get("ignored_root_continuity")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
    });
    let task_output_dir = output_dir.join(&task_id);
    fs::create_dir_all(&task_output_dir).map_err(|err| err.to_string())?;
    let bootstrap_path = task_output_dir.join(BOOTSTRAP_FILENAME);
    let mirror_bootstrap_path = output_dir.join(BOOTSTRAP_FILENAME);
    write_json_if_changed(&bootstrap_path, &payload)?;
    write_json_if_changed(&mirror_bootstrap_path, &payload)?;
    Ok(json!({
        "bootstrap_path": bootstrap_path.to_string_lossy(),
        "paths": {
            "output_dir": output_dir.to_string_lossy(),
            "task_output_dir": task_output_dir.to_string_lossy(),
            "repo_root": repo_root.to_string_lossy(),
            "memory_root": memory_recall.get("memory_root").and_then(Value::as_str).unwrap_or(""),
            "mirror_bootstrap_path": mirror_bootstrap_path.to_string_lossy(),
        },
        "memory_items": memory_recall
            .get("retrieval")
            .and_then(Value::as_object)
            .and_then(|retrieval| retrieval.get("items"))
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0),
        "proposal_count": 0,
        "payload": payload,
    }))
}

fn export_framework_skills(repo_root: &Path) -> Result<Value, String> {
    let runtime_path = repo_root.join("skills").join("SKILL_ROUTING_RUNTIME.json");
    let approval_path = repo_root.join("skills").join("SKILL_APPROVAL_POLICY.json");
    let approvals = read_json_if_exists(&approval_path);
    let records = load_records(Some(&runtime_path), None)?;
    let rows = records
        .iter()
        .map(|record| skill_record_to_export(record, &approvals))
        .collect::<Vec<_>>();
    Ok(json!({
        "skills": rows,
        "count": rows.len(),
        "source": "skills/SKILL_ROUTING_RUNTIME.json",
    }))
}

fn skill_record_to_export(record: &SkillRecord, approvals: &Value) -> Value {
    json!({
        "slug": record.slug,
        "layer": record.layer,
        "owner": record.owner,
        "gate": record.gate,
        "session_start": record.session_start,
        "summary": record.summary.chars().take(200).collect::<String>(),
        "triggers": record.trigger_hints,
        "agent_role": record.owner,
        "approval": approvals.get(&record.slug).cloned().unwrap_or(Value::Null),
    })
}

fn skill_export_search_text(row: &Value) -> String {
    let mut parts = vec![
        value_string(row.get("slug")),
        value_string(row.get("layer")),
        value_string(row.get("owner")),
        value_string(row.get("gate")),
        value_string(row.get("summary")),
    ];
    if let Some(triggers) = row.get("triggers").and_then(Value::as_array) {
        parts.extend(triggers.iter().map(|item| value_string(Some(item))));
    }
    if let Some(triggers) = row.get("trigger_hints").and_then(Value::as_array) {
        parts.extend(triggers.iter().map(|item| value_string(Some(item))));
    }
    parts.join(" ")
}

fn read_project_memory_bundle(repo_root: &Path) -> Result<String, Value> {
    let memory_root = repo_root.join(".codex").join("memory");
    let mut documents = Vec::<(&str, String)>::new();
    for file_name in STABLE_MEMORY_FILENAMES {
        let path = memory_root.join(file_name);
        if !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap_or_default();
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            documents.push((file_name, trimmed.to_string()));
        }
    }
    if documents.is_empty() {
        return Err(framework_error(
            "MISSING_RESOURCE",
            "Project memory file not found.",
            &[
                "refresh the bootstrap bundle",
                "verify the repository artifacts exist",
            ],
            true,
        ));
    }
    if documents.len() == 1 && documents[0].0 == "MEMORY.md" {
        return Ok(documents.remove(0).1);
    }
    let mut lines = vec!["# Project Memory Bundle".to_string(), String::new()];
    for (file_name, text) in documents {
        lines.push(format!("## {file_name}"));
        lines.push(String::new());
        lines.push(text);
        lines.push(String::new());
    }
    Ok(lines.join("\n").trim().to_string())
}

fn read_text_file(path: &Path, missing_message: &str) -> Result<String, Value> {
    if !path.is_file() {
        return Err(framework_error(
            "MISSING_RESOURCE",
            missing_message,
            &[
                "refresh the bootstrap bundle",
                "verify the repository artifacts exist",
            ],
            true,
        ));
    }
    fs::read_to_string(path).map_err(|err| {
        framework_error(
            "MISSING_RESOURCE",
            &format!("{missing_message} {err}"),
            &[
                "refresh the bootstrap bundle",
                "verify the repository artifacts exist",
            ],
            true,
        )
    })
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "framework_bootstrap_refresh",
            "description": "Refresh the local framework bootstrap bundle that packages skill routing, memory recall, and evolution proposals for this workspace.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Optional focus topic for memory recall."},
                    "top": {"type": "integer", "minimum": 1, "description": "Maximum memory items to include."},
                },
            },
        }),
        json!({
            "name": "framework_memory_recall",
            "description": "Recall stable framework memory, with optional active/history/debug expansion modes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Topic or keyword to retrieve."},
                    "top": {"type": "integer", "minimum": 1, "description": "Maximum retrieved items."},
                    "mode": {"type": "string", "enum": ["stable", "active", "history", "debug"], "description": "Recall mode. Defaults to stable."},
                },
            },
        }),
        json!({
            "name": "framework_skill_search",
            "description": "Search the local skill framework by skill name, summary, owner, gate, or trigger phrase.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Query string matched against local skills."},
                    "limit": {"type": "integer", "minimum": 1, "description": "Maximum returned matches."},
                },
                "required": ["query"],
            },
        }),
        json!({
            "name": "framework_runtime_snapshot",
            "description": "Read the current supervisor and artifact snapshot for this workspace.",
            "inputSchema": {"type": "object", "properties": {}},
        }),
        json!({
            "name": "framework_contract_summary",
            "description": "Summarize the current execution contract, blockers, evidence, and next actions.",
            "inputSchema": {"type": "object", "properties": {}},
        }),
    ]
}

fn resource_definitions() -> Vec<Value> {
    vec![
        json!({"uri": "framework://memory/project", "name": "Project Memory", "description": "Checked-in long-term framework memory for this repository.", "mimeType": "text/markdown"}),
        json!({"uri": "framework://routing/runtime", "name": "Routing Runtime", "description": "Machine-readable skill routing runtime map.", "mimeType": "application/json"}),
        json!({"uri": "framework://bootstrap/default", "name": "Default Bootstrap", "description": "Current framework bootstrap payload for this workspace.", "mimeType": "application/json"}),
        json!({"uri": "framework://supervisor/state", "name": "Supervisor State", "description": "Latest persisted supervisor state for the active workspace.", "mimeType": "application/json"}),
        json!({"uri": "framework://artifacts/index", "name": "Artifact Index", "description": "Compact index of current execution artifacts, evidence, and next actions.", "mimeType": "application/json"}),
    ]
}

fn require_string(payload: &Value, key: &str) -> Result<String, Value> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            framework_error(
                "INVALID_INPUT",
                &format!("Missing required string field '{key}'"),
                &[&format!("provide a non-empty string for '{key}'")],
                true,
            )
        })
}

fn optional_string(payload: &Value, key: &str, default: &str) -> Result<String, Value> {
    match payload.get(key) {
        None => Ok(default.to_string()),
        Some(Value::String(value)) => Ok(value.clone()),
        Some(other) => Err(framework_error(
            "INVALID_INPUT",
            &format!("Expected string for '{key}', got {}", json_type_name(other)),
            &[&format!("pass '{key}' as a string")],
            true,
        )),
    }
}

fn optional_usize(
    payload: &Value,
    key: &str,
    default: usize,
    minimum: usize,
) -> Result<usize, Value> {
    match payload.get(key) {
        None => Ok(default),
        Some(Value::Number(value)) => value
            .as_u64()
            .and_then(|item| usize::try_from(item).ok())
            .filter(|item| *item >= minimum)
            .ok_or_else(|| {
                framework_error(
                    "INVALID_INPUT",
                    &format!("Expected '{key}' >= {minimum}, got {value}"),
                    &[&format!("pass '{key}' as an integer >= {minimum}")],
                    true,
                )
            }),
        Some(other) => Err(framework_error(
            "INVALID_INPUT",
            &format!(
                "Expected integer for '{key}', got {}",
                json_type_name(other)
            ),
            &[&format!("pass '{key}' as an integer >= {minimum}")],
            true,
        )),
    }
}

fn success_response(request_id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": request_id, "result": result})
}

fn error_response(request_id: Value, error: Value) -> Value {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Framework MCP server error");
    json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "error": {"code": -32000, "message": message, "data": error},
    })
}

fn framework_error(
    code: &str,
    message: &str,
    suggested_next_actions: &[&str],
    recoverable: bool,
) -> Value {
    json!({
        "code": code,
        "message": message,
        "recoverable": recoverable,
        "suggested_next_actions": suggested_next_actions,
    })
}

fn read_json_if_exists(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_json_if_changed(path: &Path, payload: &Value) -> Result<bool, String> {
    let content = serde_json::to_string_pretty(payload)
        .map_err(|err| format!("serialize {} failed: {err}", path.display()))?
        + "\n";
    let existing = fs::read_to_string(path).unwrap_or_default();
    if existing == content {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    fs::write(path, content).map_err(|err| format!("write {} failed: {err}", path.display()))?;
    Ok(true)
}

fn current_local_timestamp() -> String {
    Local::now()
        .to_rfc3339_opts(SecondsFormat::Secs, false)
        .to_string()
}

fn build_framework_task_id(query: &str, workspace: &str, created_at: &str) -> String {
    let stamp = created_at
        .chars()
        .filter(|value| value.is_ascii_alphanumeric())
        .collect::<String>();
    let base = safe_slug(if query.trim().is_empty() {
        workspace
    } else {
        query
    });
    if stamp.is_empty() {
        base
    } else {
        let suffix = if stamp.len() > 14 {
            &stamp[stamp.len() - 14..]
        } else {
            &stamp
        };
        format!("{base}-{suffix}")
    }
}

fn safe_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let trimmed = slug.trim_matches(&['.', '_', '-'][..]).to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

fn workspace_name_from_root(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workspace")
        .to_string()
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| value_string(Some(item)))
            .collect::<Vec<_>>()
            .join(" "),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

#[allow(dead_code)]
fn _search_match_to_export(row: &MatchRow) -> Value {
    json!({
        "slug": row.slug,
        "layer": row.layer,
        "owner": row.owner,
        "gate": row.gate,
        "summary": row.description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("router-rs-{label}-{suffix}"));
        fs::create_dir_all(&path).expect("create temp root");
        path
    }

    fn write_text(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write text");
    }

    fn write_json(path: &Path, payload: &Value) {
        write_text(
            path,
            &(serde_json::to_string_pretty(payload).expect("serialize json") + "\n"),
        );
    }

    fn seed_runtime_artifacts(repo_root: &Path, terminal: bool) {
        let task_id = if terminal {
            "checklist-series-final-closeout-20260418210000"
        } else {
            "active-bootstrap-repair-20260418210000"
        };
        let task_root = repo_root.join("artifacts").join("current").join(task_id);
        if terminal {
            write_text(
                &task_root.join("SESSION_SUMMARY.md"),
                "- task: checklist-series final closeout\n- phase: finalized\n- status: completed\n",
            );
            write_json(
                &repo_root.join(".supervisor_state.json"),
                &json!({
                    "task_id": task_id,
                    "task_summary": "checklist-series final closeout",
                    "active_phase": "finalized",
                    "verification": {"verification_status": "completed"},
                    "continuity": {"story_state": "completed", "resume_allowed": false},
                    "execution_contract": {
                        "goal": "Do not treat closeout as active continuity",
                        "scope": ["memory/CLAUDE_MEMORY.md"],
                    },
                }),
            );
            write_json(
                &task_root.join("NEXT_ACTIONS.json"),
                &json!({"next_actions": ["Start a new standalone task before continuing related work"]}),
            );
            write_json(
                &task_root.join("TRACE_METADATA.json"),
                &json!({"task": "checklist-series final closeout", "matched_skills": ["checklist-fixer"]}),
            );
            write_json(
                &task_root.join("EVIDENCE_INDEX.json"),
                &json!({"artifacts": []}),
            );
            write_json(
                &repo_root.join("artifacts/current/task_registry.json"),
                &json!({
                    "schema_version": "task-registry-v1",
                    "focus_task_id": task_id,
                    "tasks": [{
                        "task_id": task_id,
                        "task": "checklist-series final closeout",
                        "phase": "finalized",
                        "status": "completed",
                        "resume_allowed": false,
                    }],
                }),
            );
        } else {
            write_text(
                &task_root.join("SESSION_SUMMARY.md"),
                "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
            );
            write_json(
                &repo_root.join(".supervisor_state.json"),
                &json!({
                    "task_id": task_id,
                    "task_summary": "active bootstrap repair",
                    "active_phase": "implementation",
                    "verification": {"verification_status": "in_progress"},
                    "continuity": {"story_state": "active", "resume_allowed": true},
                    "primary_owner": "skill-framework-developer",
                    "execution_contract": {
                        "goal": "Repair stale bootstrap injection",
                        "scope": ["scripts/router-rs/src/framework_runtime.rs"],
                        "acceptance_criteria": ["completed tasks never appear as current execution"],
                    },
                    "blockers": {"open_blockers": ["Need regression coverage"]},
                }),
            );
            write_json(
                &task_root.join("NEXT_ACTIONS.json"),
                &json!({"next_actions": ["Patch classifier", "Run MCP regression tests"]}),
            );
            write_json(
                &task_root.join("TRACE_METADATA.json"),
                &json!({"task": "active bootstrap repair", "matched_skills": ["execution-controller-coding", "skill-framework-developer"]}),
            );
            write_json(
                &task_root.join("EVIDENCE_INDEX.json"),
                &json!({"artifacts": []}),
            );
            write_json(
                &repo_root.join("artifacts/current/task_registry.json"),
                &json!({
                    "schema_version": "task-registry-v1",
                    "focus_task_id": task_id,
                    "tasks": [{
                        "task_id": task_id,
                        "task": "active bootstrap repair",
                        "phase": "implementation",
                        "status": "in_progress",
                        "resume_allowed": true,
                    }],
                }),
            );
        }
        write_json(
            &repo_root.join("artifacts/current/active_task.json"),
            &json!({"task_id": task_id}),
        );
        write_json(
            &repo_root.join("artifacts/current/focus_task.json"),
            &json!({"task_id": task_id}),
        );
        write_text(
            &repo_root.join("artifacts/current/SESSION_SUMMARY.md"),
            &fs::read_to_string(task_root.join("SESSION_SUMMARY.md")).expect("read summary"),
        );
        write_json(
            &repo_root.join("artifacts/current/NEXT_ACTIONS.json"),
            &read_json_if_exists(&task_root.join("NEXT_ACTIONS.json")),
        );
        write_json(
            &repo_root.join("artifacts/current/TRACE_METADATA.json"),
            &read_json_if_exists(&task_root.join("TRACE_METADATA.json")),
        );
        write_json(
            &repo_root.join("artifacts/current/EVIDENCE_INDEX.json"),
            &json!({"artifacts": []}),
        );
    }

    #[test]
    fn framework_mcp_stdio_lists_tools_and_resources() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root");
        let output_dir = temp_root("framework-mcp-bootstrap");
        let input = Cursor::new(format!(
            "{}\n{}\n",
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "resources/list", "params": {}})
        ));
        let mut output = Vec::new();
        run_framework_mcp_stdio(input, &mut output, &repo_root, &output_dir).expect("run mcp");
        let text = String::from_utf8(output).expect("utf8");
        let lines = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("parse line"))
            .collect::<Vec<_>>();
        assert_eq!(lines[0]["result"]["serverInfo"]["name"], SERVER_NAME);
        assert!(lines[1]["result"]["resources"]
            .as_array()
            .expect("resources")
            .iter()
            .any(|item| item["uri"] == "framework://memory/project"));
    }

    #[test]
    fn framework_mcp_memory_project_resource_reads_codex_memory() {
        let repo_root = temp_root("framework-mcp-memory");
        let output_dir = repo_root.join("out");
        write_text(
            &repo_root.join(".codex/memory/MEMORY.md"),
            "# 项目长期记忆\n",
        );
        let input = Cursor::new(format!(
            "{}\n",
            json!({"jsonrpc": "2.0", "id": 1, "method": "resources/read", "params": {"uri": "framework://memory/project"}})
        ));
        let mut output = Vec::new();
        run_framework_mcp_stdio(input, &mut output, &repo_root, &output_dir).expect("run mcp");
        let line = serde_json::from_slice::<Value>(&output).expect("parse response");
        assert!(line["result"]["contents"][0]["text"]
            .as_str()
            .expect("text")
            .contains("项目长期记忆"));
    }

    #[test]
    fn framework_mcp_bootstrap_refresh_writes_payload() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root");
        let output_dir = temp_root("framework-mcp-refresh");
        let input = Cursor::new(format!(
            "{}\n",
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "framework_bootstrap_refresh",
                    "arguments": {"query": "memory integration", "top": 4}
                }
            })
        ));
        let mut output = Vec::new();
        run_framework_mcp_stdio(input, &mut output, &repo_root, &output_dir).expect("run mcp");
        let line = serde_json::from_slice::<Value>(&output).expect("parse response");
        let structured = &line["result"]["structuredContent"];
        let bootstrap_path = PathBuf::from(
            structured["bootstrap_path"]
                .as_str()
                .expect("bootstrap path"),
        );
        assert_eq!(structured["ok"], Value::Bool(true));
        assert!(bootstrap_path.is_file());
        assert_eq!(
            bootstrap_path.file_name().and_then(|value| value.to_str()),
            Some(BOOTSTRAP_FILENAME)
        );
    }

    #[test]
    fn framework_mcp_artifacts_index_is_actionable() {
        let repo_root = temp_root("framework-mcp-artifacts");
        let output_dir = repo_root.join("out");
        seed_runtime_artifacts(&repo_root, true);
        let input = Cursor::new(format!(
            "{}\n",
            json!({"jsonrpc": "2.0", "id": 1, "method": "resources/read", "params": {"uri": "framework://artifacts/index"}})
        ));
        let mut output = Vec::new();
        run_framework_mcp_stdio(input, &mut output, &repo_root, &output_dir).expect("run mcp");
        let line = serde_json::from_slice::<Value>(&output).expect("parse response");
        let payload = serde_json::from_str::<Value>(
            line["result"]["contents"][0]["text"]
                .as_str()
                .expect("payload text"),
        )
        .expect("decode payload");
        assert_eq!(
            payload["workspace"],
            Value::String(workspace_name_from_root(&repo_root))
        );
        assert!(payload["next_actions"].is_array());
    }
}
