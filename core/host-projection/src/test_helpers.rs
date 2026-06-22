//! Test-only helpers for host-projection hooks: mock registrations, env lock, tokenizer, etc.
//!
//! Extracted from `hooks.rs` to keep production code focused.

use crate::hooks;
use serde_json::Value;
use std::sync::OnceLock;

// ── synthetic_post_tool (test-only OnceLock slot) ──

static SYNTHETIC_POST_TOOL: OnceLock<fn(&Value) -> Value> = OnceLock::new();

pub fn register_hook_posttool_normalize(f: fn(&Value) -> Value) {
    hooks::once_lock_set(&SYNTHETIC_POST_TOOL, f, "SYNTHETIC_POST_TOOL");
}

pub fn synthetic_post_tool_evidence_shape(event: &Value) -> Value {
    SYNTHETIC_POST_TOOL
        .get()
        .map(|f| f(event))
        .unwrap_or_else(|| serde_json::json!({}))
}

// ── harness_operator_nudges: test-only env lock ──

pub(crate) fn harness_nudges_env_test_lock() -> std::sync::MutexGuard<'static, ()> {
    core_policy::test_env_sync::harness_nudges_env_test_lock()
}

// ── Test bootstrap: install tokenizer + review context probes ──

/// Install a simple whitespace tokenizer and no-op review context probes so that
/// `core_policy::hook_common` functions work in host-projection tests.
/// This replaces the `kernel_bootstrap::ensure_kernel_bootstrap()` call that
/// runtime-core tests rely on.
pub(crate) fn install_test_deps() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        struct WhitespaceTokenizer;
        impl framework_kernel::TokenizerProvider for WhitespaceTokenizer {
            fn tokenize_query(&self, text: &str) -> Vec<String> {
                text.split_whitespace()
                    .map(|s| s.to_ascii_lowercase())
                    .collect()
            }
            fn has_parallel_review_candidate_context(
                &self,
                _query: &str,
                _tokens: &[String],
            ) -> bool {
                false
            }
        }
        framework_kernel::install_tokenizer_provider(Box::new(WhitespaceTokenizer));
        // Install no-op review context probes (the test version in core-policy is cfg(test)-only).
        core_policy::review_context_signals::install_review_context_probes(
            |_text, _tokens| false,
            |_text, _tokens| false,
        );

        // Register test-only framework runtime hooks so cursor/codex/claude hooks tests
        // can exercise closeout enforcement, record path resolution, etc. without
        // depending on the real runtime-core registration.
        fn test_closeout_enforcement_enabled() -> bool {
            std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
        }
        fn test_closeout_record_path(
            repo_root: &std::path::Path,
            task_id: &str,
        ) -> Result<std::path::PathBuf, String> {
            Ok(repo_root.join("artifacts/closeout").join(format!("{task_id}.json")))
        }
        fn test_evaluate_closeout_record(
            _repo_root: &std::path::Path,
            _task_id: &str,
            record_path: &std::path::Path,
        ) -> Result<Value, String> {
            let data = std::fs::read_to_string(record_path)
                .map_err(|e| format!("read closeout record: {e}"))?;
            let val: Value =
                serde_json::from_str(&data).map_err(|e| format!("parse closeout record: {e}"))?;
            // Simplified evaluation: allow if record has valid schema_version,
            // verification_status == "passed", and commands_run is non-empty.
            let schema_ok = val
                .get("schema_version")
                .and_then(Value::as_str)
                .map(|s| s == "closeout-record-v1")
                .unwrap_or(false);
            let verification_passed = val
                .get("verification_status")
                .and_then(Value::as_str)
                .map(|s| s == "passed")
                .unwrap_or(false);
            let has_commands = val
                .get("commands_run")
                .and_then(Value::as_array)
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            let allowed = schema_ok && verification_passed && has_commands;
            Ok(serde_json::json!({
                "schema_version": "closeout-enforcement-response-v1",
                "authority": "closeout-enforcement",
                "task_id": val.get("task_id").and_then(Value::as_str).unwrap_or(""),
                "closeout_allowed": allowed,
                "claimed_completion": true,
                "violations": [],
                "missing_evidence": [],
                "verification_status": val.get("verification_status").and_then(Value::as_str).unwrap_or(""),
            }))
        }
        fn test_first_task_id_from_registry(repo_root: &std::path::Path) -> Option<String> {
            let reg_path = repo_root.join("artifacts/current/task_registry.json");
            let data = std::fs::read_to_string(reg_path).ok()?;
            let reg: Value = serde_json::from_str(&data).ok()?;
            reg.get("focus_task_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        }
        fn test_build_contract(_repo_root: &std::path::Path) -> Result<Value, String> {
            Ok(serde_json::json!({}))
        }
        fn test_append_shell(
            _repo_root: &std::path::Path,
            _event: &Value,
            _kind: &str,
        ) -> Result<(), String> {
            Ok(())
        }
        fn test_evidence_append(payload: Value) -> Result<Value, String> {
            Ok(payload)
        }
        fn test_extract_duration(_event: &Value) -> Option<u64> {
            None
        }
        fn test_post_tool_ok(_event: &Value) -> bool {
            true
        }
        fn test_closeout_followup(
            repo_root: &std::path::Path,
            text: &str,
        ) -> Option<String> {
            if text.trim().is_empty() || !core_policy::hook_common::contains_completion_claim_token(text) {
                return None;
            }
            if !test_closeout_enforcement_enabled() {
                return None;
            }
            // Resolve task ID from task_registry.json.
            let tid = test_first_task_id_from_registry(repo_root)?;
            let record_path = test_closeout_record_path(repo_root, &tid).ok()?;
            if !record_path.is_file() {
                return Some(format!(
                    "CLOSEOUT_FOLLOWUP task_id={tid} reason=missing_record path={}\n\
请在完成态宣称前写入 closeout record 并通过评估。",
                    record_path.display()
                ));
            }
            let eval = test_evaluate_closeout_record(repo_root, &tid, &record_path).ok()?;
            if eval
                .get("closeout_allowed")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                return None;
            }
            Some(format!(
                "CLOSEOUT_FOLLOWUP task_id={tid} reason=evaluation_failed path={}",
                record_path.display()
            ))
        }
        hooks::register_framework_runtime(
            test_build_contract,
            test_append_shell,
            test_closeout_enforcement_enabled,
            test_closeout_record_path,
            test_evaluate_closeout_record,
            test_first_task_id_from_registry,
            test_evidence_append,
            test_extract_duration,
            test_post_tool_ok,
            test_closeout_followup,
        );

        // Register outbound truncation / protection hooks for tests.
        fn test_is_protected_line(line: &str) -> bool {
            let t = line.trim_start();
            t.contains("router-rs REVIEW_GATE")
                || t.starts_with("router-rs REVIEW_GATE detail")
                || t.contains("continuity_suppressed=")
                || t.contains("PAPER_PROSE_QUALITY_HOOK")
                || t.contains("PAPER_ADVERSARIAL_HOOK")
        }
        fn test_truncate_outbound(
            combined: &str,
            max_bytes: usize,
            suffix: &str,
        ) -> String {
            if combined.len() <= max_bytes {
                return combined.to_string();
            }
            let budget = max_bytes.saturating_sub(suffix.len());
            let mut end = budget.min(combined.len());
            if let Some(nl) = combined[..end].rfind('\n') {
                end = nl;
            }
            while end > 0 && !combined.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}{}", &combined[..end], suffix)
        }
        hooks::register_hook_outbound_protect(test_is_protected_line, test_truncate_outbound);

        // Register ship readiness hooks (simplified for tests).
        fn test_evaluate_goal_readiness(
            repo_root: &std::path::Path,
            goal: &Value,
            task_id: &str,
        ) -> hooks::GoalReadiness {
            // When the caller passes Value::Null, read GOAL_STATE.json from disk
            let goal = if goal.is_null() && !task_id.is_empty() {
                let goal_path = repo_root.join("artifacts/current").join(task_id).join("GOAL_STATE.json");
                std::fs::read_to_string(&goal_path)
                    .ok()
                    .and_then(|content| serde_json::from_str(&content).ok())
                    .unwrap_or_else(|| goal.clone())
            } else {
                goal.clone()
            };
            let has_goal_text = goal
                .get("goal")
                .and_then(Value::as_str)
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            let has_non_goals = goal
                .get("non_goals")
                .and_then(Value::as_array)
                .map(|a| a.iter().any(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)))
                .unwrap_or(false);
            let has_validation = goal
                .get("validation_commands")
                .and_then(Value::as_array)
                .map(|a| a.iter().any(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)))
                .unwrap_or(false);
            let done_when_count = goal
                .get("done_when")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)).count())
                .unwrap_or(0);
            let contract = has_goal_text && has_non_goals && has_validation && done_when_count >= 2;
            let has_checkpoints = goal
                .get("checkpoints")
                .and_then(Value::as_array)
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            let evidence_path = repo_root
                .join("artifacts/current")
                .join(task_id)
                .join("EVIDENCE_INDEX.json");
            let has_evidence = evidence_path.is_file();
            let status = goal
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("");
            let progress = has_checkpoints || has_evidence || status == "completed";
            let verification = has_evidence || status == "completed"
                || (has_checkpoints && status == "running");
            hooks::GoalReadiness { contract, progress, verification }
        }
        fn test_goal_stop_followup(
            contract: bool,
            progress: bool,
            verification: bool,
            goal_followup_count: u32,
        ) -> String {
            let mut missing = Vec::new();
            if !contract { missing.push("goal_contract"); }
            if !progress { missing.push("checkpoint_progress"); }
            if !verification { missing.push("verification_or_blocker"); }
            let joined = missing.join(",");
            let mut line = format!("router-rs AG_FOLLOWUP missing_parts={joined}");
            if !contract {
                line.push_str(" primary_fix=goal_contract");
            } else if !progress {
                line.push_str(" primary_fix=checkpoint_progress");
            } else if !verification {
                line.push_str(" primary_fix=verification_or_blocker");
            }
            if goal_followup_count >= 3 {
                line.push_str(" | 已连续多轮 Stop 未满足门控；若确为小任务请直接单独一行 small_task");
            }
            line
        }
        hooks::register_ship_readiness(test_evaluate_goal_readiness, test_goal_stop_followup);

        // Register paper hooks with actual content injection for tests.
        // The builtin PAPER_PROSE_QUALITY_HOOK text is included at compile time.
        const PAPER_PROSE_BUILTIN: &str =
            include_str!("../../../configs/framework/PAPER_PROSE_QUALITY_HOOK.txt");

        fn prompt_signals_prose_work(text: &str) -> bool {
            // Simplified keyword detection for prose/edit signals.
            // Avoid false positives on programming terms like "abstract class".
            let lower = text.to_lowercase();
            let keywords = [
                "润色", "改稿", "论文", "latex", "manuscript",
                "proofread", "polish", "rewrite", "段落", "不通顺", "改这段",
                "sci", "prose", "写作", "初稿", "终稿", "提纲",
            ];
            if keywords.iter().any(|kw| lower.contains(kw)) {
                return true;
            }
            // "abstract" only triggers in prose context (with SCI/paper keywords).
            // "edit" alone is too generic (code edits).
            false
        }

        fn test_append_prose_context(
            _repo_root: &std::path::Path,
            prompt_text: &str,
            contexts: &mut Vec<String>,
            host: &'static str,
        ) {
            if !hooks::router_rs_operator_inject_globally_enabled() {
                return;
            }
            let env_var = match host {
                "cursor" => "ROUTER_RS_CURSOR_PAPER_PROSE_HOOK",
                "codex" => "ROUTER_RS_CODEX_PAPER_PROSE_HOOK",
                "claude" => "ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK",
                "opencode" => "ROUTER_RS_OPENCODE_PAPER_PROSE_HOOK",
                _ => return,
            };
            if !hooks::router_rs_env_enabled_default_true(env_var) {
                return;
            }
            if !prompt_signals_prose_work(prompt_text) {
                return;
            }
            let block = PAPER_PROSE_BUILTIN.trim().to_string();
            if !block.is_empty() {
                contexts.push(block);
            }
        }

        fn test_merge_prose_before_submit(
            _repo_root: &std::path::Path,
            output: &mut Value,
            prompt_text: &str,
            use_followup_message: bool,
            _host: &'static str,
        ) {
            if !hooks::router_rs_operator_inject_globally_enabled() {
                return;
            }
            if !hooks::router_rs_env_enabled_default_true("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK") {
                return;
            }
            if !prompt_signals_prose_work(prompt_text) {
                return;
            }
            let block = PAPER_PROSE_BUILTIN.trim().to_string();
            if block.is_empty() {
                return;
            }
            let key = if use_followup_message {
                "followup_message"
            } else {
                "additional_context"
            };
            let existing = output
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if existing.contains("PAPER_PROSE_QUALITY_HOOK") {
                return;
            }
            let merged = if existing.is_empty() {
                block
            } else {
                format!("{existing}\n\n{block}")
            };
            output[key] = Value::String(merged);
        }

        fn test_append_adversarial_context(
            _repo_root: &std::path::Path,
            _prompt_text: &str,
            _contexts: &mut Vec<String>,
            _host: &'static str,
        ) {
            // No-op for tests.
        }

        fn test_merge_adversarial_before_submit(
            _repo_root: &std::path::Path,
            _output: &mut Value,
            _prompt_text: &str,
            _use_followup_message: bool,
            _host: &'static str,
        ) {
            // No-op for tests.
        }

        hooks::register_paper_hooks(
            test_append_prose_context,
            test_merge_prose_before_submit,
            test_append_adversarial_context,
            test_merge_adversarial_before_submit,
        );

        // Register session call tracker hooks for tests.
        fn test_init_tracker(repo_root: &std::path::Path) -> Result<(), String> {
            let dir = repo_root.join("artifacts/current");
            std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
            let path = dir.join("session_call_tracker.json");
            let state = serde_json::json!({
                "schema_version": "session-call-tracker-v1",
                "total_calls": 0,
                "per_tool": {},
            });
            std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap())
                .map_err(|e| format!("write tracker: {e}"))?;
            Ok(())
        }
        fn test_record_tool_call(
            repo_root: &std::path::Path,
            tool_name: &str,
            _cache_stats: Option<&Value>,
        ) -> Result<(), String> {
            let path = repo_root.join("artifacts/current/session_call_tracker.json");
            let data = std::fs::read_to_string(&path).map_err(|e| format!("read tracker: {e}"))?;
            let mut state: Value =
                serde_json::from_str(&data).map_err(|e| format!("parse tracker: {e}"))?;
            let total = state["total_calls"].as_u64().unwrap_or(0) + 1;
            state["total_calls"] = serde_json::json!(total);
            let tool_key = tool_name.to_string();
            let per_tool = state["per_tool"].as_object_mut().unwrap();
            let count = per_tool.get(&tool_key).and_then(Value::as_u64).unwrap_or(0) + 1;
            per_tool.insert(tool_key, serde_json::json!(count));
            std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap())
                .map_err(|e| format!("write tracker: {e}"))?;
            Ok(())
        }
        fn test_read_tracker_state(repo_root: &std::path::Path) -> Result<Value, String> {
            let path = repo_root.join("artifacts/current/session_call_tracker.json");
            let data = std::fs::read_to_string(&path).map_err(|e| format!("read tracker: {e}"))?;
            serde_json::from_str(&data).map_err(|e| format!("parse tracker: {e}"))
        }
        hooks::register_session_call_tracker(test_init_tracker, test_record_tool_call, test_read_tracker_state);

        // Register router_rs_observation hooks for tests.
        fn test_attach_observation(output: &mut Value, host: &'static str) {
            let host_str = match host {
                "cursor" | "codex" | "claude" | "opencode" => host,
                _ => "unknown",
            };
            // Detect review gate advisory in followup_message.
            let followup = output
                .get("followup_message")
                .and_then(Value::as_str)
                .unwrap_or("");
            let decision = output.get("decision").and_then(Value::as_str);
            let gate = if followup.contains("REVIEW_GATE") {
                Some(serde_json::json!({
                    "code": "review_gate",
                    "blocking": true,
                    "human_prefix": "review_gate",
                }))
            } else if decision == Some("block") {
                Some(serde_json::json!({
                    "code": "block",
                    "blocking": true,
                    "human_prefix": "block",
                }))
            } else {
                None
            };
            let mut obs = serde_json::json!({
                "host": host_str,
            });
            if let Some(g) = gate {
                obs["gate"] = g;
            }
            if let Some(obj) = output.as_object_mut() {
                obj.insert("router_rs_observation".to_string(), obs);
            }
        }
        fn test_strip_observation(output: &mut Value) {
            if let Some(obj) = output.as_object_mut() {
                obj.remove("router_rs_observation");
            }
        }
        hooks::register_router_rs_observation(test_attach_observation, test_strip_observation);
    });
}
