use crate::framework_runtime::build_framework_contract_summary_envelope;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

const CODEX_HOOK_AUTHORITY: &str = "rust-codex-audit";
const HOST_ENTRYPOINT_SYNC_MANIFEST_PATH: &str = ".codex/host_entrypoints_sync_manifest.json";
const HOST_ENTRYPOINT_SYNC_HINT: &str =
    "./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml codex sync --repo-root \"$PWD\"";
const CODEX_AGENT_POLICY_PATH: &str = "AGENTS.md";
const CLAUDE_AGENT_POLICY_PATH: &str = "CLAUDE.md";
const HOST_ENTRYPOINT_JSON_RELATIVE_PATHS: [&str; 0] = [];
const PROTECTED_GENERATED_PATHS: [&str; 3] = [
    CODEX_AGENT_POLICY_PATH,
    CLAUDE_AGENT_POLICY_PATH,
    HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
];
const PROTECTED_GENERATED_PREFIXES: [&str; 0] = [];
pub fn build_codex_hook_manifest() -> Value {
    json!({
        "hooks": {
            "UserPromptSubmit": [
                {
                    "matcher": "*",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "python3 \"$(git rev-parse --show-toplevel)/.codex/hooks/review_subagent_gate.py\"",
                            "timeout": 10,
                            "statusMessage": "Checking review delegation"
                        }
                    ]
                }
            ],
            "PostToolUse": [
                {
                    "matcher": ".*",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "python3 \"$(git rev-parse --show-toplevel)/.codex/hooks/review_subagent_gate.py\"",
                            "timeout": 10,
                            "statusMessage": "Recording review subagent evidence"
                        }
                    ]
                }
            ],
            "Stop": [
                {
                    "matcher": "*",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "python3 \"$(git rev-parse --show-toplevel)/.codex/hooks/review_subagent_gate.py\"",
                            "timeout": 10,
                            "statusMessage": "Checking review closeout"
                        }
                    ]
                }
            ]
        }
    })
}

struct HostEntrypointSyncSection {
    text_files: Vec<String>,
    json_files: Vec<String>,
}

fn host_entrypoint_partial_sync_section(
    desired_files: &BTreeMap<String, Vec<u8>>,
) -> HostEntrypointSyncSection {
    HostEntrypointSyncSection {
        text_files: desired_host_entrypoint_text_files(desired_files),
        json_files: HOST_ENTRYPOINT_JSON_RELATIVE_PATHS
            .iter()
            .map(|path| (*path).to_string())
            .collect(),
    }
}

#[derive(Default)]
struct SingleSyncReport {
    written: Vec<String>,
    unchanged: Vec<String>,
    created_dirs: Vec<String>,
}

pub(crate) fn sync_host_entrypoints(repo_root: &Path, apply: bool) -> Result<Value, String> {
    let root = normalize_repo_root(repo_root)?;
    let desired_files = build_host_entrypoint_files(&root)?;
    let partial_section = host_entrypoint_partial_sync_section(&desired_files);
    let (matched_worktrees, skipped_worktrees) = discover_matching_worktrees(&root);
    let mut report = json!({
        "written": [],
        "unchanged": [],
        "created_dirs": [],
        "synced_worktrees": [],
        "skipped_worktrees": skipped_worktrees,
    });
    let full_text_files = desired_host_entrypoint_text_files(&desired_files);
    let full_json_files = vec![HOST_ENTRYPOINT_SYNC_MANIFEST_PATH];
    let full_section = HostEntrypointSyncSection {
        text_files: full_text_files
            .into_iter()
            .map(|path| path.to_string())
            .collect(),
        json_files: full_json_files
            .into_iter()
            .map(|path| path.to_string())
            .collect(),
    };
    let mut targets = vec![root.clone()];
    targets.extend(matched_worktrees);

    for target_root in targets {
        let section = if target_root == root {
            &full_section
        } else {
            &partial_section
        };
        let single =
            sync_host_entrypoints_single_root(&desired_files, &target_root, &root, apply, section)?;
        extend_report_array(&mut report, "written", single.written)?;
        extend_report_array(&mut report, "unchanged", single.unchanged)?;
        extend_report_array(&mut report, "created_dirs", single.created_dirs)?;
        if target_root != root {
            extend_report_array(
                &mut report,
                "synced_worktrees",
                vec![target_root.to_string_lossy().into_owned()],
            )?;
        }
    }

    sort_report_array(&mut report, "written")?;
    sort_report_array(&mut report, "unchanged")?;
    sort_report_array(&mut report, "created_dirs")?;
    sort_report_array(&mut report, "synced_worktrees")?;
    sort_report_array(&mut report, "skipped_worktrees")?;
    Ok(report)
}

fn build_host_entrypoint_files(_repo_root: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut files = BTreeMap::new();
    let shared_policy = build_shared_agent_policy().into_bytes();
    files.insert(CODEX_AGENT_POLICY_PATH.to_string(), shared_policy.clone());
    files.insert(CLAUDE_AGENT_POLICY_PATH.to_string(), shared_policy);
    files.insert(
        HOST_ENTRYPOINT_SYNC_MANIFEST_PATH.to_string(),
        serialize_pretty_json_bytes(&build_host_entrypoint_sync_manifest(&files))?,
    );
    Ok(files)
}

fn build_host_entrypoint_sync_manifest(desired_files: &BTreeMap<String, Vec<u8>>) -> Value {
    let full_text_files = desired_host_entrypoint_text_files(desired_files);
    json!({
        "schema_version": "host-entrypoints-sync-manifest-v1",
        "shared_system": {
            "policy": "shared-agent-policy-v1",
            "source_of_truth": "skills/",
            "supported_hosts": ["codex-cli", "claude-code-cli"],
            "host_entrypoints": {
                "codex-cli": CODEX_AGENT_POLICY_PATH,
                "claude-code-cli": CLAUDE_AGENT_POLICY_PATH,
            },
        },
        "full_sync": {
            "text_files": full_text_files,
            "json_files": [
                HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
            ],
        },
        "partial_sync": {
            "text_files": full_text_files,
            "json_files": HOST_ENTRYPOINT_JSON_RELATIVE_PATHS,
        },
    })
}

fn desired_host_entrypoint_text_files(desired_files: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    desired_files
        .keys()
        .filter(|path| path.as_str() != HOST_ENTRYPOINT_SYNC_MANIFEST_PATH)
        .filter(|path| !HOST_ENTRYPOINT_JSON_RELATIVE_PATHS.contains(&path.as_str()))
        .cloned()
        .collect()
}

fn serialize_pretty_json_bytes(payload: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(payload).map_err(|err| err.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sync_host_entrypoints_single_root(
    desired_files: &BTreeMap<String, Vec<u8>>,
    target_root: &Path,
    report_root: &Path,
    apply: bool,
    section: &HostEntrypointSyncSection,
) -> Result<SingleSyncReport, String> {
    let mut report = SingleSyncReport::default();
    for relative in section.text_files.iter().chain(section.json_files.iter()) {
        let desired = desired_files
            .get(relative)
            .ok_or_else(|| format!("missing generated host-entrypoint payload for {}", relative))?;
        sync_host_entrypoint_file(
            desired,
            relative,
            target_root,
            report_root,
            apply,
            &mut report,
        )?;
    }

    Ok(report)
}

fn protected_generated_paths() -> Vec<&'static str> {
    PROTECTED_GENERATED_PATHS.to_vec()
}

fn sync_host_entrypoint_file(
    desired: &[u8],
    relative: &str,
    target_root: &Path,
    report_root: &Path,
    apply: bool,
    report: &mut SingleSyncReport,
) -> Result<(), String> {
    let destination = target_root.join(relative);
    let existing = fs::read(&destination).ok();
    let changed = existing.as_deref() != Some(desired);
    if changed && apply {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(&destination, desired).map_err(|err| err.to_string())?;
    }
    let bucket = if changed {
        &mut report.written
    } else {
        &mut report.unchanged
    };
    bucket.push(describe_host_entrypoint_path(
        report_root,
        target_root,
        &destination,
    ));
    Ok(())
}

fn extend_report_array(report: &mut Value, key: &str, items: Vec<String>) -> Result<(), String> {
    let array = report
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("host-entrypoint sync report missing {key} array"))?;
    array.extend(items.into_iter().map(Value::String));
    Ok(())
}

fn sort_report_array(report: &mut Value, key: &str) -> Result<(), String> {
    let array = report
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("host-entrypoint sync report missing {key} array"))?;
    let mut values = array
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.sort();
    *array = values.into_iter().map(Value::String).collect();
    Ok(())
}

fn normalize_repo_root(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .map_err(|err| err.to_string())?
            .join(path))
    }
}

fn discover_matching_worktrees(root: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let worktree_listing = read_git_stdout(root, &["worktree", "list", "--porcelain"]);
    if worktree_listing.is_none() {
        return (Vec::new(), Vec::new());
    }

    let mut current: BTreeMap<String, String> = BTreeMap::new();
    let mut worktrees = Vec::new();
    for raw_line in worktree_listing.unwrap_or_default().lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                worktrees.push(current);
                current = BTreeMap::new();
            }
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let key = parts.next().unwrap_or_default().to_string();
        let value = parts.next().unwrap_or_default().to_string();
        current.insert(key, value);
    }
    if !current.is_empty() {
        worktrees.push(current);
    }

    let mut matches = Vec::new();
    let mut skipped = Vec::new();
    for entry in worktrees {
        let Some(worktree_path) = entry.get("worktree") else {
            continue;
        };
        let candidate = normalize_repo_root(Path::new(worktree_path))
            .unwrap_or_else(|_| PathBuf::from(worktree_path));
        if candidate == root {
            continue;
        }
        if !candidate.exists() {
            skipped.push(format!("{} (missing)", candidate.to_string_lossy()));
            continue;
        }
        matches.push(candidate);
    }
    (matches, skipped)
}

fn read_git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn describe_host_entrypoint_path(report_root: &Path, target_root: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(report_root) {
        return relative.to_string_lossy().into_owned();
    }
    if let Ok(relative) = path.strip_prefix(target_root) {
        return format!(
            "{}::{}",
            target_root.to_string_lossy(),
            relative.to_string_lossy()
        );
    }
    path.to_string_lossy().into_owned()
}

pub fn build_codex_hook_projection() -> Value {
    json!({
        "schema_version": "router-rs-codex-hook-projection-v1",
        "authority": CODEX_HOOK_AUTHORITY,
        "codex_agent_policy": build_shared_agent_policy(),
        "codex_hooks_readme": build_codex_hooks_readme(),
        "codex_hooks": build_codex_hook_manifest(),
        "codex_audit_commands": {
            "pre_tool_use": build_codex_hook_command("pre-tool-use"),
            "contract_guard": build_codex_hook_command("contract-guard"),
        },
    })
}

fn build_shared_agent_policy() -> String {
    include_str!("../../../AGENTS.md").to_string()
}

fn build_codex_hooks_readme() -> String {
    "# Codex Hooks Projection\n\n\
Codex hooks are enabled for this repo.\n\n\
Project-local hooks live in `.codex/hooks.json` and `.codex/hooks/`.\n\n\
The active review gate requires broad/deep review requests to either spawn independent reviewer subagents or record a clear reject reason before finalizing.\n\n\
The Rust hook command remains available for explicit one-off audits.\n\n\
Use `codex hook contract-guard` as an opt-in continuity audit. It compares a caller-provided expected `contract_digest`, owner, task, goal, and evidence intent against the live Rust `framework contract-summary` payload, then fails closed on drift unless the caller sets an explicit contract update intent.\n\n\
Regenerate with:\n\n\
```sh\n\
./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml codex sync --repo-root \"$PWD\"\n\
```\n"
        .to_string()
}

fn build_hook_binary_preamble(
    project_var: &str,
    env_var: &str,
    missing_binary_fallback: &str,
) -> String {
    let mut command = String::new();
    command.push_str(&format!(
        "{project_var}=\"${{{env_var}:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}}\"; "
    ));
    command.push_str(&format!(
        "ROUTER_RS_LAUNCHER=\"${project_var}/scripts/router-rs/run_router_rs.sh\"; "
    ));
    command.push_str(&format!(
        "ROUTER_RS_MANIFEST=\"${project_var}/scripts/router-rs/Cargo.toml\"; "
    ));
    command.push_str("if [ ! -x \"$ROUTER_RS_LAUNCHER\" ]; then ");
    command.push_str(missing_binary_fallback);
    command.push_str("; fi; ");
    command
}

fn build_codex_hook_command(event: &str) -> String {
    let mut command =
        build_hook_binary_preamble("CODEX_PROJECT_ROOT", "CODEX_PROJECT_ROOT", "exit 0");
    command.push_str(&format!(
        "\"$ROUTER_RS_LAUNCHER\" \"$ROUTER_RS_MANIFEST\" codex hook {event} --repo-root \"$CODEX_PROJECT_ROOT\""
    ));
    command
}

pub fn run_codex_audit_hook(command: &str, repo_root: &Path) -> Result<Option<Value>, String> {
    let canonical = canonical_codex_audit_command(command)?;
    let payload = read_stdin_payload()?;
    match canonical {
        "pre-tool-use" => run_codex_pre_tool_use(repo_root, &payload),
        "contract-guard" => run_codex_contract_guard(repo_root, &payload),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    }
}

fn run_codex_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    run_pre_tool_use(repo_root, payload)
}

fn run_codex_contract_guard(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    let envelope = build_framework_contract_summary_envelope(repo_root)?;
    let summary = envelope
        .get("contract_summary")
        .ok_or_else(|| "framework contract summary missing contract_summary".to_string())?;
    let drift_flags = detect_contract_drift(summary, payload);
    let explicit_update = payload_bool(payload, "contract_update_intent")
        || payload_bool(payload, "allow_contract_update")
        || payload_bool(payload, "explicit_contract_update");
    let live_digest = summary
        .get("contract_digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let decision = if !drift_flags.is_empty() && !explicit_update {
        "block"
    } else {
        "approve"
    };
    let reason = if drift_flags.is_empty() {
        "contract guard passed; no drift detected".to_string()
    } else if explicit_update {
        format!(
            "contract guard observed drift but explicit update intent was provided: {}",
            drift_flags.join(", ")
        )
    } else {
        format!(
            "contract guard blocked drift without explicit contract update intent: {}",
            drift_flags.join(", ")
        )
    };
    let mut response = json!({
        "decision": decision,
        "authority": CODEX_HOOK_AUTHORITY,
        "contract_guard": {
            "schema_version": "router-rs-codex-contract-guard-v1",
            "live_contract_digest": live_digest,
            "drift_flags": drift_flags,
            "explicit_contract_update": explicit_update,
            "prompt_lines": summary.get("prompt_lines").cloned().unwrap_or(Value::Array(Vec::new())),
            "reason": reason,
        },
    });
    if decision == "block" {
        response["hookSpecificOutput"] = json!({
            "hookEventName": "ContractGuard",
            "permissionDecision": "deny",
            "permissionDecisionReason": response["contract_guard"]["reason"].clone(),
        });
    }
    Ok(Some(response))
}

fn canonical_codex_audit_command(command: &str) -> Result<&'static str, String> {
    match command {
        "pre-tool-use" => Ok("pre-tool-use"),
        "contract-guard" => Ok("contract-guard"),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    }
}

fn detect_contract_drift(summary: &Value, payload: &Value) -> Vec<String> {
    let mut flags = Vec::new();
    let live_digest = summary
        .get("contract_digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(expected) = payload_string(payload, "expected_contract_digest")
        .or_else(|| payload_string(payload, "contract_digest"))
    {
        let expected = expected.strip_prefix("sha256:").unwrap_or(&expected);
        if !expected.is_empty() && expected != live_digest {
            flags.push("contract_digest_drift".to_string());
        }
    }

    let live_owner = summary
        .get("primary_owner")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(proposed_owner) = payload_string(payload, "proposed_primary_owner")
        .or_else(|| payload_string(payload, "primary_owner"))
    {
        if !live_owner.is_empty() && proposed_owner != live_owner {
            flags.push("owner_drift".to_string());
        }
    }

    let contract_active = summary
        .get("contract_guard")
        .and_then(|guard| guard.get("contract_active"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if contract_active {
        let live_task = summary
            .get("continuity")
            .and_then(|continuity| continuity.get("task"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(proposed_task) =
            payload_string(payload, "proposed_task").or_else(|| payload_string(payload, "task"))
        {
            if !live_task.is_empty() && proposed_task != live_task {
                flags.push("scope_drift".to_string());
            }
        }

        let live_goal = scalar_contract_text(summary.get("goal"));
        if let Some(proposed_goal) =
            payload_string(payload, "proposed_goal").or_else(|| payload_string(payload, "goal"))
        {
            if !live_goal.is_empty() && proposed_goal != live_goal {
                flags.push("scope_drift".to_string());
            }
        }

        let live_evidence = string_array(summary.get("evidence_required"));
        let proposed_evidence = string_array(payload.get("proposed_evidence_required"));
        if !live_evidence.is_empty()
            && proposed_evidence.is_empty()
            && (payload_bool(payload, "drops_evidence_required")
                || payload.get("proposed_evidence_required").is_some())
        {
            flags.push("evidence_drift".to_string());
        }
    }

    flags.sort();
    flags.dedup();
    flags
}

fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn payload_bool(payload: &Value, key: &str) -> bool {
    payload.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn scalar_contract_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => String::new(),
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn block_codex_pre_tool_use(reason: String) -> Option<Value> {
    Some(json!({
        "decision": "block",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    }))
}

fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    let mut rel_paths = HashSet::new();
    for path in iter_payload_paths(payload) {
        rel_paths.insert(relative_candidate_path(&path, repo_root));
    }
    for path in rel_paths.iter().cloned().collect::<Vec<_>>() {
        if classify_protected_generated_path(&path).is_some() {
            let message = pre_tool_use_message(&path);
            return Ok(block_codex_pre_tool_use(message));
        }
    }
    if let Some(path) = bash_generated_write_target(payload) {
        let message = pre_tool_use_message(&path);
        return Ok(block_codex_pre_tool_use(message));
    }
    Ok(None)
}

fn read_stdin_payload() -> Result<Value, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|err| err.to_string())?;
    let trimmed = input.trim();
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

fn iter_payload_paths(payload: &Value) -> Vec<String> {
    let mut candidates = iter_candidate_paths(payload);
    if let Some(tool_input) = payload.get("tool_input") {
        candidates.extend(iter_candidate_paths(tool_input));
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
            return normalize_repo_relative_path(&rel.to_string_lossy());
        }
    }
    normalize_repo_relative_path(path)
}

fn normalize_repo_relative_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else {
                    parts.push(part);
                }
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn classify_protected_generated_path(path: &str) -> Option<&'static str> {
    let normalized = normalize_repo_relative_path(path);
    if protected_generated_paths().contains(&normalized.as_str()) {
        return Some("generated_file");
    }
    if PROTECTED_GENERATED_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return Some("generated_file");
    }
    None
}

fn pre_tool_use_message(path: &str) -> String {
    format!(
        "[codex-pre-tool-use] blocked direct edits to generated Codex agent surface {path}; rerun `{}` instead."
        ,
        HOST_ENTRYPOINT_SYNC_HINT
    )
}

fn bash_generated_write_target(payload: &Value) -> Option<String> {
    let tool_name = payload.get("tool_name").and_then(Value::as_str)?;
    if tool_name != "Bash" {
        return None;
    }
    let command = payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|tool_input| tool_input.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)?;
    for segment in split_bash_segments(command) {
        let looks_mutating = bash_command_looks_mutating(&segment);
        for hint in protected_generated_paths() {
            if bash_segment_mentions_generated_path(&segment, hint)
                && (looks_mutating || bash_segment_redirects_to_hint(&segment, hint))
            {
                return Some(hint.to_string());
            }
        }
    }
    None
}

fn split_bash_segments(command: &str) -> Vec<String> {
    Regex::new(r"\s*(?:&&|\|\||;|\|)\s*")
        .ok()
        .map(|regex| {
            regex
                .split(command)
                .filter_map(|segment| {
                    let trimmed = segment.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![command.trim().to_string()])
}

fn bash_command_looks_mutating(command: &str) -> bool {
    [
        r"^\s*(mv|cp|install|touch|rm|unlink|truncate)\b",
        r"^\s*ln\b[^\n]*\s-[^\n]*[fs][^\n]*\b",
        r"^\s*git\s+(checkout\s+--|restore\b)",
        r"\bsed\s+-i\b",
        r"\bperl\s+-pi\b",
        r"\bpython3?\s+-c\b",
        r"\bnode\s+-e\b",
        r"\bruby\s+-e\b",
        r"\btee\b",
        r"\bdd\b",
    ]
    .iter()
    .any(|pattern| {
        Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(command))
            .unwrap_or(false)
    })
}

fn bash_segment_mentions_generated_path(segment: &str, hint: &str) -> bool {
    segment
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '\'' | '"' | ';' | '&' | '|'))
        .map(|token| token.trim_start_matches('>').trim_start_matches("of="))
        .any(|token| normalize_repo_relative_path(token) == hint)
}

fn bash_segment_redirects_to_hint(segment: &str, hint: &str) -> bool {
    let escaped = regex::escape(hint);
    [
        format!(r#"(>>?|>\|)\s*['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#),
        format!(r#"\btee\b(?:\s+-a)?\s+['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#),
        format!(r#"\bdd\b[^\n;&|]*\bof=['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#),
    ]
    .iter()
    .any(|pattern| {
        Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(segment))
            .unwrap_or(false)
    }) || bash_segment_mentions_generated_path(segment, hint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn protected_generated_paths_match_lexical_variants() {
        assert_eq!(normalize_repo_relative_path("./AGENTS.md"), "AGENTS.md");
        assert_eq!(
            normalize_repo_relative_path(".codex/../.codex/host_entrypoints_sync_manifest.json"),
            ".codex/host_entrypoints_sync_manifest.json"
        );
        assert!(classify_protected_generated_path("./AGENTS.md").is_some());
        assert!(classify_protected_generated_path(
            ".codex/../.codex/host_entrypoints_sync_manifest.json"
        )
        .is_some());
        assert!(classify_protected_generated_path("./.codex/prompts/gitx.md").is_none());
    }

    #[test]
    fn pre_tool_use_blocks_normalized_direct_paths() {
        let payload = json!({"tool_input": {"file_path": "./AGENTS.md"}});
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({"tool_input": {"file_path": ".codex/../.codex/host_entrypoints_sync_manifest.json"}});
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({"tool_input": {"file_path": ".codex/../.codex/prompts/autopilot.md"}});
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());
    }

    #[test]
    fn pre_tool_use_blocks_normalized_bash_write_targets() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x > ./AGENTS.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x | tee .codex/../.codex/host_entrypoints_sync_manifest.json"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x | tee .codex/prompts/gitx.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());
    }
}
