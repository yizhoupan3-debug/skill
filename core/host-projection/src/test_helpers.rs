//! Test-only helpers for host-projection hooks: mock registrations, env lock, tokenizer, etc.
//!
//! Extracted from `hooks.rs` to keep production code focused.

use crate::hooks;
use core_errors::FrameworkError;
use serde_json::Value;
use std::sync::OnceLock;

// ── synthetic_post_tool (test-only OnceLock slot) ──

static SYNTHETIC_POST_TOOL: OnceLock<fn(&Value) -> Value> = OnceLock::new();

pub fn register_hook_posttool_normalize(f: fn(&Value) -> Value) {
    SYNTHETIC_POST_TOOL.set(f).unwrap_or_else(|_| {
        tracing::warn!("SYNTHETIC_POST_TOOL already registered — second call ignored");
    });
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
        fn test_closeout_enabled() -> bool {
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
        #[allow(dead_code)]
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
            if !test_closeout_enabled() {
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
        // Set RuntimeHooks directly (replaces old register_framework_runtime).
        // Paper hooks and research mode inference use individual OnceLock below (external override pattern).
        hooks::set_runtime_hooks(hooks::RuntimeHooks {
            // framework_runtime
            closeout_record_path_for_task: test_closeout_record_path,
            evaluate_closeout_record_file_for_task: test_evaluate_closeout_record,
            extract_post_tool_duration_ms: test_extract_duration,
            post_tool_call_succeeded: test_post_tool_ok,
            closeout_stop_followup_for_completion_text: test_closeout_followup,
            // paper hooks (defaults — individual OnceLock has priority)
            maybe_append_paper_prose_context: |_, _, _, _| {},
            maybe_merge_paper_prose_before_submit: |_, _, _, _, _| {},
            maybe_append_paper_adversarial_context: |_, _, _, _| {},
            maybe_merge_paper_adversarial_before_submit: |_, _, _, _, _| {},
            // research activity
            maybe_record_research_activity: |_, _, _| {},
            // kernel bootstrap
            ensure_kernel_bootstrap: || {},
            // framework_runtime_extra
            current_local_timestamp: || "1970-01-01T00:00:00Z".into(),
            write_framework_session_artifacts: |_| Err(FrameworkError::validation("not registered in test")),
            route_task_with_manifest_fallback: |_, _, _, _, _, _| Err(FrameworkError::validation("not registered in test")),
            build_automatic_continuity_checkpoint_payload: |_, _, _, _, _, _| Value::Null,
            append_evidence_index: |_, _, _| Err(FrameworkError::validation("not registered in test")),
            closeout_record_schema_version: || "closeout-record-v1",
            // web_fetch_guard
            validate_and_resolve_web_fetch_url: |_| Err(FrameworkError::validation("not registered in test")),
            resolve_web_fetch_redirect: |_, _| Err(FrameworkError::validation("not registered in test")),
            resolve_web_fetch_addresses: |_, _| Err(FrameworkError::validation("not registered in test")),
            // mcp_pre_guard
            evaluate_mcp_pre_guard_safe: |_, _, _| hooks::McpPreGuardVerdict { blocked: false, reason: None },
            // research_tool_dispatch
            research_tool_dispatch: |_, _| Err(FrameworkError::validation("not registered in test")),
            // mcp_tool_routing
            mcp_tool_skill_route: |_, _, _, _| Err(FrameworkError::validation("not registered in test")),
            mcp_tool_search_skills: |_, _, _, _| Err(FrameworkError::validation("not registered in test")),
            // tool_dispatch
            tool_goal_state_manage_dispatch: |_, _, _| Err(FrameworkError::validation("not registered in test")),
            tool_closeout_record_write_dispatch: |_, _| Err(FrameworkError::validation("not registered in test")),
            tool_closeout_gate_evaluate: |_, _, _| Err(FrameworkError::validation("not registered in test")),
            // browser_dispatch
            browser_dispatch: |_| Err(FrameworkError::validation("not registered in test")),
            // runtime_trace_transport
            attach_runtime_event_transport: |_| Err(FrameworkError::validation("not registered in test")),
            inspect_trace_stream: |_| Err(FrameworkError::validation("not registered in test")),
        });

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

        hooks::modify_runtime_hooks(|hooks| {
            hooks.maybe_append_paper_prose_context = test_append_prose_context;
            hooks.maybe_merge_paper_prose_before_submit = test_merge_prose_before_submit;
            hooks.maybe_append_paper_adversarial_context = test_append_adversarial_context;
            hooks.maybe_merge_paper_adversarial_before_submit = test_merge_adversarial_before_submit;
        });

    });
}
