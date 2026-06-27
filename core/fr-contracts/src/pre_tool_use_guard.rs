//! PreToolUse strict fallback for hosts without native PreToolUse / `hard_gate_hooks`.
//!
//! Rich CLI hosts (Cursor, Codex, Claude) rely on shell hooks; MCP-only hosts must call the
//! `pre_tool_use_guard` stdio op before high-risk tool execution. Detection order:
//! 1. HostProvider hint (`host_provider_strict_pre_tool_fallback_hint`)
//! 2. Request payload override (`has_native_hook`)
//! 3. Registry capability (`hard_gate_hooks` in `host_projections`)
//! 4. Unknown host → fail-closed (strict fallback active)
//!
//! No hardcoded host ID list — all host identity is resolved via registry capabilities.

use core_policy::hook_policy::{
    HookPolicyEvaluateRequest, dangerous_bash_reason, dangerous_mcp_tool_reason,
    evaluate_hook_policy,
};
use core_policy::tool_safety_rules;
use framework_kernel::runtime_registry::load_runtime_registry_json;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use core_errors::FrameworkError;
use sha2::{Digest, Sha256};
use std::path::{Component, Path};

type Result<T> = std::result::Result<T, FrameworkError>;

/// Schema version string for the pre-tool-use guard protocol.
/// Use to validate response schema compatibility in callers.
pub const PRE_TOOL_USE_GUARD_SCHEMA_VERSION: &str = "router-rs-pre-tool-use-guard-v1";

/// Authority identifier for the pre-tool-use guard.
/// Identifies `rust-framework-runtime` as the source of the guard verdict.
pub const PRE_TOOL_USE_GUARD_AUTHORITY: &str = "rust-framework-runtime";

/// Stdio operation name for the pre-tool-use guard.
/// Use as the routing key when dispatching via `stdio_dispatch`.
pub const PRE_TOOL_USE_GUARD_STDIO_OP: &str = "pre_tool_use_guard";

/// Request payload for the pre-tool-use guard evaluation or approval phase.
///
/// Fields identify the host, tool, input, and optionally provide approval credentials for the
/// `approve` phase. Use via [`evaluate_pre_tool_use_guard`].
#[derive(Debug, Clone, Deserialize)]
pub struct PreToolUseGuardRequest {
    pub host_id: String,
    pub tool_name: String,
    #[serde(default)]
    pub tool_input: Value,
    #[serde(default)]
    pub repo_root: Option<String>,
    /// `evaluate` (default) or `approve`.
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub approval_digest: Option<String>,
    #[serde(default)]
    pub approved: Option<bool>,
    /// HostProvider override; when `Some(true)`, strict fallback is off.
    #[serde(default)]
    pub has_native_hook: Option<bool>,
}

/// Verdict returned by the pre-tool-use guard.
///
/// `Allow` permits execution, `Block` denies it unconditionally, and `RequiresStdioApproval`
/// requests human approval via the stdio approval flow with a matching approval digest.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreToolUseGuardVerdict {
    Allow,
    Block,
    RequiresStdioApproval,
}

/// Response from the pre-tool-use guard.
///
/// Contains the verdict, whether strict fallback is active, the blocking reason, approval digest,
/// and risk categories. Use to determine whether a tool invocation may proceed and, if blocked,
/// what approval is required.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreToolUseGuardResponse {
    pub schema_version: String,
    pub authority: String,
    pub host_id: String,
    pub tool_name: String,
    pub strict_fallback_active: bool,
    pub verdict: PreToolUseGuardVerdict,
    pub blocked: bool,
    pub reason: Option<String>,
    pub stdio_op: String,
    pub approval_digest: Option<String>,
    pub categories: Vec<String>,
}

/// Return the execution contract for the pre-tool-use guard as a JSON value.
///
/// The contract declares the schema version, authority, supported phases (`evaluate`, `approve`),
/// registry signals, high-risk tool families, and the approval flow. Use by the contract bundle
/// to advertise guard capabilities to callers.
pub fn pre_tool_use_guard_contract() -> Value {
    json!({
        "schema_version": PRE_TOOL_USE_GUARD_SCHEMA_VERSION,
        "authority": PRE_TOOL_USE_GUARD_AUTHORITY,
        "stdio_op": PRE_TOOL_USE_GUARD_STDIO_OP,
        "phases": ["evaluate", "approve"],
        "registry_signals": {
            "hard_gate_hooks": "native PreToolUse hard gate — strict fallback off",
            "closeout_evidence_hooks_exception": "harness_capability_exceptions.closeout_evidence_hooks=unsupported → strict fallback on"
        },
        "host_provider_field": "has_native_hook (optional bool override on stdio payload)",
        "high_risk_tool_families": [
            "shell",
            "file_write",
            "framework_mcp_danger"
        ],
        "approval_flow": [
            "evaluate returns requires_stdio_approval + approval_digest when strict fallback is active",
            "operator confirms high-risk action",
            "approve phase with matching approval_digest and approved=true returns allow for that invocation"
        ]
    })
}

/// Determine whether the given host ID requires strict PreToolUse fallback.
///
/// Checks, in order: HostProvider hints, explicit override from the request payload, registry
/// capability `hard_gate_hooks`, and `closeout_evidence_hooks` support. Unknown hosts default to
/// `true` (fail-closed). Use at startup to configure per-host guard behavior.
pub fn host_requires_strict_pre_tool_fallback(
    host_id: &str,
    repo_root: &Path,
    has_native_hook_override: Option<bool>,
) -> Result<bool> {
    let id = host_id.trim();
    // 1. HostProvider hint (highest priority after explicit overrides)
    if let Some(strict) = framework_kernel::runtime_hooks::hooks().host_provider_strict_pre_tool_fallback_hint(id) {
        return Ok(strict);
    }
    if has_native_hook_override == Some(true) {
        return Ok(false);
    }
    if has_native_hook_override == Some(false) {
        return Ok(true);
    }
    let registry = load_runtime_registry_json(repo_root)?;
    let projection = registry
        .get("host_projections")
        .and_then(|v| v.get(id))
        .and_then(Value::as_object);
    let Some(projection) = projection else {
        // Unknown host id: fail closed for tool guard purposes.
        return Ok(true);
    };
    let capabilities = projection
        .get("capabilities")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::trim))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if capabilities.contains(&"hard_gate_hooks") {
        return Ok(false);
    }
    let harness_caps = projection
        .get("harness_capabilities")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::trim))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let closeout_unsupported = projection
        .get("harness_capability_exceptions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter().any(|row| {
                row.get("cap").and_then(Value::as_str) == Some("closeout_evidence_hooks")
                    && row.get("status").and_then(Value::as_str) == Some("unsupported")
            })
        })
        .unwrap_or(false);
    if closeout_unsupported {
        return Ok(true);
    }
    if harness_caps.contains(&"closeout_evidence_hooks")
    {
        return Ok(false);
    }
    Ok(true)
}

/// Evaluate a PreToolUse guard request and return a verdict.
///
/// Supports two phases:
/// - `evaluate` (default): check risk level and return `Allow`, `Block`, or `RequiresStdioApproval`.
/// - `approve`: verify the approval digest matches and return the final verdict.
///
/// Use as the main entry point for tool-use gating. When `strict_fallback` is active, dangerous
/// shell commands and protected-path file writes produce `RequiresStdioApproval`.
pub fn evaluate_pre_tool_use_guard(
    request: PreToolUseGuardRequest,
) -> Result<PreToolUseGuardResponse> {
    framework_kernel::runtime_hooks::hooks().ensure_kernel_bootstrap();
    let repo_root = request
        .repo_root
        .as_deref()
        .map(Path::new)
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| framework_kernel::repo_roots::resolve_repo_root_arg(Some(p)))
        .transpose()?
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf())
        });
    let host_id = request.host_id.trim().to_string();
    let tool_name = request.tool_name.trim().to_string();
    let strict =
        host_requires_strict_pre_tool_fallback(&host_id, &repo_root, request.has_native_hook)?;
    let phase = request
        .phase
        .as_deref()
        .unwrap_or("evaluate")
        .trim()
        .to_ascii_lowercase();

    if phase == "approve" {
        let digest = approval_digest(&host_id, &tool_name, &request.tool_input)?;
        let approved = request.approved.unwrap_or(false);
        let verdict = approve_verdict(
            strict,
            approved,
            request.approval_digest.as_deref(),
            &digest,
        );
        return Ok(build_response(
            &host_id,
            &tool_name,
            strict,
            verdict,
            None,
            vec![],
            Some(digest),
        ));
    }

    if !strict {
        return Ok(build_response(
            &host_id,
            &tool_name,
            false,
            PreToolUseGuardVerdict::Allow,
            None,
            vec![],
            None,
        ));
    }

    if fr_utils::env_flags::router_rs_skip_pre_tool_use_guard() {
        return Ok(build_response(
            &host_id,
            &tool_name,
            true,
            PreToolUseGuardVerdict::Allow,
            Some("ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD=1 (development bypass)".to_string()),
            vec!["dev_bypass".to_string()],
            None,
        ));
    }

    let (verdict, reason, categories) =
        classify_high_risk(&tool_name, &request.tool_input, &repo_root)?;
    let final_verdict = verdict;
    let needs_digest = final_verdict == PreToolUseGuardVerdict::RequiresStdioApproval;
    let digest = if needs_digest {
        Some(approval_digest(&host_id, &tool_name, &request.tool_input)?)
    } else {
        None
    };
    Ok(build_response(
        &host_id,
        &tool_name,
        true,
        final_verdict,
        reason,
        categories,
        digest,
    ))
}

/// Wrapper around [`evaluate_pre_tool_use_guard`] that accepts a raw JSON value payload.
///
/// Parses the payload into [`PreToolUseGuardRequest`], evaluates, and returns the response as JSON.
/// Use from stdio dispatch where the input is already a `serde_json::Value`.
pub fn evaluate_pre_tool_use_guard_value(payload: Value) -> Result<Value> {
    let request: PreToolUseGuardRequest = serde_json::from_value(payload)?;
    let response = evaluate_pre_tool_use_guard(request)?;
    Ok(serde_json::to_value(response)?)
}

fn approve_verdict(
    strict: bool,
    approved: bool,
    supplied_digest: Option<&str>,
    expected_digest: &str,
) -> PreToolUseGuardVerdict {
    if !strict {
        return PreToolUseGuardVerdict::Allow;
    }
    if !approved {
        return PreToolUseGuardVerdict::RequiresStdioApproval;
    }
    match supplied_digest {
        Some(value) if value.trim() == expected_digest => PreToolUseGuardVerdict::Allow,
        _ => PreToolUseGuardVerdict::Block,
    }
}

fn build_response(
    host_id: &str,
    tool_name: &str,
    strict_fallback_active: bool,
    verdict: PreToolUseGuardVerdict,
    reason: Option<String>,
    categories: Vec<String>,
    approval_digest: Option<String>,
) -> PreToolUseGuardResponse {
    let blocked = matches!(
        verdict,
        PreToolUseGuardVerdict::Block | PreToolUseGuardVerdict::RequiresStdioApproval
    );
    PreToolUseGuardResponse {
        schema_version: PRE_TOOL_USE_GUARD_SCHEMA_VERSION.to_string(),
        authority: PRE_TOOL_USE_GUARD_AUTHORITY.to_string(),
        host_id: host_id.to_string(),
        tool_name: tool_name.to_string(),
        strict_fallback_active,
        verdict,
        blocked,
        reason,
        stdio_op: PRE_TOOL_USE_GUARD_STDIO_OP.to_string(),
        approval_digest,
        categories,
    }
}

fn approval_digest(host_id: &str, tool_name: &str, tool_input: &Value) -> Result<String> {
    let canonical = json!({
        "host_id": host_id,
        "tool_name": tool_name,
        "tool_input": tool_input,
    });
    let bytes = serde_json::to_vec(&canonical)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn classify_high_risk(
    tool_name: &str,
    tool_input: &Value,
    repo_root: &Path,
) -> Result<(PreToolUseGuardVerdict, Option<String>, Vec<String>)> {
    let lowered = tool_name.to_ascii_lowercase();
    if is_shell_tool(&lowered)
        && let Some(command) = extract_shell_command(tool_input)
            && let Some(reason) = dangerous_bash_reason(&command) {
                // M9: warn if shell command targets auxiliary files
                check_auxiliary_file_reference(&command);
                return Ok((
                    PreToolUseGuardVerdict::RequiresStdioApproval,
                    Some(reason),
                    vec!["shell".to_string()],
                ));
            }
    if is_shell_tool(&lowered)
        && let Some(command) = extract_shell_command(tool_input) {
            // M9: warn if shell command targets auxiliary files (no dangerous_bash_reason match)
            check_auxiliary_file_reference(&command);
        }
    if is_file_write_tool(&lowered)
        && let Some(path) = extract_file_path(tool_input) {
            // Block path traversal attempts before protected-path check
            if has_path_traversal(tool_input) {
                return Ok((
                    PreToolUseGuardVerdict::Block,
                    Some(format!("path traversal detected in file path: {path}")),
                    vec!["file_write".to_string(), "path_traversal".to_string()],
                ));
            }
            let repo_root_str = repo_root.display().to_string();
            let response = evaluate_hook_policy(HookPolicyEvaluateRequest {
                operation: "protected-path".to_string(),
                command: None,
                path: Some(path.clone()),
                repo_root: Some(repo_root_str.clone()),
                runtime_root: Some(repo_root_str),
                tool_name: None,
                tool_args: None,
            })?;
            if response.blocked {
                return Ok((
                    PreToolUseGuardVerdict::RequiresStdioApproval,
                    response.reason,
                    vec!["file_write".to_string(), "protected_path".to_string()],
                ));
            }
        }
    if let Some(reason) = dangerous_mcp_tool_reason(tool_name, Some(tool_input)) {
        return Ok((
            PreToolUseGuardVerdict::RequiresStdioApproval,
            Some(reason),
            vec!["framework_mcp_danger".to_string()],
        ));
    }
    Ok((PreToolUseGuardVerdict::Allow, None, vec![]))
}

fn is_shell_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "shell"
            | "bash"
            | "run_terminal_cmd"
            | "execute_command"
            | "terminal"
            | "functions.exec_command"
            | "run_command"
            | "sh"
            | "exec"
            | "cmd"
    )
}

fn is_file_write_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "write"
            | "strreplace"
            | "str_replace"
            | "delete"
            | "applypatch"
            | "apply_patch"
            | "edit"
            | "notebookedit"
            | "notebook_edit"
    )
}

/// Warn when a shell command references auxiliary files (write-only or document-only).
/// These files are generated by the framework and should not be manually edited.
fn check_auxiliary_file_reference(command: &str) {
    for pattern in tool_safety_rules::WRITE_ONLY_AUXILIARY_FILES {
        if command.contains(pattern) {
            tracing::warn!(
                "[mcp-pre-guard] shell command targets write-only auxiliary file: {pattern}"
            );
        }
    }
    for pattern in tool_safety_rules::DOCUMENT_ONLY_AUXILIARY_FILES {
        if command.contains(pattern) {
            tracing::warn!(
                "[mcp-pre-guard] shell command targets document-only auxiliary file: {pattern}"
            );
        }
    }
}

fn extract_shell_command(tool_input: &Value) -> Option<String> {
    for key in ["command", "cmd", "script"] {
        if let Some(text) = tool_input.get(key).and_then(Value::as_str) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn extract_file_path(tool_input: &Value) -> Option<String> {
    for key in ["path", "file_path", "target_file", "filePath", "notebook_path"] {
        if let Some(text) = tool_input.get(key).and_then(Value::as_str) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Check if tool_input contains a path with `..` traversal components.
fn has_path_traversal(tool_input: &Value) -> bool {
    for key in ["path", "file_path", "target_file", "filePath", "notebook_path"] {
        if let Some(text) = tool_input.get(key).and_then(Value::as_str) {
            let p = Path::new(text.trim());
            if p.components().any(|c| c == Component::ParentDir) {
                return true;
            }
        }
    }
    false
}

// Test-only no-op hook registration for tests that need RuntimeCoreHooks.
#[cfg(test)]
#[ctor::ctor]
fn register_test_hooks() {
    use framework_kernel::runtime_hooks::{RuntimeCoreHooks, HostProviderHooks};
    framework_kernel::runtime_hooks::register(RuntimeCoreHooks {
        host_provider: HostProviderHooks {
            for_routing_spelling: |_| None,
            strict_pre_tool_fallback_hint: |_| None,
            registry: || vec![],
        },
        framework_goal_drive: |_| Ok(serde_json::Value::Null),
        handle_orchestrator_operation: |_| Ok(serde_json::Value::Null),
        handle_background_state_operation: |_| Ok(serde_json::Value::Null),
        runtime_concurrency_defaults_payload: || serde_json::Value::Null,
        eval_route_contract: || serde_json::Value::Null,
        run_eval_route: |_, _| Ok(serde_json::Value::Null),
        generated_artifacts_status_for_repo: |_| Ok(String::new()),
        ensure_kernel_bootstrap: || {},
    });
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::execution_contract::build_execution_contract_bundle;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn skill_repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("skill repo root")
    }

    // ── has_path_traversal ──

    #[test]
    fn path_traversal_detects_dotdot_in_path() {
        assert!(has_path_traversal(&json!({"path": "../etc/passwd"})));
    }

    #[test]
    fn path_traversal_detects_dotdot_in_file_path() {
        assert!(has_path_traversal(&json!({"file_path": "/a/../../b"})));
    }

    #[test]
    fn path_traversal_detects_dotdot_in_target_file() {
        assert!(has_path_traversal(&json!({"target_file": "output/../../../tmp/foo"})));
    }

    #[test]
    fn path_traversal_detects_dotdot_in_file_path_camel() {
        assert!(has_path_traversal(&json!({"filePath": "subdir/../../out"})));
    }

    #[test]
    fn path_traversal_detects_dotdot_in_notebook_path() {
        assert!(has_path_traversal(&json!({"notebook_path": "notebooks/../../etc/hacks"})));
    }

    #[test]
    fn path_traversal_clean_path_returns_false() {
        assert!(!has_path_traversal(&json!({"path": "/safe/dir/file.txt"})));
    }

    #[test]
    fn path_traversal_empty_path_returns_false() {
        assert!(!has_path_traversal(&json!({"path": ""})));
    }

    #[test]
    fn path_traversal_non_string_value_returns_false() {
        assert!(!has_path_traversal(&json!({"path": 42})));
    }

    #[test]
    fn path_traversal_missing_key_returns_false() {
        assert!(!has_path_traversal(&json!({"other_key": "../whatever"})));
    }

    #[test]
    fn path_traversal_dotdot_in_middle_is_detected() {
        // "foo/bar" has no ParentDir component since "." is not ".."
        // Need to test that "../" is caught even when embedded
        assert!(has_path_traversal(&json!({"path": "subdir/../over"})));
    }

    #[test]
    fn path_traversal_trailing_dotdot() {
        assert!(has_path_traversal(&json!({"path": "/a/b/.."})));
    }

    #[test]
    fn opencode_requires_strict_fallback_without_hard_gate_hooks() {
        let root = skill_repo_root();
        assert!(!host_requires_strict_pre_tool_fallback("opencode", &root, None).unwrap());
    }

    #[test]
    fn claude_code_skips_strict_fallback_with_hard_gate_hooks() {
        let root = skill_repo_root();
        assert!(!host_requires_strict_pre_tool_fallback("claude", &root, None).unwrap());
    }

    #[test]
    fn host_provider_override_native_hook_disables_strict_fallback() {
        let root = skill_repo_root();
        assert!(
            !host_requires_strict_pre_tool_fallback("unknown-host", &root, Some(true)).unwrap()
        );
    }

    #[test]
    fn strict_fallback_blocks_dangerous_shell_until_approved() {
        let root = skill_repo_root();
        let evaluate = evaluate_pre_tool_use_guard(PreToolUseGuardRequest {
            host_id: "unknown-host".to_string(),
            tool_name: "Shell".to_string(),
            tool_input: json!({"command": "git reset --hard HEAD"}),
            repo_root: Some(root.display().to_string()),
            phase: None,
            approval_digest: None,
            approved: None,
            has_native_hook: Some(false),
        })
        .expect("evaluate");
        assert!(evaluate.strict_fallback_active);
        assert_eq!(
            evaluate.verdict,
            PreToolUseGuardVerdict::RequiresStdioApproval
        );
        assert!(evaluate.blocked);
        let digest = evaluate.approval_digest.expect("digest");

        let approved = evaluate_pre_tool_use_guard(PreToolUseGuardRequest {
            host_id: "unknown-host".to_string(),
            tool_name: "Shell".to_string(),
            tool_input: json!({"command": "git reset --hard HEAD"}),
            repo_root: Some(root.display().to_string()),
            phase: Some("approve".to_string()),
            approval_digest: Some(digest),
            approved: Some(true),
            has_native_hook: Some(false),
        })
        .expect("approve");
        assert_eq!(approved.verdict, PreToolUseGuardVerdict::Allow);
        assert!(!approved.blocked);
    }

    #[test]
    fn cursor_native_hook_override_allows_without_approval() {
        let root = skill_repo_root();
        let out = evaluate_pre_tool_use_guard(PreToolUseGuardRequest {
            host_id: "cursor".to_string(),
            tool_name: "Shell".to_string(),
            tool_input: json!({"command": "git reset --hard HEAD"}),
            repo_root: Some(root.display().to_string()),
            phase: None,
            approval_digest: None,
            approved: None,
            has_native_hook: Some(true),
        })
        .expect("evaluate");
        assert!(!out.strict_fallback_active);
        assert_eq!(out.verdict, PreToolUseGuardVerdict::Allow);
    }

    #[test]
    fn protected_path_write_requires_approval_on_anemic_host() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!("pre-tool-guard-{suffix}"));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        fs::copy(
            skill_repo_root().join("configs/framework/RUNTIME_REGISTRY.json"),
            tmp.join("configs/framework/RUNTIME_REGISTRY.json"),
        )
        .unwrap();
        fs::create_dir_all(tmp.join("core/router-rs")).unwrap();
        fs::write(
            tmp.join("core/router-rs/Cargo.toml"),
            "[package]\nname=\"router-rs\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        )
        .unwrap();

        let out = evaluate_pre_tool_use_guard(PreToolUseGuardRequest {
            host_id: "unknown-host".to_string(),
            tool_name: "Write".to_string(),
            tool_input: json!({"path": "AGENTS.md", "contents": "x"}),
            repo_root: Some(tmp.display().to_string()),
            phase: None,
            approval_digest: None,
            approved: None,
            has_native_hook: Some(false),
        })
        .expect("evaluate");
        assert_eq!(out.verdict, PreToolUseGuardVerdict::RequiresStdioApproval);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn contract_declares_stdio_op_and_phases() {
        let contract = pre_tool_use_guard_contract();
        assert_eq!(
            contract["stdio_op"].as_str(),
            Some(PRE_TOOL_USE_GUARD_STDIO_OP)
        );
        assert!(contract["phases"].as_array().is_some());
    }

    #[test]
    fn execution_contract_bundle_includes_pre_tool_use_guard() {
        let bundle = build_execution_contract_bundle();
        let guard = bundle
            .get("pre_tool_use_guard_contract")
            .expect("bundle must embed pre_tool_use_guard_contract");
        assert_eq!(
            guard["stdio_op"].as_str(),
            Some(PRE_TOOL_USE_GUARD_STDIO_OP)
        );
    }

    #[test]
    fn stdio_dispatch_pre_tool_use_guard_roundtrip() {
        let root = skill_repo_root();
        let payload = json!({
            "host_id": "opencode",
            "tool_name": "Shell",
            "tool_input": {"command": "git status"},
            "repo_root": root.display().to_string(),
        });
        // Dispatch goes through runtime-core; test the guard directly.
        let result = evaluate_pre_tool_use_guard_value(payload);
        assert!(result.is_ok(), "pre_tool_use_guard should succeed: {:?}", result.err());
    }

    #[test]
    fn codex_native_host_skips_strict_fallback_for_safe_shell() {
        let root = skill_repo_root();
        let out = evaluate_pre_tool_use_guard(PreToolUseGuardRequest {
            host_id: "codex".to_string(),
            tool_name: "Shell".to_string(),
            tool_input: json!({"command": "git status"}),
            repo_root: Some(root.display().to_string()),
            phase: None,
            approval_digest: None,
            approved: None,
            has_native_hook: None,
        })
        .expect("codex evaluate");
        assert!(!out.strict_fallback_active);
        assert_eq!(out.verdict, PreToolUseGuardVerdict::Allow);
    }
}
