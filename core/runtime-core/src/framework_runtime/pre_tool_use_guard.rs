//! PreToolUse strict fallback for anemic hosts (no native PreToolUse / `hard_gate_hooks`).
//!
//! Rich CLI hosts (Cursor, Codex, Claude Code) rely on shell hooks; MCP-only hosts must call the
//! `pre_tool_use_guard` stdio op before high-risk tool execution. HostProvider may override
//! `has_native_hook` on the request payload (roadmap §4.1); registry remains the default.

use crate::hook_policy::{
    dangerous_bash_reason, dangerous_mcp_tool_reason, evaluate_hook_policy, HookPolicyEvaluateRequest,
};
use crate::runtime_registry::load_runtime_registry_json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const PRE_TOOL_USE_GUARD_SCHEMA_VERSION: &str = "router-rs-pre-tool-use-guard-v1";
pub const PRE_TOOL_USE_GUARD_AUTHORITY: &str = "rust-framework-runtime";
pub const PRE_TOOL_USE_GUARD_STDIO_OP: &str = "pre_tool_use_guard";

const ANEMIC_MCP_HOST_IDS: &[&str] = &["opencode", "antigravity"];

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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreToolUseGuardVerdict {
    Allow,
    Block,
    RequiresStdioApproval,
}

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

pub fn pre_tool_use_guard_contract() -> Value {
    json!({
        "schema_version": PRE_TOOL_USE_GUARD_SCHEMA_VERSION,
        "authority": PRE_TOOL_USE_GUARD_AUTHORITY,
        "stdio_op": PRE_TOOL_USE_GUARD_STDIO_OP,
        "phases": ["evaluate", "approve"],
        "anemic_host_ids": ANEMIC_MCP_HOST_IDS,
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

pub fn host_requires_strict_pre_tool_fallback(
    host_id: &str,
    repo_root: &Path,
    has_native_hook_override: Option<bool>,
) -> Result<bool, String> {
    if has_native_hook_override == Some(true) {
        return Ok(false);
    }
    if has_native_hook_override == Some(false) {
        return Ok(true);
    }
    let id = host_id.trim();
    if let Some(strict) = crate::hosts::host_provider_strict_pre_tool_fallback_hint(id) {
        return Ok(strict);
    }
    if ANEMIC_MCP_HOST_IDS.contains(&id) {
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
    if capabilities.iter().any(|cap| *cap == "hard_gate_hooks") {
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
    if harness_caps
        .iter()
        .any(|cap| *cap == "closeout_evidence_hooks")
    {
        return Ok(false);
    }
    Ok(true)
}

pub fn evaluate_pre_tool_use_guard(
    request: PreToolUseGuardRequest,
) -> Result<PreToolUseGuardResponse, String> {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    let repo_root = request
        .repo_root
        .as_deref()
        .map(Path::new)
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| super::resolve_repo_root_arg(Some(p)))
        .transpose()?
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf())
        });
    let host_id = request.host_id.trim().to_string();
    let tool_name = request.tool_name.trim().to_string();
    let strict = host_requires_strict_pre_tool_fallback(
        &host_id,
        &repo_root,
        request.has_native_hook,
    )?;
    let phase = request
        .phase
        .as_deref()
        .unwrap_or("evaluate")
        .trim()
        .to_ascii_lowercase();
    let digest = approval_digest(&host_id, &tool_name, &request.tool_input);

    if phase == "approve" {
        let approved = request.approved.unwrap_or(false);
        let verdict = approve_verdict(
            strict,
            approved,
            request.approval_digest.as_deref(),
            &digest,
        );
        crate::telemetry_emit::emit_tool_call(&tool_name, 0, approved && verdict != PreToolUseGuardVerdict::Block);
        crate::telemetry_emit::emit_hook_fired(
            "pre_tool_use_guard",
            if approved { "approve" } else { "deny" },
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

    if crate::router_env_flags::router_rs_skip_pre_tool_use_guard() {
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

    let (verdict, reason, categories) = classify_high_risk(&tool_name, &request.tool_input, &repo_root)?;
    let final_verdict = match verdict {
        PreToolUseGuardVerdict::Block => PreToolUseGuardVerdict::Block,
        PreToolUseGuardVerdict::RequiresStdioApproval => PreToolUseGuardVerdict::RequiresStdioApproval,
        PreToolUseGuardVerdict::Allow => PreToolUseGuardVerdict::Allow,
    };
    let needs_digest = final_verdict == PreToolUseGuardVerdict::RequiresStdioApproval;
    let action = match final_verdict {
        PreToolUseGuardVerdict::Allow => "allow",
        PreToolUseGuardVerdict::Block => "block",
        PreToolUseGuardVerdict::RequiresStdioApproval => "require_approval",
    };
    crate::telemetry_emit::emit_hook_fired("pre_tool_use_guard", action);
    Ok(build_response(
        &host_id,
        &tool_name,
        true,
        final_verdict,
        reason,
        categories,
        if needs_digest {
            Some(digest)
        } else {
            None
        },
    ))
}

pub fn evaluate_pre_tool_use_guard_value(payload: Value) -> Result<Value, String> {
    let request = serde_json::from_value::<PreToolUseGuardRequest>(payload)
        .map_err(|err| format!("parse pre_tool_use_guard input failed: {err}"))?;
    serde_json::to_value(evaluate_pre_tool_use_guard(request)?)
        .map_err(|err| format!("serialize pre_tool_use_guard output failed: {err}"))
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

fn approval_digest(host_id: &str, tool_name: &str, tool_input: &Value) -> String {
    let canonical = json!({
        "host_id": host_id,
        "tool_name": tool_name,
        "tool_input": tool_input,
    });
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn classify_high_risk(
    tool_name: &str,
    tool_input: &Value,
    repo_root: &Path,
) -> Result<(PreToolUseGuardVerdict, Option<String>, Vec<String>), String> {
    let lowered = tool_name.to_ascii_lowercase();
    if is_shell_tool(&lowered) {
        if let Some(command) = extract_shell_command(tool_input) {
            if let Some(reason) = dangerous_bash_reason(&command) {
                return Ok((
                    PreToolUseGuardVerdict::RequiresStdioApproval,
                    Some(reason),
                    vec!["shell".to_string()],
                ));
            }
        }
    }
    if is_file_write_tool(&lowered) {
        if let Some(path) = extract_file_path(tool_input) {
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
    }
    let args_str = serde_json::to_string(tool_input).unwrap_or_default();
    if let Some(reason) = dangerous_mcp_tool_reason(tool_name, &args_str) {
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
    for key in ["path", "file_path", "target_file", "filePath"] {
        if let Some(text) = tool_input.get(key).and_then(Value::as_str) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn opencode_requires_strict_fallback_from_registry() {
        let root = skill_repo_root();
        assert!(host_requires_strict_pre_tool_fallback("opencode", &root, None).unwrap());
    }

    #[test]
    fn claude_code_skips_strict_fallback_with_hard_gate_hooks() {
        let root = skill_repo_root();
        assert!(!host_requires_strict_pre_tool_fallback("claude-code", &root, None).unwrap());
    }

    #[test]
    fn host_provider_override_native_hook_disables_strict_fallback() {
        let root = skill_repo_root();
        assert!(!host_requires_strict_pre_tool_fallback("opencode", &root, Some(true)).unwrap());
    }

    #[test]
    fn strict_fallback_blocks_dangerous_shell_until_approved() {
        let root = skill_repo_root();
        let evaluate = evaluate_pre_tool_use_guard(PreToolUseGuardRequest {
            host_id: "opencode".to_string(),
            tool_name: "Shell".to_string(),
            tool_input: json!({"command": "git reset --hard HEAD"}),
            repo_root: Some(root.display().to_string()),
            phase: None,
            approval_digest: None,
            approved: None,
            has_native_hook: None,
        })
        .expect("evaluate");
        assert!(evaluate.strict_fallback_active);
        assert_eq!(evaluate.verdict, PreToolUseGuardVerdict::RequiresStdioApproval);
        assert!(evaluate.blocked);
        let digest = evaluate.approval_digest.expect("digest");

        let approved = evaluate_pre_tool_use_guard(PreToolUseGuardRequest {
            host_id: "opencode".to_string(),
            tool_name: "Shell".to_string(),
            tool_input: json!({"command": "git reset --hard HEAD"}),
            repo_root: Some(root.display().to_string()),
            phase: Some("approve".to_string()),
            approval_digest: Some(digest),
            approved: Some(true),
            has_native_hook: None,
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
            host_id: "opencode".to_string(),
            tool_name: "Write".to_string(),
            tool_input: json!({"path": "AGENTS.md", "contents": "x"}),
            repo_root: Some(tmp.display().to_string()),
            phase: None,
            approval_digest: None,
            approved: None,
            has_native_hook: None,
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
        let out = crate::framework_runtime::stdio_dispatch::dispatch_stdio_json_request("pre_tool_use_guard", payload)
            .expect("stdio dispatch");
        assert_eq!(out["verdict"], "allow");
    }

    /// P3 integration: HostProvider closed-set hints drive strict fallback without registry disk I/O.
    #[test]
    fn host_provider_registry_drives_strict_fallback_integration() {
        let root = skill_repo_root();
        let cases: [(&str, bool); 5] = [
            ("cursor", false),
            ("codex", false),
            ("claude-code", false),
            ("opencode", true),
            ("antigravity", true),
        ];
        for (host_id, expect_strict) in cases {
            let strict =
                host_requires_strict_pre_tool_fallback(host_id, &root, None).expect(host_id);
            assert_eq!(
                strict, expect_strict,
                "HostProvider hint mismatch for {host_id}"
            );
            let hint = crate::hosts::host_provider_strict_pre_tool_fallback_hint(host_id);
            assert_eq!(
                hint,
                Some(expect_strict),
                "provider hint must match guard for {host_id}"
            );
        }
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
