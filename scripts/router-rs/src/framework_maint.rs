//! `router-rs framework maint …` — replaces retired `scripts/*.sh` maintenance wrappers.
//!
//! `update-one-shot` runs **offline-stable** integration suites by default (policy, docs contracts,
//! Markdown UTF-8 surface, rust_cli_tools, host_integration, browser MCP scripts, codex aggregator).
//! Set `ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS=1` to also run `autoresearch_cli` (network / arXiv).

use crate::cli::args::{InstallCodexUserHooksArgs, MaintRepoArgs, MaintRootsArgs, MaintSubcommand};
use crate::host_integration::{
    cargo_router_rs_executable, resolve_maint_roots, run_host_integration_from_args,
};
use serde_json::Value;
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
            "cursor",
        ],
    )?;

    verify_cursor_hooks(fw.clone())?;
    eprintln!("ok: refreshed codex + cursor (project-level) projections");
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
