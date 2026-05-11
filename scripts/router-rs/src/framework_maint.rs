//! `router-rs framework maint …` — replaces retired `scripts/*.sh` maintenance wrappers.
//!
//! `update-one-shot` runs **offline-stable** integration suites by default (policy, docs contracts,
//! Markdown UTF-8 surface, rust_cli_tools, host_integration, browser MCP scripts, codex aggregator).
//! Set `ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS=1` to also run `autoresearch_cli` (network / arXiv).

use crate::cli::args::{
    InstallCodexUserHooksArgs, MaintRepoArgs, MaintRootsArgs, MaintSubcommand, UpdateAuditArgs,
};
use crate::host_integration::{
    cargo_router_rs_executable, resolve_maint_roots, run_host_integration_from_args,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) fn dispatch(command: MaintSubcommand) -> Result<(), String> {
    match command {
        MaintSubcommand::RefreshHostProjections(args) => refresh_host_projections(args),
        MaintSubcommand::VerifyCursorHooks(args) => {
            verify_cursor_hooks(repo_from_maint_repo_args(&args)?)
        }
        MaintSubcommand::VerifyCodexHooks(args) => {
            verify_codex_hooks(repo_from_maint_repo_args(&args)?)
        }
        MaintSubcommand::UpdateOneShot(args) => update_one_shot(args),
        MaintSubcommand::UpdateAudit(args) => update_audit(args),
        MaintSubcommand::CleanRustTargets(args) => {
            let root = repo_from_maint_repo_args(&args)?;
            clean_rust_target_dirs(&root)
        }
        MaintSubcommand::PrintLocalHomes(args) => {
            print_local_homes(repo_from_maint_repo_args(&args)?)
        }
        MaintSubcommand::InstallCodexUserHooks(args) => install_codex_user_hooks(args),
    }
}

fn repo_from_maint_repo_args(args: &MaintRepoArgs) -> Result<PathBuf, String> {
    Ok(resolve_maint_roots(args.framework_root.as_deref(), None)?.0)
}

fn repo_from_update_audit_args(args: &UpdateAuditArgs) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|err| err.to_string())?;
    let candidate = args
        .repo_root
        .as_deref()
        .or(args.framework_root.as_deref())
        .map(Path::to_path_buf)
        .unwrap_or(cwd.clone());
    let candidate = if candidate.is_absolute() {
        candidate
    } else {
        cwd.join(candidate)
    };
    let output = Command::new("git")
        .args([
            "-C",
            candidate.to_string_lossy().as_ref(),
            "rev-parse",
            "--show-toplevel",
        ])
        .output()
        .map_err(|err| format!("git rev-parse spawn failed: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "update-audit requires a git repository root or subdirectory; {} failed git discovery: {}",
            candidate.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return Err("git rev-parse returned an empty repository root".to_string());
    }
    fs::canonicalize(&root).map_err(|err| format!("failed to canonicalize git root {root}: {err}"))
}

fn refresh_host_projections(args: MaintRootsArgs) -> Result<(), String> {
    let (fw, art) = resolve_maint_roots(
        args.framework_root.as_deref(),
        args.artifact_root.as_deref(),
    )?;
    eprintln!("repo_root: {}", fw.display());
    eprintln!("artifact_root: {}", art.display());

    let manifest = fw.join("scripts/router-rs/Cargo.toml");
    run_cargo(
        &fw,
        &[
            "build",
            "--manifest-path",
            manifest.to_string_lossy().as_ref(),
        ],
    )?;

    run_router(
        &fw,
        &[
            "codex",
            "sync",
            "--repo-root",
            fw.to_string_lossy().as_ref(),
        ],
    )?;

    for tool in ["cursor", "claude"] {
        run_router(
            &fw,
            &[
                "framework",
                "host-integration",
                "install",
                "--framework-root",
                fw.to_string_lossy().as_ref(),
                "--project-root",
                fw.to_string_lossy().as_ref(),
                "--artifact-root",
                art.to_string_lossy().as_ref(),
                "--scope",
                "project",
                "--to",
                tool,
            ],
        )?;
    }

    verify_cursor_hooks(fw.clone())?;
    verify_claude_projection(&fw)?;
    eprintln!("ok: refreshed codex + cursor + claude (project-level) projections");
    Ok(())
}

fn verify_cursor_hooks(repo_root: PathBuf) -> Result<(), String> {
    let hooks_json = repo_root.join(".cursor/hooks.json");
    let harness = repo_root.join("configs/framework/HARNESS_OPERATOR_NUDGES.json");
    for path in [&hooks_json, &harness] {
        if !path.is_file() {
            return Err(format!("verify_cursor_hooks: missing {}", path.display()));
        }
    }

    let text = fs::read_to_string(&hooks_json).map_err(|e| e.to_string())?;
    let payload: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let hooks = payload
        .get("hooks")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "verify_cursor_hooks: .cursor/hooks.json must contain a hooks object".to_string()
        })?;

    const REQUIRED_EVENTS: &[&str] = &[
        "beforeSubmitPrompt",
        "stop",
        "sessionStart",
        "sessionEnd",
        "postToolUse",
        "beforeShellExecution",
        "afterShellExecution",
        "afterFileEdit",
        "preCompact",
        "subagentStart",
        "subagentStop",
    ];
    const NEEDLE: &str = "router-rs cursor hook";

    for event in REQUIRED_EVENTS {
        let entries = hooks
            .get(*event)
            .and_then(Value::as_array)
            .filter(|a| !a.is_empty())
            .ok_or_else(|| format!("verify_cursor_hooks: missing hook event {event}"))?;
        let cmds: Vec<&str> = entries
            .iter()
            .filter_map(|entry| entry.get("command").and_then(Value::as_str))
            .collect();
        if cmds.is_empty() {
            return Err(format!(
                "verify_cursor_hooks: event {event} must contain command hooks"
            ));
        }
        if !cmds.iter().any(|c| c.contains(NEEDLE)) {
            return Err(format!(
                "verify_cursor_hooks: {event} must invoke `{NEEDLE}` (see hooks.json)"
            ));
        }
    }

    run_cursor_smoke_session_start(&repo_root)?;
    eprintln!("verify_cursor_hooks: ok");
    Ok(())
}

fn verify_claude_projection(repo_root: &Path) -> Result<(), String> {
    let rule = repo_root.join(".claude/rules/framework.md");
    let manifest = repo_root.join(".claude/.framework-projection.json");
    for path in [&rule, &manifest] {
        if !path.is_file() {
            return Err(format!(
                "verify_claude_projection: missing {}",
                path.display()
            ));
        }
    }
    let rule_text = fs::read_to_string(&rule).map_err(|e| e.to_string())?;
    if !rule_text.contains("host_projection: claude-code") {
        return Err(
            "verify_claude_projection: .claude/rules/framework.md must declare claude-code projection"
                .to_string(),
        );
    }
    let manifest_text = fs::read_to_string(&manifest).map_err(|e| e.to_string())?;
    let manifest_json: Value = serde_json::from_str(&manifest_text).map_err(|e| e.to_string())?;
    if manifest_json.get("host_projection").and_then(Value::as_str) != Some("claude-code") {
        return Err(
            "verify_claude_projection: .claude/.framework-projection.json must declare claude-code"
                .to_string(),
        );
    }
    eprintln!("verify_claude_projection: ok");
    Ok(())
}

fn run_cursor_smoke_session_start(repo_root: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let status = Command::new(&exe)
        .args([
            "cursor",
            "hook",
            "--event=SessionStart",
            "--repo-root",
            &repo_root.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!(
            "verify_cursor_hooks: cursor hook SessionStart smoke failed: {status}"
        ));
    }
    Ok(())
}

fn verify_codex_hooks(repo_root: PathBuf) -> Result<(), String> {
    eprintln!("Verifying Codex hook projection");
    let exe = resolve_router_rs_binary(&repo_root)?;
    for rel in [
        ".codex/config.toml",
        ".codex/hooks.json",
        ".codex/README.md",
        "AGENTS.md",
    ] {
        let p = repo_root.join(rel);
        if !p.is_file() {
            return Err(format!("verify_codex_hooks: missing {}", p.display()));
        }
    }

    let config =
        fs::read_to_string(repo_root.join(".codex/config.toml")).map_err(|e| e.to_string())?;
    if !config.contains("hooks = true") {
        return Err("verify_codex_hooks: .codex/config.toml must enable hooks".into());
    }
    if config.contains("codex_hooks") {
        return Err(
            "verify_codex_hooks: .codex/config.toml must not use deprecated codex_hooks".into(),
        );
    }

    let hooks_text =
        fs::read_to_string(repo_root.join(".codex/hooks.json")).map_err(|e| e.to_string())?;
    for event in [
        "SessionStart",
        "PreToolUse",
        "UserPromptSubmit",
        "PostToolUse",
        "Stop",
    ] {
        if !hooks_text.contains(event) {
            return Err(format!(
                "verify_codex_hooks: missing Codex hook event: {event}"
            ));
        }
    }
    if hooks_text.contains("scripts/codex_hook_entrypoint.sh") {
        return Err(
            "verify_codex_hooks: .codex/hooks.json must call router-rs codex hook directly".into(),
        );
    }

    let readme =
        fs::read_to_string(repo_root.join(".codex/README.md")).map_err(|e| e.to_string())?;
    if hooks_text.contains("sessionEnd") || hooks_text.contains("Kiro") {
        return Err(
            "verify_codex_hooks: Codex hook projection contains stale lifecycle or host wording"
                .into(),
        );
    }
    if readme.contains("sessionEnd") || readme.contains("Kiro") {
        return Err(
            "verify_codex_hooks: Codex README contains stale lifecycle or host wording".into(),
        );
    }
    verify_codex_skill_runtime_health(&repo_root)?;

    codex_hook_smoke(
        &exe,
        &repo_root,
        "SessionStart",
        r#"{"hook_event_name":"SessionStart","session_id":"verify-session","source":"startup"}"#,
    )?;
    codex_hook_smoke(
        &exe,
        &repo_root,
        "PreToolUse",
        r#"{"hook_event_name":"PreToolUse","session_id":"verify-session","tool_name":"functions.exec_command","tool_input":{"cmd":"true"}}"#,
    )?;
    codex_hook_smoke(
        &exe,
        &repo_root,
        "UserPromptSubmit",
        r#"{"hook_event_name":"UserPromptSubmit","session_id":"verify-session","prompt":"review this PR"}"#,
    )?;
    codex_hook_smoke(
        &exe,
        &repo_root,
        "PostToolUse",
        r#"{"hook_event_name":"PostToolUse","session_id":"verify-session","tool_name":"functions.spawn_agent","tool_input":{"agent_type":"explorer"}}"#,
    )?;
    codex_hook_smoke(
        &exe,
        &repo_root,
        "Stop",
        r#"{"hook_event_name":"Stop","session_id":"verify-session","prompt":"review this PR","stop_hook_active":true}"#,
    )?;

    eprintln!("Codex hook projection verified");
    Ok(())
}

fn verify_codex_skill_runtime_health(repo_root: &Path) -> Result<(), String> {
    let repo_runtime = repo_root.join("skills/SKILL_ROUTING_RUNTIME.json");
    verify_runtime_skill_paths(repo_root, &repo_runtime, "repo skill runtime")?;

    let surface_runtime =
        repo_root.join("artifacts/codex-skill-surface/skills/SKILL_ROUTING_RUNTIME.json");
    let surface_paths = if surface_runtime.is_file() {
        let surface_root = surface_runtime
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                format!(
                    "verify_codex_hooks: invalid Codex surface runtime path {}",
                    surface_runtime.display()
                )
            })?;
        Some(verify_runtime_skill_paths(
            surface_root,
            &surface_runtime,
            "Codex skill surface runtime",
        )?)
    } else {
        None
    };

    let codex_home = codex_home_path()?;
    let global_runtime = codex_home.join("skills/SKILL_ROUTING_RUNTIME.json");
    if global_runtime.is_file() {
        let global_paths =
            verify_runtime_skill_paths(&codex_home, &global_runtime, "Codex global skill runtime")?;
        if let Some(surface_paths) = surface_paths {
            if global_paths != surface_paths {
                return Err(format!(
                    "verify_codex_hooks: Codex global skill runtime drifted from generated surface runtime\nsurface_only={:?}\nglobal_only={:?}",
                    surface_paths
                        .difference(&global_paths)
                        .cloned()
                        .collect::<Vec<_>>(),
                    global_paths
                        .difference(&surface_paths)
                        .cloned()
                        .collect::<Vec<_>>()
                ));
            }
        }
    }
    Ok(())
}

fn verify_runtime_skill_paths(
    root: &Path,
    runtime_path: &Path,
    label: &str,
) -> Result<BTreeSet<String>, String> {
    let text = fs::read_to_string(runtime_path).map_err(|err| {
        format!(
            "verify_codex_hooks: failed to read {label} {}: {err}",
            runtime_path.display()
        )
    })?;
    let runtime: Value = serde_json::from_str(&text).map_err(|err| {
        format!(
            "verify_codex_hooks: failed to parse {label} {}: {err}",
            runtime_path.display()
        )
    })?;
    let paths = collect_runtime_skill_paths(&runtime);
    for path in &paths {
        if path.contains("artifacts/codex-skill-surface") {
            return Err(format!(
                "verify_codex_hooks: {label} must not reference generated surface paths: {path}"
            ));
        }
        if !root.join(path).is_file() {
            return Err(format!(
                "verify_codex_hooks: {label} references missing skill_path {path} under {}",
                root.display()
            ));
        }
    }
    Ok(paths)
}

fn collect_runtime_skill_paths(value: &Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    collect_runtime_skill_paths_inner(value, &mut paths);
    paths
}

fn collect_runtime_skill_paths_inner(value: &Value, paths: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "skill_path" {
                    if let Some(path) = child.as_str() {
                        paths.insert(path.to_string());
                    }
                } else if key == "skill_paths" {
                    if let Some(items) = child.as_array() {
                        for path in items.iter().filter_map(Value::as_str) {
                            paths.insert(path.to_string());
                        }
                    }
                } else {
                    collect_runtime_skill_paths_inner(child, paths);
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_runtime_skill_paths_inner(child, paths);
            }
        }
        _ => {}
    }
}

fn codex_hook_smoke(
    exe: &Path,
    repo_root: &Path,
    label: &str,
    json_line: &str,
) -> Result<(), String> {
    let mut child = Command::new(exe)
        .args([
            "codex",
            "hook",
            "--event",
            label,
            "--repo-root",
            &repo_root.to_string_lossy(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(format!("{json_line}\n").as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("verify_codex_hooks: smoke {label} exited {status}"));
    }
    Ok(())
}

fn resolve_router_rs_binary(repo_root: &Path) -> Result<PathBuf, String> {
    if let Ok(p) = std::env::current_exe() {
        if p.file_name()
            .and_then(|s| s.to_str())
            .map(|n| n == "router-rs" || n.contains("router-rs"))
            .unwrap_or(false)
        {
            return Ok(p);
        }
    }
    cargo_router_rs_executable(repo_root)
        .or_else(|| which::which("router-rs").ok())
        .ok_or_else(|| {
            "router-rs binary not found; build with: cargo build --manifest-path scripts/router-rs/Cargo.toml"
                .to_string()
        })
}

fn update_one_shot(args: MaintRootsArgs) -> Result<(), String> {
    let (fw, art) = resolve_maint_roots(
        args.framework_root.as_deref(),
        args.artifact_root.as_deref(),
    )?;
    eprintln!("repo_root={} artifact_root={}", fw.display(), art.display());

    refresh_host_projections(MaintRootsArgs {
        framework_root: Some(fw.clone()),
        artifact_root: Some(art.clone()),
    })?;

    let skill_compiler = fw.join("scripts/skill-compiler-rs/Cargo.toml");
    run_cargo(
        &fw,
        &[
            "run",
            "--manifest-path",
            skill_compiler.to_string_lossy().as_ref(),
            "--",
            "--skills-root",
            "skills",
            "--source-manifest",
            "skills/SKILL_SOURCE_MANIFEST.json",
            "--apply",
        ],
    )?;

    eprintln!("cargo test → integration harness (offline-stable suites; see maint module docs)");
    const DEFAULT_SUITES: &[&str] = &[
        "policy_contracts",
        "documentation_contracts",
        "tracked_markdown_utf8_contract",
        "rust_cli_tools",
        "host_integration",
        "browser_mcp_scripts",
        "codex_aggregator_rustification",
    ];
    for suite in DEFAULT_SUITES {
        run_cargo(&fw, &["test", "--test", suite])?;
    }
    if autoresearch_integration_tests_enabled() {
        eprintln!(
            "ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS → cargo test --test autoresearch_cli"
        );
        run_cargo(&fw, &["test", "--test", "autoresearch_cli"])?;
    }

    eprintln!("cargo test → skill-compiler-rs");
    run_cargo(
        &fw,
        &[
            "test",
            "--manifest-path",
            skill_compiler.to_string_lossy().as_ref(),
        ],
    )?;

    let status_json = run_host_integration_from_args(&[
        "generated-artifacts-status".into(),
        "--framework-root".into(),
        fw.to_string_lossy().into_owned(),
        "--artifact-root".into(),
        art.to_string_lossy().into_owned(),
    ])?;
    if status_json.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(format!(
            "generated-artifacts-status not ok: {}",
            serde_json::to_string(&status_json).unwrap_or_default()
        ));
    }

    if host_skills_publish_enabled() {
        eprintln!(
            "ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS → codex host-integration install-skills install"
        );
        let codex_home = codex_home_path()?;
        let cursor_home = cursor_home_path()?;
        run_router(
            &fw,
            &[
                "codex",
                "host-integration",
                "install-skills",
                "--repo-root",
                fw.to_string_lossy().as_ref(),
                "--artifact-root",
                art.to_string_lossy().as_ref(),
                "--codex-home",
                codex_home.to_string_lossy().as_ref(),
                "--cursor-home",
                cursor_home.to_string_lossy().as_ref(),
                "install",
            ],
        )?;
    }

    eprintln!("ok: framework maint update-one-shot complete");
    Ok(())
}

fn update_audit(args: UpdateAuditArgs) -> Result<(), String> {
    let root = repo_from_update_audit_args(&args)?;
    let tracked = git_lines(&root, &["ls-files"])?;
    let status = git_lines_preserve_leading(
        &root,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let untracked = git_lines(&root, &["ls-files", "-o", "--exclude-standard"])?;
    let ignored_untracked = git_lines(&root, &["ls-files", "-o", "-i", "--exclude-standard"])?;
    let tracked_ignored = git_lines(&root, &["ls-files", "-ci", "--exclude-standard"])?;
    let suspected_dead_code_markers = dead_code_markers(&root, &untracked)?;
    let suspected_stale_docs = stale_doc_markers(&root, &untracked)?;

    let payload = json!({
        "schema_version": "framework-maint-update-audit-v1",
        "repo_root": root,
        "mode": "dry-run",
        "mutates_files": false,
        "key_document_candidates": key_document_candidates(&tracked, &untracked),
        "git_tracking": {
            "status_porcelain": cap_lines(status, 120),
            "untracked_not_ignored": cap_lines(untracked.clone(), 120),
            "ignored_untracked": cap_lines(ignored_untracked, 120),
            "tracked_ignored": cap_lines(tracked_ignored, 120),
            "tracked_suspicious_generated_or_temp": cap_lines(suspicious_tracked_generated(&tracked), 120),
        },
        "suspected_dead_code_markers": cap_lines(suspected_dead_code_markers, 120),
        "suspected_stale_docs": cap_lines(suspected_stale_docs, 120),
        "suspected_retired_files": cap_lines(suspected_retired_files(&tracked), 120),
        "recommended_actions": [
            "Refresh README/AGENTS/docs indexes and research-facing ledgers before cleanup.",
            "Review git_tracking.untracked_not_ignored for files that should be added or ignored.",
            "Remove tracked generated/cache/temp files only after confirming they are not source artifacts.",
            "Treat suspected_dead_code_markers and suspected_stale_docs as an inventory, not proof of deletion.",
            "Do not delete research data, manuscripts, experiment logs, or citation stores without explicit evidence."
        ]
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn git_lines(repo_root: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    git_lines_with_trim(repo_root, args, true)
}

fn git_lines_preserve_leading(repo_root: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    git_lines_with_trim(repo_root, args, false)
}

fn git_lines_with_trim(
    repo_root: &Path,
    args: &[&str],
    trim_leading: bool,
) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("git {} spawn failed: {e}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let lines = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| {
            if trim_leading {
                line.trim().to_string()
            } else {
                line.trim_end().to_string()
            }
        })
        .filter(|line| !line.is_empty())
        .collect();
    Ok(lines)
}

fn git_grep_lines(
    repo_root: &Path,
    pattern: &str,
    pathspecs: &[&str],
) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("grep")
        .arg("-n")
        .arg("-E")
        .arg(pattern)
        .arg("--")
        .args(pathspecs)
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("git grep spawn failed: {e}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect());
    }
    if output.status.code() == Some(1) {
        return Ok(Vec::new());
    }
    Err(format!(
        "git grep failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn key_document_candidates(tracked: &[String], untracked: &[String]) -> Vec<Value> {
    let mut out = Vec::new();
    for path in tracked {
        if is_key_document_path(path) {
            out.push(json!({"path": path, "tracking": "tracked"}));
        }
    }
    for path in untracked {
        if is_key_document_path(path) {
            out.push(json!({"path": path, "tracking": "untracked"}));
        }
    }
    cap_values(out, 160)
}

fn is_key_document_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let is_root_doc = matches!(
        path,
        "README.md" | "AGENTS.md" | "RTK.md" | "docs/README.md"
    );
    let is_research_doc = lower.contains("research")
        || lower.contains("paper")
        || lower.contains("experiment")
        || lower.contains("reproduc")
        || lower.contains("citation")
        || lower.contains("literature")
        || lower.contains("method")
        || lower.contains("result")
        || lower.ends_with(".bib")
        || lower.ends_with(".tex")
        || lower.ends_with(".ipynb");
    let is_plan_doc = lower.starts_with("docs/plans/") || lower.contains("plan");
    is_root_doc || ((is_research_doc || is_plan_doc) && is_document_like_path(&lower))
}

fn is_document_like_path(lower: &str) -> bool {
    matches!(
        Path::new(lower).extension().and_then(|ext| ext.to_str()),
        Some("md" | "mdx" | "txt" | "tex" | "bib" | "ipynb" | "csv" | "tsv" | "json")
    )
}

fn dead_code_markers(repo_root: &Path, untracked: &[String]) -> Result<Vec<String>, String> {
    let mut markers = git_grep_lines(
        repo_root,
        r"(allow\(dead_code\)|dead code|unused|obsolete|deprecated|retired)",
        &["*.rs", "*.py", "*.ts", "*.tsx", "*.js", "*.jsx", "*.md"],
    )?;
    markers.extend(untracked_keyword_markers(
        repo_root,
        untracked,
        &[
            "allow(dead_code)",
            "dead code",
            "unused",
            "obsolete",
            "deprecated",
            "retired",
        ],
        is_code_or_markdown_path,
    )?);
    Ok(markers)
}

fn stale_doc_markers(repo_root: &Path, untracked: &[String]) -> Result<Vec<String>, String> {
    let mut markers = git_grep_lines(
        repo_root,
        r"(stale|obsolete|deprecated|retired|outdated|TODO|FIXME|旧|废弃|过期)",
        &["*.md"],
    )?;
    markers.extend(untracked_keyword_markers(
        repo_root,
        untracked,
        &[
            "stale",
            "obsolete",
            "deprecated",
            "retired",
            "outdated",
            "TODO",
            "FIXME",
            "旧",
            "废弃",
            "过期",
        ],
        |lower| {
            matches!(
                Path::new(lower).extension().and_then(|ext| ext.to_str()),
                Some("md" | "mdx" | "txt")
            )
        },
    )?);
    Ok(markers)
}

fn suspected_retired_files(tracked: &[String]) -> Vec<String> {
    tracked
        .iter()
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.contains("/history/")
                || lower.contains("deprecated")
                || lower.contains("retired")
                || lower.contains("obsolete")
                || lower.contains("legacy")
                || lower.contains("stale")
                || lower.contains("backup")
                || lower.ends_with(".bak")
                || lower.ends_with(".old")
        })
        .cloned()
        .collect()
}

fn suspicious_tracked_generated(tracked: &[String]) -> Vec<String> {
    tracked
        .iter()
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.contains("/target/")
                || lower.contains("/node_modules/")
                || lower.contains("/__pycache__/")
                || lower.ends_with(".tmp")
                || lower.ends_with(".temp")
                || lower.ends_with(".log")
                || lower.ends_with(".pyc")
                || lower.ends_with(".swp")
                || lower.ends_with(".dSYM")
                || lower.ends_with(".bak")
        })
        .cloned()
        .collect()
}

fn is_code_or_markdown_path(lower: &str) -> bool {
    matches!(
        Path::new(lower).extension().and_then(|ext| ext.to_str()),
        Some("rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "md" | "mdx")
    )
}

fn untracked_keyword_markers(
    repo_root: &Path,
    untracked: &[String],
    keywords: &[&str],
    include_path: impl Fn(&str) -> bool,
) -> Result<Vec<String>, String> {
    let mut markers = Vec::new();
    let lowercase_keywords: Vec<String> = keywords
        .iter()
        .map(|keyword| keyword.to_ascii_lowercase())
        .collect();
    for path in untracked {
        let lower_path = path.to_ascii_lowercase();
        if !include_path(&lower_path) {
            continue;
        }
        let full_path = repo_root.join(path);
        if !full_path.is_file() {
            continue;
        }
        let Ok(text) = fs::read_to_string(&full_path) else {
            continue;
        };
        for (idx, line) in text.lines().enumerate() {
            let lower_line = line.to_ascii_lowercase();
            if lowercase_keywords
                .iter()
                .any(|keyword| lower_line.contains(keyword))
            {
                let snippet: String = line.chars().take(200).collect();
                markers.push(format!("{path}:{}:{snippet}", idx + 1));
            }
        }
    }
    Ok(markers)
}

fn cap_lines(mut lines: Vec<String>, max: usize) -> Vec<String> {
    if lines.len() > max {
        lines.truncate(max);
        lines.push(format!("... truncated at {max} entries"));
    }
    lines
}

fn cap_values(mut values: Vec<Value>, max: usize) -> Vec<Value> {
    if values.len() > max {
        values.truncate(max);
        values.push(json!({"truncated": true, "max_entries": max}));
    }
    values
}

fn codex_home_path() -> Result<PathBuf, String> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))
        .ok_or_else(|| "CODEX_HOME or HOME must be set for host skill publish".to_string())
}

fn cursor_home_path() -> Result<PathBuf, String> {
    std::env::var_os("CURSOR_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cursor")))
        .ok_or_else(|| "CURSOR_HOME or HOME must be set for host skill publish".to_string())
}

fn autoresearch_integration_tests_enabled() -> bool {
    std::env::var("ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS")
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            matches!(t.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn host_skills_publish_enabled() -> bool {
    std::env::var("ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS")
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            matches!(t.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn print_local_homes(fw: PathBuf) -> Result<(), String> {
    let codex = fw.join(".local/codex-home");
    let cursor = fw.join(".local/cursor-home");
    fs::create_dir_all(&codex).map_err(|e| e.to_string())?;
    fs::create_dir_all(&cursor).map_err(|e| e.to_string())?;
    println!("export CODEX_HOME={}", codex.display());
    println!("export CURSOR_HOME={}", cursor.display());
    println!(
        "# note: GUI apps may need launching from this shell to inherit CODEX_HOME / CURSOR_HOME"
    );
    Ok(())
}

fn install_codex_user_hooks(args: InstallCodexUserHooksArgs) -> Result<(), String> {
    let fw = resolve_maint_roots(args.framework_root.as_deref(), None)?.0;
    let manifest = fw.join("scripts/router-rs/Cargo.toml");

    // Prefer existing dev binary (typically already built before `maint install-*` nested under
    // `cargo test`). Avoid unconditional `--release`; it contends forever on Cargo package locks.
    let bin = match cargo_router_rs_executable(&fw) {
        Some(p) if p.is_file() => p,
        _ => {
            eprintln!("Building router-rs (dev) for codex hook install...");
            run_cargo(
                &fw,
                &[
                    "build",
                    "--manifest-path",
                    manifest.to_string_lossy().as_ref(),
                ],
            )?;
            cargo_router_rs_executable(&fw).ok_or_else(|| {
                "router-rs binary missing after dev build (check cargo metadata target_directory)"
                    .to_string()
            })?
        }
    };
    let codex_home = match args.codex_home.clone() {
        Some(p) => p,
        None => std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".codex"))
            .ok_or_else(|| "HOME not set; pass --codex-home".to_string())?,
    };
    fs::create_dir_all(&codex_home).map_err(|e| e.to_string())?;
    let status = Command::new(&bin)
        .args([
            "codex",
            "install-hooks",
            "--codex-home",
            codex_home.to_string_lossy().as_ref(),
            "--apply",
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("codex install-hooks failed: {status}"));
    }
    eprintln!(
        "Installed codex-cli hooks into {}\n- {}\n- {}",
        codex_home.display(),
        codex_home.join("config.toml").display(),
        codex_home.join("hooks.json").display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn fresh_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "framework-maint-{label}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    #[test]
    fn runtime_path_check_rejects_missing_skill_path() {
        let root = fresh_root("missing-skill-path");
        let runtime = root.join("skills/SKILL_ROUTING_RUNTIME.json");
        write(
            &runtime,
            r#"{"records":[{"slug":"x","skill_path":"skills/missing/SKILL.md"}]}"#,
        );
        let err = verify_runtime_skill_paths(&root, &runtime, "test runtime").unwrap_err();
        assert!(err.contains("missing skill_path"), "{err}");
    }

    #[test]
    fn runtime_path_check_rejects_generated_surface_paths() {
        let root = fresh_root("surface-path");
        let runtime = root.join("skills/SKILL_ROUTING_RUNTIME.json");
        write(
            &runtime,
            r#"{"records":[{"slug":"x","skill_path":"artifacts/codex-skill-surface/skills/x/SKILL.md"}]}"#,
        );
        let err = verify_runtime_skill_paths(&root, &runtime, "test runtime").unwrap_err();
        assert!(
            err.contains("must not reference generated surface paths"),
            "{err}"
        );
    }
}

fn clean_rust_target_dirs(repo_root: &Path) -> Result<(), String> {
    clean_targets_walk(repo_root)?;
    Ok(())
}

fn clean_targets_walk(path: &Path) -> Result<(), String> {
    if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
        return Ok(());
    }
    if path.file_name().and_then(|n| n.to_str()) == Some("target") && path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())?;
        return Ok(());
    }
    if path.is_dir() {
        let read = fs::read_dir(path).map_err(|e| e.to_string())?;
        for ent in read {
            clean_targets_walk(&ent.map_err(|e| e.to_string())?.path())?;
        }
    }
    Ok(())
}

fn run_cargo(repo_root: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new("cargo")
        .args(args)
        .current_dir(repo_root)
        .status()
        .map_err(|e| format!("cargo spawn failed: {e}"))?;
    if !status.success() {
        return Err(format!("cargo failed with {status}"));
    }
    Ok(())
}

fn run_router(repo_root: &Path, args: &[&str]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let status = Command::new(&exe)
        .args(args)
        .current_dir(repo_root)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("router-rs {} failed: {status}", args.join(" ")));
    }
    Ok(())
}
