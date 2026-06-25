//! `router-rs framework maint …` — replaces retired `scripts/*.sh` maintenance wrappers.
//!
//! `update-one-shot` runs **offline-stable** integration suites by default (policy, docs contracts,
//! Markdown UTF-8 surface, rust_cli_tools, host_integration, browser MCP scripts, codex aggregator).
//! Set `ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS=1` to also run `autoresearch_cli` (network / arXiv).

use framework_kernel::cli_args::{
    MaintRepoArgs, MaintRootsArgs, MaintSubcommand, UpdateAuditArgs,
};
use host_projection::host_integration::{
    resolve_maint_roots, run_host_integration_from_args,
};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "clean.rs"] mod clean;

pub fn dispatch(command: MaintSubcommand) -> Result<(), String> {
    match command {
        MaintSubcommand::RefreshHostProjections(args) => refresh_host_projections(args),
        MaintSubcommand::VerifyHostHooks { host_id, args } => {
            host_projection::hosts::host_extensions::schema_drift::verify_host_projection(
                &repo_from_maint_repo_args(&args)?,
                &host_id,
            )
        }
        MaintSubcommand::UpdateOneShot(args) => update_one_shot(args),
        MaintSubcommand::UpdateAudit(args) => update_audit(args),
        MaintSubcommand::CleanRustTargets(args) => {
            let root = repo_from_maint_repo_args(&args)?;
            clean::clean_rust_target_dirs(&root, args.dry_run)
        }
        MaintSubcommand::PrintLocalHomes(args) => {
            print_local_homes(repo_from_maint_repo_args(&args)?)
        }
        MaintSubcommand::ContinuityAudit(args) => {
            let root = repo_from_maint_repo_args(&args)?;
            framework_extra::framework_doctor::run_continuity_audit(&root).map(|_| ())
        }
        MaintSubcommand::CleanHookState(args) => {
            let root = repo_from_framework_root_arg(args.framework_root.as_deref())?;
            let dry_run = args.dry_run;
            let ttl_days = args.older_than_days.unwrap_or(7);
            clean::clean_hook_state_files(&root, dry_run, ttl_days)
        }
        MaintSubcommand::CleanOrphans(args) => {
            let root = repo_from_framework_root_arg(args.framework_root.as_deref())?;
            let dry_run = args.dry_run;
            let ttl_days = args.older_than_days.unwrap_or(30);
            clean::clean_orphan_directories(&root, dry_run, ttl_days)
        }
    }
}

fn repo_from_maint_repo_args(args: &MaintRepoArgs) -> Result<PathBuf, String> {
    Ok(resolve_maint_roots(args.framework_root.as_deref(), None)?.0)
}

fn repo_from_framework_root_arg(framework_root: Option<&Path>) -> Result<PathBuf, String> {
    Ok(resolve_maint_roots(framework_root, None)?.0)
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

    let manifest = fw.join("core/router-rs/Cargo.toml");
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
            "framework",
            "sync-entrypoints",
            "--repo-root",
            fw.to_string_lossy().as_ref(),
        ],
    )?;

    let installable_tools = installable_projection_tools(&fw)?;
    // All installable hosts use projection install — codex included.
    for tool in &installable_tools {
        for scope in projection_install_scopes_for_tool(tool) {
            let mut install_args = vec![
                "framework".to_string(),
                "host-integration".to_string(),
                "install".to_string(),
                "--framework-root".to_string(),
                fw.to_string_lossy().into_owned(),
                "--project-root".to_string(),
                fw.to_string_lossy().into_owned(),
                "--artifact-root".to_string(),
                art.to_string_lossy().into_owned(),
                "--scope".to_string(),
                scope.to_string(),
                "--to".to_string(),
                tool.clone(),
            ];
            if let Some(home) = args.home.as_ref() {
                install_args.push("--home".to_string());
                install_args.push(home.to_string_lossy().into_owned());
            }
            let install_refs: Vec<&str> = install_args.iter().map(String::as_str).collect();
            run_router(&fw, &install_refs)?;
        }
    }

    verify_installable_projections(&fw, &installable_tools)?;
    // Verify all installable hosts
    for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
        host_projection::hosts::host_extensions::schema_drift::verify_host_projection(&fw, host_id)?;
    }
    eprintln!(
        "ok: refreshed installable host projections (cursor=user; claude=project+user; others=project): {}",
        installable_tools.join(", ")
    );
    Ok(())
}

/// Host-integration install scopes per tool (aligns Claude user `~/.claude/rules/framework.md` with Cursor user `framework.mdc`).
fn maint_skip_user_projection() -> bool {
    std::env::var_os("ROUTER_RS_MAINT_SKIP_USER_PROJECTION").is_some()
}

/// Install scopes per tool from RUNTIME_REGISTRY.json host_targets.metadata.*.install_scopes.
/// Fallback: ["project"].
fn projection_install_scopes_for_tool(tool: &str) -> Vec<&'static str> {
    let scopes = framework_kernel::runtime_registry::install_scopes(tool);
    if scopes.is_empty() {
        return vec!["project"];
    }
    // Special case: skip user scope if env var is set
    if scopes.contains(&"user") && maint_skip_user_projection() {
        return vec!["project"];
    }
    scopes.to_vec()
}

fn installable_projection_tools(repo_root: &Path) -> Result<Vec<String>, String> {
    let pairs = framework_kernel::framework_host_targets::installable_host_id_and_skills_install_tool_pairs(
        repo_root,
    )?;
    let mut tools = Vec::new();
    for (_host_id, tool) in pairs {
        if !tools.contains(&tool) {
            tools.push(tool);
        }
    }
    Ok(tools)
}

/// Verify host projections using the unified registry-driven verifier.
/// All host-specific data (hooks path, events, launcher) is derived from
/// the HostProvider registry via `verify_host_projection`.
fn verify_installable_projections(repo_root: &Path, tools: &[String]) -> Result<(), String> {
    for tool in tools {
        // tool is an install_tool name (e.g. "cursor", "claude").
        // Resolve to host_id via registry, then verify.
        let host_id = framework_kernel::framework_host_targets::installable_host_id_and_skills_install_tool_pairs(repo_root)?
            .iter()
            .find(|(_, t)| t == tool)
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| tool.clone());
        host_projection::hosts::host_extensions::schema_drift::verify_host_projection(
            repo_root,
            &host_id,
        )?;
    }
    Ok(())
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
        home: args.home.clone(),
    })?;

    let router_manifest = fw.join("core/router-rs/Cargo.toml");
    run_cargo(
        &fw,
        &[
            "run",
            "--quiet",
            "--manifest-path",
            router_manifest.to_string_lossy().as_ref(),
            "--",
            "framework",
            "skills",
            "refresh",
            "--framework-root",
            &fw.to_string_lossy(),
            "--write",
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
            "ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS → host-integration install-skills + user projections"
        );
        // Build host-home args dynamically from registry
        let mut host_home_args: Vec<String> = Vec::new();
        for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
            let home = host_home_path(host_id)?;
            host_home_args.push(format!("--{host_id}-home"));
            host_home_args.push(home.to_string_lossy().into_owned());
        }
        let _host_home_arg_refs: Vec<&str> = host_home_args.iter().map(String::as_str).collect();
        let mut skill_args = vec![
            "framework".to_string(),
            "host-integration".to_string(),
            "install-skills".to_string(),
            "--framework-root".to_string(),
            fw.to_string_lossy().into_owned(),
            "--project-root".to_string(),
            fw.to_string_lossy().into_owned(),
            "--artifact-root".to_string(),
            art.to_string_lossy().into_owned(),
        ];
        skill_args.extend(host_home_args.clone());
        skill_args.push("install".to_string());
        let skill_arg_refs: Vec<&str> = skill_args.iter().map(String::as_str).collect();
        run_router(&fw, &skill_arg_refs)?;

        // Install host-specific projections for all hosts
        for tool in framework_kernel::runtime_registry::ALL_HOST_IDS {
            let home = host_home_path(tool)?;
            for scope in projection_install_scopes_for_tool(tool) {
                let home_flag = format!("--{tool}-home");
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
                        &home_flag,
                        home.to_string_lossy().as_ref(),
                        "--scope",
                        scope,
                        "--to",
                        tool,
                    ],
                )?;
            }
        }
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
        "README.md"
            | "AGENTS.md"
            | "docs/README.md"
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
    let is_plan_doc = lower.contains("plan");
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
        let Ok(full_path) = core_state_utils::path_guard::join_repo_relative_under_root(repo_root, path)
        else {
            continue;
        };
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

// host_home_path(host_id) is the single generic entry point (above).

/// Generic host home path resolution: checks `$HOST_HOME` env var,
/// falls back to `$HOME/<config_dir>` from RUNTIME_REGISTRY.json.
fn host_home_path(host_id: &str) -> Result<PathBuf, String> {
    let env_var = framework_kernel::runtime_registry::home_env_var(host_id);
    if !env_var.is_empty()
        && let Some(path) = std::env::var_os(env_var) {
            return Ok(PathBuf::from(path));
        }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(framework_kernel::runtime_registry::host_private_config_dir(host_id)))
        .ok_or_else(|| format!("{env_var} or HOME must be set for host skill publish"))
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
    for host_dir in framework_kernel::runtime_registry::host_home_dirs() {
        let host_id = host_dir.trim_start_matches('.');
        let local = fw.join(format!(".local/{host_id}-home"));
        fs::create_dir_all(&local).map_err(|e| e.to_string())?;
        let env_var = framework_kernel::runtime_registry::home_env_var(host_id);
        let home = std::env::var_os(env_var)
            .map(PathBuf::from)
            .unwrap_or_else(|| local.clone());
        println!("export {env_var}={}", home.display());
    }
    println!(
        "# note: GUI apps may need launching from this shell to inherit host HOME vars"
    );
    println!(
        "# Claude Desktop MCP (macOS): ~/Library/Application Support/Claude/claude_desktop_config.json"
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_key_document_path ──

    #[test]
    fn key_doc_root_files() {
        assert!(is_key_document_path("README.md"));
        assert!(is_key_document_path("AGENTS.md"));
    }

    #[test]
    fn key_doc_research_md() {
        assert!(is_key_document_path("docs/research-notes.md"));
        assert!(is_key_document_path("experiments/methodology.tex"));
    }

    #[test]
    fn key_doc_non_doc_extension() {
        assert!(!is_key_document_path("src/main.rs"));
        assert!(!is_key_document_path("research-script.py"));
    }

    #[test]
    fn key_doc_bib_tex_ipynb() {
        assert!(is_key_document_path("references.bib"));
        assert!(is_key_document_path("paper/main.tex"));
        assert!(is_key_document_path("analysis.ipynb"));
    }

    // ── is_document_like_path ──

    #[test]
    fn doc_like_md_txt_json() {
        assert!(is_document_like_path("readme.md"));
        assert!(is_document_like_path("notes.txt"));
        assert!(is_document_like_path("config.json"));
    }

    #[test]
    fn doc_like_rs_py() {
        assert!(!is_document_like_path("main.rs"));
        assert!(!is_document_like_path("script.py"));
    }

    // ── is_code_or_markdown_path ──

    #[test]
    fn code_or_md_rs_md() {
        assert!(is_code_or_markdown_path("src/lib.rs"));
        assert!(is_code_or_markdown_path("README.md"));
    }

    #[test]
    fn code_or_md_non_code() {
        assert!(!is_code_or_markdown_path("image.png"));
    }

    // ── cap_lines ──

    #[test]
    fn cap_lines_under_limit() {
        let lines = vec!["a".into(), "b".into()];
        assert_eq!(cap_lines(lines.clone(), 5), lines);
    }

    #[test]
    fn cap_lines_at_limit() {
        let lines: Vec<String> = (0..3).map(|i| format!("line {i}")).collect();
        assert_eq!(cap_lines(lines.clone(), 3), lines);
    }

    #[test]
    fn cap_lines_over_limit() {
        let lines: Vec<String> = (0..5).map(|i| format!("line {i}")).collect();
        let capped = cap_lines(lines, 3);
        assert_eq!(capped.len(), 4); // 3 lines + truncation message
        assert!(capped[3].contains("truncated at 3"));
    }

    // ── cap_values ──

    #[test]
    fn cap_values_under_limit() {
        let vals = vec![json!(1), json!(2)];
        assert_eq!(cap_values(vals.clone(), 5), vals);
    }

    #[test]
    fn cap_values_over_limit() {
        let vals: Vec<Value> = (0..5).map(|i| json!(i)).collect();
        let capped = cap_values(vals, 3);
        assert_eq!(capped.len(), 4);
        assert_eq!(capped[3]["truncated"], true);
        assert_eq!(capped[3]["max_entries"], 3);
    }

    // ── host home dirs consistency ──

    #[test]
    fn host_home_dirs_match_registry() {
        let registry = framework_kernel::runtime_registry::host_home_dirs();
        let all_known = framework_kernel::runtime_registry::ALL_KNOWN_HOST_DIRS;
        for host in registry {
            assert!(
                all_known.contains(host),
                "{host} in host_home_dirs() but not in ALL_KNOWN_HOST_DIRS"
            );
        }
    }
}

