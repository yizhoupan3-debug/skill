mod common;

use common::{
    host_integration_json, json_from_output, read_json, router_rs_command, write_json, write_text,
};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn render_context_stable_mode_excludes_active_task_and_archive() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    seed_stable_memory(tmp.path());
    write_text(
        &tmp.path()
            .join(".codex/memory/archive/pre-cutover-2026-04-18/sessions/2026-04-18.md"),
        "task=old\n",
    );
    let result = render_context(tmp.path(), "active bootstrap repair", "stable", 8);
    assert_eq!(result["mode"], "stable");
    assert_eq!(result["active_task_included"], false);
    assert!(items(&result)
        .iter()
        .all(|item| item["path"] != "runtime/current_task.md"));
    assert!(items(&result)
        .iter()
        .all(|item| !item["path"].as_str().unwrap().contains("archive/")));
}

#[test]
fn render_context_active_mode_includes_matching_current_task_when_fresh() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    seed_stable_memory(tmp.path());
    let result = render_context(tmp.path(), "active bootstrap repair", "active", 8);
    assert_eq!(result["active_task_included"], true);
    assert_eq!(result["freshness"]["state"], "fresh");
    assert!(items(&result)
        .iter()
        .any(|item| item["path"] == "runtime/current_task.md"));
}

#[test]
fn render_context_active_mode_ignores_stale_continuity_cache() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    seed_stable_memory(tmp.path());
    let state_path = tmp
        .path()
        .join("artifacts/current/active-bootstrap-repair-20260418210000/CONTINUITY_STATE.json");
    write_json(
        &state_path,
        &json!({
            "schema_version": "continuity-state-v1",
            "source_task_id": "older-task",
            "content_hash": "stale",
            "source_updated_at": "2026-04-18T20:00:00+08:00"
        }),
    );
    let result = render_context(tmp.path(), "active bootstrap repair", "active", 8);
    assert_eq!(result["active_task_included"], true);
    assert_eq!(result["freshness"]["state"], "fresh");
    let state = read_json(&state_path);
    assert_eq!(state["source_task_id"], "older-task");
}

#[test]
fn render_context_active_mode_does_not_require_continuity_cache() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    write_text(
        &tmp.path().join(".codex/memory/MEMORY.md"),
        "# 项目长期记忆\n",
    );
    write_text(
        &tmp.path().join(".codex/memory/preferences.md"),
        "# preferences\n",
    );
    let result = render_context(tmp.path(), "active bootstrap repair", "active", 8);
    assert_eq!(result["active_task_included"], true);
    assert!(!tmp
        .path()
        .join("artifacts/current/active-bootstrap-repair-20260418210000/CONTINUITY_STATE.json")
        .exists());
    assert_eq!(result["freshness"]["state"], "fresh");
}

#[test]
fn debug_mode_writes_continuity_cache_for_inspection_only() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    seed_stable_memory(tmp.path());
    let result = render_context(tmp.path(), "active bootstrap repair", "debug", 8);
    assert!(items(&result)
        .iter()
        .any(|item| item["path"] == "runtime/CONTINUITY_STATE.json"));
    assert!(tmp
        .path()
        .join("artifacts/current/active-bootstrap-repair-20260418210000/CONTINUITY_STATE.json")
        .is_file());
}

#[test]
fn render_context_history_mode_can_read_archive() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    seed_stable_memory(tmp.path());
    write_text(
        &tmp.path()
            .join(".codex/memory/archive/pre-cutover-2026-04-18/sessions/2026-04-18.md"),
        "task=old closeout\n",
    );
    let result = render_context(tmp.path(), "old closeout", "history", 8);
    assert!(items(&result)
        .iter()
        .any(|item| item["path"].as_str().unwrap().contains("archive/")));
}

#[test]
fn default_modes_do_not_include_sqlite_sections() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    seed_stable_memory(tmp.path());
    seed_sqlite_memory(tmp.path());
    for mode in ["stable", "active", "history"] {
        let result = render_context(tmp.path(), "sqlite", mode, 8);
        assert!(items(&result)
            .iter()
            .all(|item| !item["path"].as_str().unwrap().starts_with("sqlite/")));
    }
}

#[test]
fn stable_mode_without_topic_compacts_stable_documents() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    write_text(
        &tmp.path().join(".codex/memory/MEMORY.md"),
        "# 项目长期记忆\n\n## Active Patterns\n\n- AP-1: keep recall compact\n- AP-2: use stable summaries\n\n## 稳定决策\n\n- SD-1: avoid full document injection\n",
    );
    write_text(
        &tmp.path().join(".codex/memory/runbooks.md"),
        "# runbooks\n\n## 标准操作\n\n- step 1\n- step 2\n- step 3\n",
    );
    let result = render_context(tmp.path(), "", "stable", 8);
    let memory_item = items(&result)
        .into_iter()
        .find(|item| item["path"] == "MEMORY.md")
        .unwrap();
    let runbook_item = items(&result)
        .into_iter()
        .find(|item| item["path"] == "runbooks.md")
        .unwrap();
    assert!(memory_item["content"]
        .as_str()
        .unwrap()
        .contains("AP-1: keep recall compact"));
    assert!(!memory_item["content"]
        .as_str()
        .unwrap()
        .contains("avoid full document injection"));
    assert!(runbook_item["content"].as_str().unwrap().contains("step 1"));
    assert!(!runbook_item["content"].as_str().unwrap().contains("step 3"));
}

#[test]
fn default_recall_prefers_tracked_project_memory_over_stale_codex_memory() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    write_text(
        &tmp.path().join("memory/decisions.md"),
        "# decisions\n\n- Rust-owned tracked memory is canonical for runtime recall.\n",
    );
    write_text(
        &tmp.path().join(".codex/memory/decisions.md"),
        "# decisions\n\n- stale codex memory should not be selected.\n",
    );

    let result = render_context(tmp.path(), "runtime", "stable", 8);
    let expected_memory_root = tmp.path().canonicalize().unwrap().join("memory");
    assert_eq!(
        canonical_value_path(&result["memory_root"]),
        expected_memory_root.display().to_string()
    );
    let content = items(&result)
        .into_iter()
        .map(|item| item["content"].as_str().unwrap().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(content.contains("Rust-owned tracked memory is canonical"));
    assert!(!content.contains("stale codex memory"));
}

#[test]
fn stable_recall_tokenizes_cjk_punctuation_and_mixed_runtime_terms() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    write_text(
        &tmp.path().join("memory/decisions.md"),
        "# decisions\n\n- Rust 是路由、host entrypoint、hook、framework runtime 和记忆策略的真源。\n",
    );

    let result = render_context(tmp.path(), "记忆系统，runtime系统", "stable", 8);
    let content = items(&result)
        .into_iter()
        .map(|item| item["content"].as_str().unwrap().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(content.contains("framework runtime 和记忆策略"));
}

#[test]
fn memory_automation_uses_same_default_memory_root_as_recall() {
    let tmp = tempdir().unwrap();
    write_text(
        &tmp.path().join("memory/MEMORY.md"),
        "# 项目长期记忆\n\n## Runtime\n\n- tracked memory powers automation.\n",
    );
    write_text(
        &tmp.path().join(".codex/memory/MEMORY.md"),
        "# stale codex memory\n",
    );

    let repo_root = tmp.path().to_string_lossy().to_string();
    let output = host_integration_json(&[
        "run-memory-automation",
        "--repo-root",
        &repo_root,
        "--workspace",
        "skill",
        "--query",
        "runtime",
        "--top",
        "8",
    ]);
    let expected_memory_root = tmp.path().canonicalize().unwrap().join("memory");
    assert_eq!(
        canonical_value_path(&output["memory_root"]),
        expected_memory_root.display().to_string()
    );
    assert_eq!(
        canonical_value_path(&output["retrieval"]["memory_root"]),
        expected_memory_root.display().to_string()
    );
    let memory_text = fs::read_to_string(tmp.path().join("memory/MEMORY.md")).unwrap();
    assert!(memory_text.contains("tracked memory powers automation"));
    assert!(!memory_text.contains("Project-local memory is managed"));
}

#[test]
fn debug_mode_exposes_sqlite_sections() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    seed_stable_memory(tmp.path());
    seed_sqlite_memory(tmp.path());
    let result = render_context(tmp.path(), "sqlite", "debug", 8);
    assert!(items(&result)
        .iter()
        .any(|item| item["path"] == "sqlite/memory_items.md"));
}

#[test]
fn stable_recall_does_not_fallback_on_partial_sqlite_token_match() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    write_text(
        &tmp.path().join(".codex/memory/MEMORY.md"),
        "# 项目长期记忆\n\n## 稳定决策\n\n### 执行编排\n\n- runtime contract only\n",
    );
    insert_memory_item(
        tmp.path(),
        "runtime-contract",
        "decision",
        "sqlite",
        "runtime contract only",
        "tracks execution guarantees only",
        &["runtime", "contract"],
    );
    let result = render_context(tmp.path(), "runtime observability", "stable", 8);
    assert!(items(&result).is_empty());
}

#[test]
fn memory_store_search_requires_strong_query_match() {
    let tmp = tempdir().unwrap();
    seed_runtime(tmp.path(), "active bootstrap repair");
    seed_stable_memory(tmp.path());
    insert_memory_item(
        tmp.path(),
        "runtime-contract",
        "decision",
        "sqlite",
        "runtime contract only",
        "tracks execution guarantees only",
        &["runtime", "contract"],
    );
    insert_memory_item(
        tmp.path(),
        "runtime-observability",
        "decision",
        "sqlite",
        "runtime observability contract",
        "tracks runtime observability guarantees",
        &["runtime", "observability"],
    );
    let result = render_context(tmp.path(), "runtime observability", "debug", 8);
    let sqlite_item = items(&result)
        .into_iter()
        .find(|item| item["path"] == "sqlite/memory_items.md")
        .unwrap();
    let content = sqlite_item["content"].as_str().unwrap();
    assert!(content.contains("runtime observability contract"));
    assert!(!content.contains("runtime contract only"));
}

#[test]
fn rust_memory_policy_extracts_and_dedupes_facts() {
    let payload = json!({
        "messages": [
            {"role": "user", "content": "remember: prefer Rust-owned memory policy."},
            {"role": "assistant", "content": "noted"},
            {"role": "user", "content": "Remember: prefer Rust-owned memory policy."},
            {"role": "user", "content": "决定: prompt wording policy is Rust-owned now."}
        ],
        "limit": 8
    });
    let payload_text = payload.to_string();
    let output = common::run(router_rs_command([
        "framework",
        "memory-policy",
        "--input-json",
        &payload_text,
    ]));
    let result = json_from_output(&output);
    let policy = &result["memory_policy"];
    assert_eq!(result["authority"], "rust-framework-memory-policy");
    assert_eq!(policy["policy_owner"], "rust");
    assert_eq!(policy["facts"].as_array().unwrap().len(), 2);
    assert_eq!(policy["persistence"]["requested"], false);
    assert!(policy["facts"]
        .as_array()
        .unwrap()
        .contains(&json!("prefer Rust-owned memory policy")));
    assert!(policy["facts"]
        .as_array()
        .unwrap()
        .contains(&json!("prompt wording policy is Rust-owned now")));
}

#[test]
fn rust_memory_policy_ignores_assistant_text_and_part_arrays() {
    let payload = json!({
        "messages": [
            {"role": "assistant", "content": "remember: assistant notes are not user memory."},
            {"role": "user", "content": [
                {"type": "text", "text": "记住: 用户消息数组也能提取"},
                {"type": "text", "text": "我更喜欢 结构化记忆"}
            ]}
        ],
        "limit": 8
    });
    let payload_text = payload.to_string();
    let output = common::run(router_rs_command([
        "framework",
        "memory-policy",
        "--input-json",
        &payload_text,
    ]));
    let result = json_from_output(&output);
    let policy = &result["memory_policy"];
    assert_eq!(
        policy["facts"],
        json!(["用户消息数组也能提取", "结构化记忆"])
    );
    assert_eq!(policy["items"][0]["source_role"], "user");
    assert_eq!(policy["items"][0]["source_index"], 1);
}

#[test]
fn rust_memory_policy_can_persist_to_sqlite_store() {
    let tmp = tempdir().unwrap();
    let memory_root = tmp.path().join(".codex/memory");
    let payload = json!({
        "workspace": "skill",
        "memory_root": memory_root,
        "persist": true,
        "messages": [
            {"role": "user", "content": "remember: persisted Rust memory policy row."}
        ],
        "limit": 8
    });
    let payload_text = payload.to_string();
    let output = common::run(router_rs_command([
        "framework",
        "memory-policy",
        "--input-json",
        &payload_text,
    ]));
    let result = json_from_output(&output);
    let persistence = &result["memory_policy"]["persistence"];
    assert_eq!(persistence["persisted"], true);
    assert_eq!(persistence["item_count"], 1);
    assert_eq!(
        persistence["stable_journal_path"],
        json!(memory_root.join("decisions.md").display().to_string())
    );

    let conn = Connection::open(tmp.path().join(".codex/memory/memory.sqlite3")).unwrap();
    let mut stmt = conn
        .prepare("SELECT workspace, category, source, summary, status FROM memory_items LIMIT 1")
        .unwrap();
    let row = stmt
        .query_row([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .unwrap();
    assert_eq!(row.0, "skill");
    assert_eq!(row.1, "fact");
    assert_eq!(row.2, "framework_memory_policy");
    assert_eq!(row.3, "persisted Rust memory policy row");
    assert_eq!(row.4, "active");

    let journal = fs::read_to_string(memory_root.join("decisions.md")).unwrap();
    assert!(journal.contains("## Rust memory policy facts"));
    assert!(journal.contains("- [fact] persisted Rust memory policy row"));
}

#[test]
fn rust_memory_policy_can_skip_stable_journal_when_requested() {
    let tmp = tempdir().unwrap();
    let memory_root = tmp.path().join(".codex/memory");
    let payload = json!({
        "workspace": "skill",
        "memory_root": memory_root,
        "persist": true,
        "stable_journal": false,
        "messages": [
            {"role": "user", "content": "remember: sqlite-only Rust memory policy row."}
        ],
        "limit": 8
    });
    let payload_text = payload.to_string();
    let output = common::run(router_rs_command([
        "framework",
        "memory-policy",
        "--input-json",
        &payload_text,
    ]));
    let result = json_from_output(&output);
    let persistence = &result["memory_policy"]["persistence"];
    assert_eq!(persistence["persisted"], true);
    assert_eq!(persistence["stable_journal_path"], json!(null));
    assert!(!memory_root.join("decisions.md").exists());
}

#[test]
fn rust_prompt_compression_policy_owns_prompt_wording() {
    let prompt = (1..=20)
        .map(|index| format!("line {index} content that makes the prompt long"))
        .collect::<Vec<_>>()
        .join("\n");
    let payload = json!({"prompt": prompt, "token_budget": 64});
    let payload_text = payload.to_string();
    let output = common::run(router_rs_command([
        "framework",
        "prompt-compression",
        "--input-json",
        &payload_text,
    ]));
    let result = json_from_output(&output);
    let compression = &result["compression"];
    assert_eq!(result["authority"], "rust-framework-prompt-policy");
    assert_eq!(compression["prompt_policy_owner"], "rust");
    assert_eq!(compression["strategy"], "structured_head_tail");
    assert_eq!(compression["truncated"], true);
    assert!(compression["compressed_prompt"]
        .as_str()
        .unwrap()
        .contains("[omitted 15 middle lines]"));
    assert_eq!(compression["artifact_offload_decision"], false);
}

fn render_context(
    repo_root: &std::path::Path,
    topic: &str,
    mode: &str,
    top: usize,
) -> serde_json::Value {
    let top_value = top.to_string();
    let output = common::run(router_rs_command([
        "framework",
        "memory-recall",
        topic,
        "--mode",
        mode,
        "--limit",
        &top_value,
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]));
    json_from_output(&output)["memory_recall"]["prompt_payload"]["retrieval"].clone()
}

fn seed_runtime(repo_root: &std::path::Path, task: &str) {
    let task_id = "active-bootstrap-repair-20260418210000";
    let task_root = repo_root.join("artifacts/current").join(task_id);
    write_text(
        &task_root.join("SESSION_SUMMARY.md"),
        &format!("- task: {task}\n- phase: implementation\n- status: in_progress\n"),
    );
    write_json(
        &task_root.join("NEXT_ACTIONS.json"),
        &json!({"next_actions": ["Patch classifier", "Run pytest"]}),
    );
    write_json(
        &task_root.join("EVIDENCE_INDEX.json"),
        &json!({"artifacts": []}),
    );
    write_json(
        &task_root.join("TRACE_METADATA.json"),
        &json!({"task": task, "matched_skills": ["execution-controller-coding"]}),
    );
    write_json(
        &repo_root.join("artifacts/current/active_task.json"),
        &json!({
            "task_id": task_id,
            "task": task,
            "task_root": task_root.display().to_string(),
            "session_summary": task_root.join("SESSION_SUMMARY.md").display().to_string(),
            "next_actions": task_root.join("NEXT_ACTIONS.json").display().to_string(),
            "evidence_index": task_root.join("EVIDENCE_INDEX.json").display().to_string(),
            "trace_metadata": task_root.join("TRACE_METADATA.json").display().to_string()
        }),
    );
    write_json(
        &repo_root.join(".supervisor_state.json"),
        &json!({
            "task_id": task_id,
            "task_summary": task,
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {
                "story_state": "active",
                "resume_allowed": true,
                "last_updated_at": "2026-04-18T22:49:57+08:00"
            },
            "blockers": {"open_blockers": ["Need regression coverage"]}
        }),
    );
}

fn seed_stable_memory(repo_root: &std::path::Path) {
    write_text(
        &repo_root.join(".codex/memory/MEMORY.md"),
        "# 项目长期记忆\n\n## Active Patterns\n\n- AP-1: Stable only by default\n",
    );
    write_text(
        &repo_root.join(".codex/memory/preferences.md"),
        "# preferences\n\n- prefer compact recall\n",
    );
}

fn seed_sqlite_memory(repo_root: &std::path::Path) {
    insert_memory_item(
        repo_root,
        "sqlite-item-1",
        "general",
        "sqlite",
        "sqlite-only row",
        "diagnostic row",
        &[],
    );
}

fn insert_memory_item(
    repo_root: &std::path::Path,
    item_id: &str,
    category: &str,
    source: &str,
    summary: &str,
    notes: &str,
    keywords: &[&str],
) {
    let db_path = repo_root.join(".codex/memory/memory.sqlite3");
    std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    let conn = Connection::open(db_path).unwrap();
    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_items (
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
        )",
        [],
    )
    .unwrap();
    let updated_at = "2026-04-18T22:49:57+08:00";
    conn.execute(
        "INSERT OR REPLACE INTO memory_items (
            item_id, workspace, category, source, confidence, status, summary, notes,
            evidence_json, metadata_json, keywords_json, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        (
            item_id,
            repo_root.file_name().unwrap().to_string_lossy().as_ref(),
            category,
            source,
            0.8,
            "active",
            summary,
            notes,
            "[]",
            "{}",
            serde_json::to_string(keywords).unwrap(),
            updated_at,
            updated_at,
        ),
    )
    .unwrap();
}

fn items(result: &serde_json::Value) -> Vec<serde_json::Value> {
    result["items"].as_array().unwrap().clone()
}

fn canonical_value_path(value: &serde_json::Value) -> String {
    fs::canonicalize(value.as_str().unwrap())
        .unwrap()
        .display()
        .to_string()
}
