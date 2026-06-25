//! Test-only helpers for host-projection hooks: mock registrations, env lock, tokenizer, etc.
//!
//! Extracted from `hooks.rs` to keep production code focused.

use crate::hooks;
use core_policy::error::FrameworkError;
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
        ) -> Result<std::path::PathBuf, FrameworkError> {
            Ok(repo_root.join("artifacts/closeout").join(format!("{task_id}.json")))
        }
        fn test_evaluate_closeout_record(
            _repo_root: &std::path::Path,
            _task_id: &str,
            record_path: &std::path::Path,
        ) -> Result<Value, FrameworkError> {
            let data = std::fs::read_to_string(record_path)?;
            let val: Value = serde_json::from_str(&data)?;
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
        fn test_build_contract(_repo_root: &std::path::Path) -> Result<Value, FrameworkError> {
            Ok(serde_json::json!({}))
        }
        fn test_append_shell(
            _repo_root: &std::path::Path,
            _event: &Value,
            _kind: &str,
        ) -> Result<(), FrameworkError> {
            Ok(())
        }
        fn test_evidence_append(payload: Value) -> Result<Value, FrameworkError> {
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
            if !core_policy::env_flags::router_rs_operator_inject_globally_enabled() {
                return;
            }
            let env_var = framework_kernel::runtime_registry::paper_prose_env(host);
            if env_var.is_empty() {
                return;
            }
            if !core_policy::env_flags::env_enabled_default_true(env_var) {
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
            if !core_policy::env_flags::router_rs_operator_inject_globally_enabled() {
                return;
            }
            if !core_policy::env_flags::env_enabled_default_true("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK") {
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

        // Register research mode inference for tests (simplified version).
        hooks::register_research_mode_inference(|payload: &Value| {
            // 1. Explicit research_mode field
            if let Some(mode) = payload.get("research_mode").and_then(Value::as_str) {
                let m = mode.trim().to_ascii_lowercase();
                if m.contains("deep") || m.contains("深度") {
                    return "deep".to_string();
                }
                return "quick".to_string();
            }
            // 2. execution_protocol field
            if let Some(proto) = payload.get("execution_protocol").and_then(Value::as_str) {
                let p = proto.trim().to_ascii_lowercase();
                if p.contains("deep") || p.contains("深度") || p.contains("research") {
                    return "deep".to_string();
                }
            }
            // 3. Task text signals
            let task = payload.get("task").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
            if task.contains("deep dive") || task.contains("深度调研") || task.contains("深度研究")
                || task.contains("literature review") || task.contains("文献综述")
                || task.contains("external research")
            {
                return "deep".to_string();
            }
            // 4. Reasons
            if let Some(reasons) = payload.get("reasons").and_then(Value::as_array) {
                for r in reasons {
                    if let Some(s) = r.as_str() {
                        let low = s.to_ascii_lowercase();
                        if low.contains("deep") || low.contains("literature review") || low.contains("深度研究") {
                            return "deep".to_string();
                        }
                    }
                }
            }
            "quick".to_string()
        });

        // Register session call tracker hooks for tests.
        fn test_init_tracker(repo_root: &std::path::Path) -> Result<(), FrameworkError> {
            let dir = repo_root.join("artifacts/current");
            std::fs::create_dir_all(&dir)?;
            let path = dir.join("session_call_tracker.json");
            let state = serde_json::json!({
                "schema_version": "session-call-tracker-v1",
                "total_calls": 0,
                "per_tool": {},
            });
            std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap())?;
            Ok(())
        }
        fn test_record_tool_call(
            repo_root: &std::path::Path,
            tool_name: &str,
            _cache_stats: Option<&Value>,
        ) -> Result<(), FrameworkError> {
            let path = repo_root.join("artifacts/current/session_call_tracker.json");
            let data = std::fs::read_to_string(&path)?;
            let mut state: Value = serde_json::from_str(&data)?;
            let total = state["total_calls"].as_u64().unwrap_or(0) + 1;
            state["total_calls"] = serde_json::json!(total);
            let tool_key = tool_name.to_string();
            let per_tool = state["per_tool"].as_object_mut().unwrap();
            let count = per_tool.get(&tool_key).and_then(Value::as_u64).unwrap_or(0) + 1;
            per_tool.insert(tool_key, serde_json::json!(count));
            std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap())?;
            Ok(())
        }
        fn test_read_tracker_state(repo_root: &std::path::Path) -> Result<Value, FrameworkError> {
            let path = repo_root.join("artifacts/current/session_call_tracker.json");
            let data = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&data)?)
        }
        hooks::register_session_call_tracker(test_init_tracker, test_record_tool_call, test_read_tracker_state);
    });
}
