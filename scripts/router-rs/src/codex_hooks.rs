use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

const CODEX_HOOK_AUTHORITY: &str = "rust-codex-hook";
const HOST_ENTRYPOINT_SYNC_MANIFEST_PATH: &str = ".codex/host_entrypoints_sync_manifest.json";
const HOST_ENTRYPOINT_SYNC_HINT: &str =
    "./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --sync-host-entrypoints-json --repo-root \"$PWD\"";
const HOST_ENTRYPOINT_FULL_SYNC_MANAGED_DIRECTORIES: [&str; 1] = [".codex/skills"];
const HOST_ENTRYPOINT_PARTIAL_SYNC_TEXT_FILES: [&str; 2] = ["AGENTS.md", CODEX_HOOK_README_PATH];
const HOST_ENTRYPOINT_JSON_RELATIVE_PATHS: [&str; 1] = [".codex/hooks.json"];
const RETIRED_HOST_ENTRYPOINT_PATH_BYTES: [&[u8]; 17] = [
    &[65, 71, 69, 78, 84, 46, 109, 100],
    &[67, 76, 65, 85, 68, 69, 46, 109, 100],
    &[71, 69, 77, 73, 78, 73, 46, 109, 100],
    &[67, 85, 82, 83, 79, 82, 46, 109, 100],
    &[46, 99, 108, 97, 117, 100, 101],
    &[46, 103, 101, 109, 105, 110, 105],
    &[46, 99, 117, 114, 115, 111, 114],
    &[46, 97, 105, 111, 110, 117, 105],
    &[97, 105, 111, 110, 117, 105],
    &[119, 111, 114, 107, 98, 117, 100, 100, 121],
    &[107, 105, 109, 105],
    &[103, 108, 109],
    &[46, 109, 99, 112, 46, 106, 115, 111, 110],
    &[
        99, 111, 110, 102, 105, 103, 115, 47, 99, 111, 100, 101, 120, 47, 65, 71, 69, 78, 84, 83,
        46, 109, 100,
    ],
    &[
        99, 111, 110, 102, 105, 103, 115, 47, 99, 108, 97, 117, 100, 101,
    ],
    &[
        99, 111, 110, 102, 105, 103, 115, 47, 103, 101, 109, 105, 110, 105,
    ],
    &[
        46, 99, 111, 100, 101, 120, 47, 109, 111, 100, 101, 108, 95, 105, 110, 115, 116, 114, 117,
        99, 116, 105, 111, 110, 115, 46, 109, 100,
    ],
];
const PROTECTED_GENERATED_PATH_BYTES: [&[u8]; 4] = [
    &[65, 71, 69, 78, 84, 83, 46, 109, 100],
    &[
        46, 99, 111, 100, 101, 120, 47, 104, 111, 111, 107, 115, 46, 106, 115, 111, 110,
    ],
    &[
        46, 99, 111, 100, 101, 120, 47, 82, 69, 65, 68, 77, 69, 46, 109, 100,
    ],
    &[
        46, 99, 111, 100, 101, 120, 47, 104, 111, 115, 116, 95, 101, 110, 116, 114, 121, 112, 111,
        105, 110, 116, 115, 95, 115, 121, 110, 99, 95, 109, 97, 110, 105, 102, 101, 115, 116, 46,
        106, 115, 111, 110,
    ],
];
const PROTECTED_GENERATED_PREFIXES: [&str; 0] = [];
const CODEX_HOOK_README_PATH: &str = ".codex/README.md";
pub fn build_codex_hook_manifest() -> Value {
    json!({
        "hooks": {
            "PreToolUse": [
                build_codex_command_hook("pre-tool-use", "Bash"),
            ],
        }
    })
}

struct HostEntrypointSyncSection {
    text_files: Vec<String>,
    json_files: Vec<String>,
    managed_directories: Vec<String>,
    retired_paths: Vec<String>,
}

fn host_entrypoint_partial_sync_section() -> HostEntrypointSyncSection {
    HostEntrypointSyncSection {
        text_files: HOST_ENTRYPOINT_PARTIAL_SYNC_TEXT_FILES
            .iter()
            .map(|path| (*path).to_string())
            .collect(),
        json_files: HOST_ENTRYPOINT_JSON_RELATIVE_PATHS
            .iter()
            .map(|path| (*path).to_string())
            .collect(),
        managed_directories: HOST_ENTRYPOINT_FULL_SYNC_MANAGED_DIRECTORIES
            .iter()
            .map(|path| (*path).to_string())
            .collect(),
        retired_paths: retired_host_entrypoint_paths(),
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
    let partial_section = host_entrypoint_partial_sync_section();
    let (matched_worktrees, skipped_worktrees) = discover_matching_worktrees(&root);
    let mut report = json!({
        "written": [],
        "unchanged": [],
        "created_dirs": [],
        "synced_worktrees": [],
        "skipped_worktrees": skipped_worktrees,
    });
    let full_text_files = desired_files
        .keys()
        .filter(|path| path.as_str() != HOST_ENTRYPOINT_SYNC_MANIFEST_PATH)
        .filter(|path| !HOST_ENTRYPOINT_JSON_RELATIVE_PATHS.contains(&path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let full_json_files = vec![
        HOST_ENTRYPOINT_JSON_RELATIVE_PATHS[0],
        HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
    ];
    let full_section = HostEntrypointSyncSection {
        text_files: full_text_files
            .into_iter()
            .map(|path| path.to_string())
            .collect(),
        json_files: full_json_files
            .into_iter()
            .map(|path| path.to_string())
            .collect(),
        managed_directories: HOST_ENTRYPOINT_FULL_SYNC_MANAGED_DIRECTORIES
            .iter()
            .map(|path| (*path).to_string())
            .collect(),
        retired_paths: retired_host_entrypoint_paths(),
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
    files.insert(
        "AGENTS.md".to_string(),
        build_codex_agent_policy().into_bytes(),
    );
    files.insert(
        ".codex/hooks.json".to_string(),
        serialize_pretty_json_bytes(&build_codex_hook_manifest())?,
    );
    for (slug, body) in build_codex_skill_stubs() {
        files.insert(format!(".codex/skills/{slug}/SKILL.md"), body.into_bytes());
    }
    files.insert(
        CODEX_HOOK_README_PATH.to_string(),
        build_codex_hooks_readme().into_bytes(),
    );
    files.insert(
        HOST_ENTRYPOINT_SYNC_MANIFEST_PATH.to_string(),
        serialize_pretty_json_bytes(&build_host_entrypoint_sync_manifest(&files))?,
    );
    Ok(files)
}

fn build_host_entrypoint_sync_manifest(desired_files: &BTreeMap<String, Vec<u8>>) -> Value {
    let mut retired_paths = retired_host_entrypoint_paths();
    retired_paths.sort();
    let full_text_files = desired_files
        .keys()
        .filter(|path| path.as_str() != HOST_ENTRYPOINT_SYNC_MANIFEST_PATH)
        .filter(|path| !HOST_ENTRYPOINT_JSON_RELATIVE_PATHS.contains(&path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    json!({
        "schema_version": "host-entrypoints-sync-manifest-v1",
        "full_sync": {
            "text_files": full_text_files,
            "json_files": [
                HOST_ENTRYPOINT_JSON_RELATIVE_PATHS[0],
                HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
            ],
            "managed_directories": HOST_ENTRYPOINT_FULL_SYNC_MANAGED_DIRECTORIES,
            "retired_paths": retired_paths,
        },
        "partial_sync": {
            "text_files": HOST_ENTRYPOINT_PARTIAL_SYNC_TEXT_FILES,
            "json_files": HOST_ENTRYPOINT_JSON_RELATIVE_PATHS,
            "managed_directories": HOST_ENTRYPOINT_FULL_SYNC_MANAGED_DIRECTORIES,
            "retired_paths": retired_paths,
        },
    })
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
    for relative in &section.managed_directories {
        let directory = target_root.join(relative);
        if symlink_exists(&directory) && apply {
            remove_path(&directory).map_err(|err| err.to_string())?;
        }
        if !directory.exists() {
            if apply {
                fs::create_dir_all(&directory).map_err(|err| err.to_string())?;
            }
            report.created_dirs.push(describe_host_entrypoint_path(
                report_root,
                target_root,
                &directory,
            ));
        }
    }

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

    for relative in &section.retired_paths {
        let path = target_root.join(relative);
        let exists = path.exists() || symlink_exists(&path);
        if exists && apply {
            remove_path(&path).map_err(|err| err.to_string())?;
        }
        if exists {
            report.written.push(describe_host_entrypoint_path(
                report_root,
                target_root,
                &path,
            ));
        }
    }

    Ok(report)
}

fn retired_host_entrypoint_paths() -> Vec<String> {
    RETIRED_HOST_ENTRYPOINT_PATH_BYTES
        .iter()
        .filter_map(|bytes| std::str::from_utf8(bytes).ok())
        .map(str::to_string)
        .collect()
}

fn protected_generated_paths() -> Vec<&'static str> {
    PROTECTED_GENERATED_PATH_BYTES
        .iter()
        .filter_map(|bytes| std::str::from_utf8(bytes).ok())
        .collect()
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

fn remove_path(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn symlink_exists(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

pub fn build_codex_hook_projection() -> Value {
    json!({
        "schema_version": "router-rs-codex-hook-projection-v1",
        "authority": CODEX_HOOK_AUTHORITY,
        "codex_agent_policy": build_codex_agent_policy(),
        "codex_hooks_readme": build_codex_hooks_readme(),
        "codex_hooks": build_codex_hook_manifest(),
    })
}

fn build_codex_agent_policy() -> String {
    include_str!("../../../AGENTS.md").to_string()
}

fn build_codex_skill_stubs() -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        (
            "autopilot",
            r#"---
name: autopilot
description: Use for `$autopilot`, `autopilot`, full auto, or requests to keep executing end to end until a bounded task is verified.
---

# autopilot

Small Codex entry stub.

Use when the user explicitly asks for `$autopilot` / `autopilot`, full auto, automatic end-to-end execution, or continuing until a concrete task converges.

Do not load the whole skill library. For full workflow details, read only:

`skills/autopilot/SKILL.md`

Default shape:

1. Clarify only if the task is still risky or vague.
2. Plan the smallest executable path.
3. Implement real changes.
4. Verify with evidence.
5. Leave recovery anchors if interrupted.
"#
            .to_string(),
        ),
        (
            "deepinterview",
            r#"---
name: deepinterview
description: Use for `$deepinterview`, deepinterview, deep clarification, convergence review, strict review, or one-question-at-a-time ambiguity reduction.
---

# deepinterview

Small Codex entry stub.

Use when the user explicitly asks for `$deepinterview` / `deepinterview`, deep clarification, strict review, review to convergence, or "do not assume" style questioning.

Do not load the whole skill library. For full workflow details, read only:

`skills/deepinterview/SKILL.md`

Default shape:

1. Check repository evidence before asking.
2. Ask one question at a time.
3. Target the weakest unclear point.
4. Prefer findings first for review.
5. Handoff only when the scope is clear.
"#
            .to_string(),
        ),
        (
            "gitx",
            r#"---
name: gitx
description: Use for `$gitx`, `gitx`, or practical Git closeout work such as status, branch, rebase, worktree, commit, merge, or push.
---

# gitx

Small Codex entry stub.

Use when the user explicitly asks for `$gitx` / `gitx`, Git closeout, branch cleanup, worktree handling, review-fix-commit-merge-push, or push failure triage.

Do not load the whole skill library. For full workflow details, read only:

`skills/gitx/SKILL.md`

Default shape:

1. Inspect real Git state first.
2. Review the intended change surface before committing.
3. Keep user changes safe.
4. Verify the narrow useful slice.
5. Push only with explicit remote and branch.
"#
            .to_string(),
        ),
    ])
}

fn build_codex_hooks_readme() -> String {
    "# Codex Hooks Projection\n\n\
Codex hook config for this repo is generated from `scripts/router-rs/`.\n\n\
Active hooks:\n\n\
| Event | Runner | Purpose |\n\
| --- | --- | --- |\n\
| `PreToolUse` | `router-rs --codex-hook-command pre-tool-use` | Guard Bash writes to generated host outputs before they run. |\n\
\n\
Codex only installs Bash `PreToolUse`; behavior rules live in `AGENTS.md`.\n\n\
Regenerate with:\n\n\
```sh\n\
./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --sync-host-entrypoints-json --repo-root \"$PWD\"\n\
```\n"
        .to_string()
}

fn build_codex_command_hook(event: &str, matcher: &str) -> Value {
    json!({
        "matcher": matcher,
        "hooks": [
            {
                "type": "command",
                "command": build_codex_hook_command(event),
                "timeout": 8,
            }
        ]
    })
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
        "\"$ROUTER_RS_LAUNCHER\" \"$ROUTER_RS_MANIFEST\" --codex-hook-command {event} --repo-root \"$CODEX_PROJECT_ROOT\""
    ));
    command
}

pub fn run_codex_audit_hook(command: &str, repo_root: &Path) -> Result<Option<Value>, String> {
    let canonical = canonical_codex_audit_command(command)?;
    let payload = read_stdin_payload()?;
    match canonical {
        "pre-tool-use" => run_codex_pre_tool_use(repo_root, &payload),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    }
}

fn run_codex_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    run_pre_tool_use(repo_root, payload)
}

fn canonical_codex_audit_command(command: &str) -> Result<&'static str, String> {
    match command {
        "pre-tool-use" => Ok("pre-tool-use"),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    }
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
            return rel.to_string_lossy().replace('\\', "/");
        }
    }
    path.replace('\\', "/")
}

fn classify_protected_generated_path(path: &str) -> Option<&'static str> {
    if protected_generated_paths().contains(&path) {
        return Some("generated_file");
    }
    if PROTECTED_GENERATED_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
    {
        return Some("generated_file");
    }
    None
}

fn pre_tool_use_message(path: &str) -> String {
    format!(
        "[codex-pre-tool-use] blocked direct edits to generated Codex surface {path}; rerun `{}` instead."
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
            if !segment.contains(hint) {
                continue;
            }
            if looks_mutating || bash_segment_redirects_to_hint(&segment, hint) {
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

fn bash_segment_redirects_to_hint(segment: &str, hint: &str) -> bool {
    let escaped = regex::escape(hint);
    [
        format!(r#"(>>?|>\|)\s*['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
        format!(r#"\btee\b(?:\s+-a)?\s+['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
        format!(r#"\bdd\b[^\n;&|]*\bof=['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
    ]
    .iter()
    .any(|pattern| {
        Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(segment))
            .unwrap_or(false)
    })
}
