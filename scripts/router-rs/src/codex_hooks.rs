use crate::framework_runtime::build_framework_contract_summary_envelope;
use crate::cursor_hooks::{
    has_delegation_override, has_override, has_review_override, is_parallel_delegation_prompt,
    is_review_prompt, normalize_subagent_type, normalize_tool_name, saw_reject_reason,
};
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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
const HOST_ENTRYPOINT_JSON_RELATIVE_PATHS: [&str; 0] = [];
const PROTECTED_GENERATED_PATHS: [&str; 2] =
    [CODEX_AGENT_POLICY_PATH, HOST_ENTRYPOINT_SYNC_MANIFEST_PATH];
const PROTECTED_GENERATED_PREFIXES: [&str; 0] = [];
const CODEX_REVIEW_SUBAGENT_TOOL_NAMES: [&str; 6] = [
    "task",
    "functions.task",
    "functions.subagent",
    "functions.spawn_agent",
    "subagent",
    "spawn_agent",
];
const CODEX_REVIEW_SUBAGENT_TYPES: [&str; 13] = [
    "generalpurpose",
    "explore",
    "shell",
    "browser-use",
    "browseruse",
    "cursor-guide",
    "cursorguide",
    "ci-investigator",
    "ciinvestigator",
    "best-of-n-runner",
    "bestofnrunner",
    "explorer",
    "general-purpose",
];

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct CodexReviewGateState {
    #[serde(default)]
    seq: i64,
    #[serde(default)]
    subagent_required: bool,
    #[serde(default)]
    review_required: bool,
    #[serde(default)]
    delegation_required: bool,
    #[serde(default)]
    review_override: bool,
    #[serde(default)]
    delegation_override: bool,
    #[serde(default)]
    reject_reason_seen: bool,
    #[serde(default)]
    review_subagent_seen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    review_subagent_tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
}

fn codex_state_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".codex").join("hook-state")
}

fn codex_session_key(event: &Value) -> String {
    let raw = event
        .get("session_id")
        .or_else(|| event.get("conversation_id"))
        .or_else(|| event.get("thread_id"))
        .or_else(|| event.get("cwd"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("default");
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    let full_hex = digest
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    full_hex.chars().take(32).collect()
}

fn codex_state_path(repo_root: &Path, event: &Value) -> PathBuf {
    codex_state_dir(repo_root).join(format!("review-subagent-{}.json", codex_session_key(event)))
}

fn codex_load_state(repo_root: &Path, event: &Value) -> Result<Option<CodexReviewGateState>, String> {
    let path = codex_state_path(repo_root, event);
    let text = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("state_read_failed".to_string()),
    };
    serde_json::from_str::<CodexReviewGateState>(&text)
        .map(Some)
        .map_err(|_| "state_json_invalid".to_string())
}

fn codex_save_state(repo_root: &Path, event: &Value, state: &CodexReviewGateState) -> bool {
    let directory = codex_state_dir(repo_root);
    let target = codex_state_path(repo_root, event);
    let mut payload = match serde_json::to_string_pretty(state) {
        Ok(value) => value,
        Err(_) => return false,
    };
    payload.push('\n');
    if fs::create_dir_all(&directory).is_err() {
        return false;
    }
    let tmp = directory.join(format!(
        ".tmp-{}-{}",
        std::process::id(),
        target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("state.json")
    ));
    if fs::write(&tmp, payload).is_err() {
        return false;
    }
    if fs::rename(&tmp, &target).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    true
}

fn codex_prompt_text(event: &Value) -> String {
    for key in ["prompt", "user_prompt", "message", "input"] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            return value.to_string();
        }
    }
    String::new()
}

fn codex_tool_name(event: &Value) -> String {
    event
        .get("tool_name")
        .or_else(|| event.get("tool"))
        .or_else(|| event.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn codex_tool_input(event: &Value) -> Value {
    event
        .get("tool_input")
        .or_else(|| event.get("input"))
        .or_else(|| event.get("arguments"))
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn saw_subagent_codex(event: &Value) -> bool {
    let name = normalize_tool_name(Some(&codex_tool_name(event)));
    if !CODEX_REVIEW_SUBAGENT_TOOL_NAMES.contains(&name.as_str()) {
        return false;
    }
    let input = codex_tool_input(event);
    let typed_fields = [
        input.get("subagent_type").and_then(Value::as_str),
        input.get("agent_type").and_then(Value::as_str),
        input.get("agentType").and_then(Value::as_str),
    ];
    typed_fields.iter().any(|field| {
        let normalized = normalize_subagent_type(*field);
        CODEX_REVIEW_SUBAGENT_TYPES.contains(&normalized.as_str())
    })
}

fn handle_codex_userpromptsubmit(repo_root: &Path, event: &Value) -> Option<Value> {
    let text = codex_prompt_text(event);
    let mut state = CodexReviewGateState {
        seq: 0,
        subagent_required: true,
        ..CodexReviewGateState::default()
    };

    if is_review_prompt(&text) {
        state.review_required = true;
        state.prompt = Some(text.chars().take(500).collect());
    }
    if is_parallel_delegation_prompt(&text) {
        state.delegation_required = true;
        state.prompt = Some(text.chars().take(500).collect());
    }
    if has_override(&text) {
        state.review_override = true;
        state.delegation_override = true;
    }
    if saw_reject_reason(&text) {
        state.reject_reason_seen = true;
    }

    if !codex_save_state(repo_root, event, &state) {
        return Some(json!({
            "decision": "block",
            "reason": "Review gate state could not be persisted under .codex/hook-state. Fail-closed to avoid silent policy bypass.",
        }));
    }

    let mut contexts: Vec<String> = Vec::new();
    if state.subagent_required && !state.review_override {
        contexts.push(
            "Default subagent policy is active. Spawn one or more bounded subagent lanes NOW (before doing the main analysis). If you will not spawn, explicitly state one reject reason: small_task / shared_context_heavy / write_scope_overlap / next_step_blocked / verification_missing / token_overhead_dominates.\n\nQuick evaluation guide:\n- Use subagent by default for search, review, testing, or independent module work\n- Skip subagent only when one reject reason clearly applies\n".to_string()
        );
    }
    if state.review_required && !state.review_override {
        contexts.push(
            "Broad/deep review detected. Spawn independent reviewer subagent lanes NOW (before doing the main analysis). If you will not spawn, explicitly state one reject reason: small_task / shared_context_heavy / write_scope_overlap / next_step_blocked / verification_missing / token_overhead_dominates.\n\nSuggested lanes (copy/paste and customize):\n- Security lane: threat model + secrets + auth + supply chain risks\n- Architecture lane: boundaries + data flow + invariants + sharp edges\n- Regression lane: behavior changes + tests gaps + rollout risk\n".to_string()
        );
    }
    if state.delegation_required && !state.delegation_override {
        contexts.push(
            "Parallel lane request detected. Spawn bounded subagent lanes NOW (before integrating). If you will not spawn, explicitly state one reject reason.\n\nSuggested lanes (copy/paste):\n- API lane: contracts + backward compatibility + error semantics\n- DB lane: migrations + indexes + consistency + performance\n- UI lane: UX regressions + accessibility + edge cases\n".to_string()
        );
    }

    if contexts.is_empty() {
        None
    } else {
        Some(json!({
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": contexts.join("\n"),
            }
        }))
    }
}

fn handle_codex_posttooluse(repo_root: &Path, event: &Value) -> Option<Value> {
    if !saw_subagent_codex(event) {
        return None;
    }
    let mut state = codex_load_state(repo_root, event).ok().flatten().unwrap_or_default();
    state.review_subagent_seen = true;
    state.review_subagent_tool = Some(codex_tool_name(event));
    if !codex_save_state(repo_root, event, &state) {
        return Some(json!({
            "decision": "block",
            "reason": "Review gate state update failed under .codex/hook-state after subagent evidence. Fail-closed to avoid inconsistent gating.",
        }));
    }
    None
}

fn handle_codex_stop(repo_root: &Path, event: &Value) -> Option<Value> {
    if event
        .get("stop_hook_active")
        .or_else(|| event.get("stopHookActive"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }

    let loaded = codex_load_state(repo_root, event);
    let text = codex_prompt_text(event);
    let inferred_overridden = has_override(&text)
        || has_review_override(&text)
        || has_delegation_override(&text)
        || saw_reject_reason(&text);
    let inferred_required = !text.trim().is_empty() && !inferred_overridden;

    let state = match loaded {
        Ok(None) => {
            let mut reason = "Review gate state is missing under .codex/hook-state. Fail-closed to avoid bypass when enforcement state is unavailable.".to_string();
            if !inferred_required {
                reason.push_str(" Stop payload had no review context; blocking conservatively.");
            }
            return Some(json!({
                "decision": "block",
                "reason": reason,
            }));
        }
        Err(io_error) => {
            return Some(json!({
                "decision": "block",
                "reason": format!(
                    "Review gate state is unreadable or unavailable under .codex/hook-state ({}). Fail-closed to avoid silent policy bypass.",
                    io_error
                ),
            }))
        }
        Ok(Some(value)) => value,
    };

    if state.subagent_required
        && !state.review_override
        && !state.reject_reason_seen
        && !state.review_subagent_seen
    {
        return Some(json!({
            "decision": "block",
            "reason": "Default subagent policy is active, but no subagent was observed. Spawn a bounded subagent lane now, or explicitly record a reject reason (small_task/shared_context_heavy/write_scope_overlap/next_step_blocked/verification_missing/token_overhead_dominates).",
        }));
    }
    if state.review_required
        && !state.review_override
        && !state.reject_reason_seen
        && !state.review_subagent_seen
    {
        return Some(json!({
            "decision": "block",
            "reason": "Broad/deep review was requested, but no independent subagent review was observed. Spawn suitable reviewer sidecars now, or explicitly record why spawning is rejected.",
        }));
    }
    if state.delegation_required
        && !state.delegation_override
        && !state.reject_reason_seen
        && !state.review_subagent_seen
    {
        return Some(json!({
            "decision": "block",
            "reason": "Independent parallel lanes were requested, but no bounded subagent sidecar was observed. Spawn suitable sidecars before finalizing, or rerun with an explicit no-subagent override.",
        }));
    }

    let reset = CodexReviewGateState {
        seq: 0,
        ..CodexReviewGateState::default()
    };
    let _ = codex_save_state(repo_root, event, &reset);
    None
}

fn run_codex_review_subagent_gate(
    repo_root: &Path,
    payload: &Value,
) -> Result<Option<Value>, String> {
    let event_name = payload
        .get("hook_event_name")
        .or_else(|| payload.get("event"))
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    Ok(match event_name.as_str() {
        "userpromptsubmit" => handle_codex_userpromptsubmit(repo_root, payload),
        "posttooluse" => handle_codex_posttooluse(repo_root, payload),
        "stop" => handle_codex_stop(repo_root, payload),
        _ => None,
    })
}
pub fn build_codex_hook_manifest() -> Value {
    json!({
        "hooks": {}
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
    would_write: Vec<String>,
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
        "would_write": [],
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
        extend_report_array(&mut report, "would_write", single.would_write)?;
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
    sort_report_array(&mut report, "would_write")?;
    sort_report_array(&mut report, "unchanged")?;
    sort_report_array(&mut report, "created_dirs")?;
    sort_report_array(&mut report, "synced_worktrees")?;
    sort_report_array(&mut report, "skipped_worktrees")?;
    Ok(report)
}

fn build_host_entrypoint_files(_repo_root: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut files = BTreeMap::new();
    files.insert(
        CODEX_AGENT_POLICY_PATH.to_string(),
        build_codex_agent_policy().into_bytes(),
    );
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
            "policy": "host-specific-agent-policy-v1",
            "source_of_truth": "skills/",
            "supported_hosts": ["codex-cli"],
            "host_entrypoints": {
                "codex-cli": CODEX_AGENT_POLICY_PATH,
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
    let bucket = if changed && apply {
        &mut report.written
    } else if changed {
        &mut report.would_write
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
        "codex_agent_policy": build_codex_agent_policy(),
        "codex_hooks_readme": build_codex_hooks_readme(),
        "codex_hooks": build_codex_hook_manifest(),
        "codex_audit_commands": {
            "pre_tool_use": build_codex_hook_command("pre-tool-use"),
            "contract_guard": build_codex_hook_command("contract-guard"),
            "review_subagent_gate": build_codex_hook_command("review-subagent-gate"),
        },
    })
}

fn build_codex_agent_policy() -> String {
    include_str!("../../../AGENTS.md").to_string()
}

fn build_codex_hooks_readme() -> String {
    "# Codex Hooks Projection\n\n\
Codex hooks are disabled for this repo by default.\n\n\
Project-local `.codex/hooks.json` intentionally contains no active hooks.\n\n\
By default, the hook scripts under `.codex/hooks/` are inactive fixtures or explicit audit helpers.\n\n\
After running `scripts/install_codex_cli_hooks.sh`, `~/.codex/hooks.json` will include a codex-cli command hook for `.codex/hooks/review_subagent_gate.py` on `UserPromptSubmit`, `PostToolUse`, and `Stop`.\n\n\
The Rust hook commands remain available for explicit one-off audits.\n\n\
Use `scripts/install_codex_cli_hooks.sh` to install user-level hooks into `~/.codex/` for codex-cli only. The installer validates `python3` and hook script presence, enables `[features].codex_hooks = true` in `~/.codex/config.toml`, keeps existing hooks, and idempotently appends the review-subagent command hook without replacing unrelated handlers.\n\n\
The review-subagent hook writes transient state under `.codex/hook-state/` in the current repository while the session is active.\n\n\
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
        "review-subagent-gate" => run_codex_review_subagent_gate(repo_root, &payload),
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
        "review-subagent-gate" => Ok("review-subagent-gate"),
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
        let proposed_evidence_exists = payload.get("proposed_evidence_required").is_some();
        let proposed_evidence = string_array(payload.get("proposed_evidence_required"));
        let drops_evidence = payload_bool(payload, "drops_evidence_required");
        if drops_evidence && !live_evidence.is_empty() {
            flags.push("evidence_drift".to_string());
        } else if proposed_evidence_exists
            && normalized_string_set(&proposed_evidence) != normalized_string_set(&live_evidence)
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

fn normalized_string_set(values: &[String]) -> Vec<String> {
    let mut deduped = HashSet::new();
    let mut normalized = values
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .filter_map(|item| {
            let lower = item.to_ascii_lowercase();
            deduped.insert(lower.clone()).then_some(lower)
        })
        .collect::<Vec<_>>();
    normalized.sort();
    normalized
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
    let chars = command.chars().collect::<Vec<_>>();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut idx = 0usize;

    while idx < chars.len() {
        let current = chars[idx];
        let next = chars.get(idx + 1).copied();
        let prev = if idx > 0 { Some(chars[idx - 1]) } else { None };
        let mut separator_len = 0usize;

        if current == ';' {
            separator_len = 1;
        } else if current == '&' && next == Some('&') {
            separator_len = 2;
        } else if current == '|' && next == Some('|') {
            separator_len = 2;
        } else if current == '|' && prev != Some('>') {
            separator_len = 1;
        }

        if separator_len > 0 {
            let segment = chars[start..idx].iter().collect::<String>();
            let trimmed = segment.trim();
            if !trimmed.is_empty() {
                segments.push(trimmed.to_string());
            }
            idx += separator_len;
            start = idx;
            continue;
        }

        idx += 1;
    }

    let tail = chars[start..].iter().collect::<String>();
    let trimmed = tail.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }

    if segments.is_empty() {
        vec![command.trim().to_string()]
    } else {
        segments
    }
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

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

        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x >| ./AGENTS.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
    }

    #[test]
    fn pre_tool_use_allows_read_only_bash_commands_on_protected_paths() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cat ./AGENTS.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());

        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg contract_digest .codex/host_entrypoints_sync_manifest.json"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());
    }

    #[test]
    fn sync_host_entrypoints_reports_would_write_in_dry_run() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("router-rs-codex-hooks-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join(".codex")).unwrap();
        fs::write(root.join("AGENTS.md"), "stale").unwrap();
        fs::write(root.join(".codex/host_entrypoints_sync_manifest.json"), "{}").unwrap();

        let report = sync_host_entrypoints(&root, false).unwrap();
        let would_write = report
            .get("would_write")
            .and_then(Value::as_array)
            .unwrap()
            .len();
        let written = report.get("written").and_then(Value::as_array).unwrap().len();
        assert!(would_write > 0);
        assert_eq!(written, 0);

        fs::remove_dir_all(&root).unwrap();
    }

    mod review_gate_tests {
        use super::*;
        use serde_json::json;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQ: AtomicU64 = AtomicU64::new(0);

        fn fresh_repo() -> std::path::PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "codex-review-gate-test-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(dir.join(".codex/hook-state")).unwrap();
            std::fs::create_dir_all(dir.join(".codex/hooks")).unwrap();
            std::fs::write(dir.join(".codex/hooks/review_subagent_gate.py"), b"# stub").unwrap();
            dir
        }

        #[test]
        fn user_prompt_submit_review_emits_additional_context() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-1",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review全仓找bug"
            });
            let out = run_codex_review_subagent_gate(&repo, &payload).unwrap();
            let ctx = out
                .and_then(|v| v.get("hookSpecificOutput").cloned())
                .unwrap()
                .get("additionalContext")
                .and_then(Value::as_str)
                .unwrap()
                .to_string();
            assert!(ctx.contains("Default subagent policy is active."));
            assert!(ctx.contains("Broad/deep review detected."));
        }

        #[test]
        fn user_prompt_submit_with_override_does_not_emit() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-ovr",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review全仓找bug，不要用子代理"
            });
            let out = run_codex_review_subagent_gate(&repo, &payload).unwrap();
            assert!(out.is_none());
        }

        #[test]
        fn post_tool_use_with_subagent_marks_seen() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore"}
            });
            let out = run_codex_review_subagent_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(state.review_subagent_seen);
        }

        #[test]
        fn stop_without_state_blocks_when_required() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-3",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let out = run_codex_review_subagent_gate(&repo, &payload).unwrap().unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
        }

        #[test]
        fn stop_without_state_still_blocks_when_no_text() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-4",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":""
            });
            let out = run_codex_review_subagent_gate(&repo, &payload).unwrap().unwrap();
            let reason = out.get("reason").and_then(Value::as_str).unwrap();
            assert!(reason.contains("Stop payload had no review context"));
        }

        #[test]
        fn stop_with_review_required_no_subagent_blocks() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-5",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-5",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_codex_review_subagent_gate(&repo, &stop).unwrap().unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
        }

        #[test]
        fn stop_with_delegation_required_blocks() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-6",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"前端后端测试并行推进"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-6",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_codex_review_subagent_gate(&repo, &stop).unwrap().unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
        }

        #[test]
        fn stop_with_subagent_seen_resets_state() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-7",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-7",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore"}
            });
            let _ = run_codex_review_subagent_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-7",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_codex_review_subagent_gate(&repo, &stop).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &stop).unwrap().unwrap();
            assert_eq!(state.seq, 0);
            assert!(!state.review_subagent_seen);
        }

        #[test]
        fn stop_hook_active_returns_none() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-8",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review",
                "stop_hook_active": true
            });
            let out = run_codex_review_subagent_gate(&repo, &payload).unwrap();
            assert!(out.is_none());
        }

        #[test]
        fn dispatch_unknown_event_returns_none() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Other",
                "session_id":"sm-9",
                "cwd": repo.to_string_lossy().to_string()
            });
            let out = run_codex_review_subagent_gate(&repo, &payload).unwrap();
            assert!(out.is_none());
        }
    }
}
