mod review_gate_lanes;

#[allow(unused_imports)]
pub use review_gate_lanes::{assert_reviewer_lanes_closed, reviewer_lanes_from_registry};

use serde_json::{Value, json};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn pick_router_rs_under_target_dir(base: &Path) -> Option<PathBuf> {
    [
        base.join("debug/router-rs-cli"),
        base.join("release/router-rs-cli"),
        base.join("debug/router-rs"),
        base.join("release/router-rs"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file() && !is_redirect_shim(candidate))
}

/// Align with `host_integration::cargo_router_rs_executable`: same `cargo metadata` target dir
/// as the running `router-rs` uses for MCP payload generation (avoids stale `target-dir` picks).
/// Same shape as `host_integration::cursor_mcp_server_payload` for pre-seeding `mcp.json` in
/// tests (matches `cargo_router_rs_executable` + `which::which(\"router-rs\")` fallback).
pub fn browser_mcp_server_payload_like_host(framework_root: &Path) -> Value {
    let manifest = framework_root.join("core/router-rs/Cargo.toml");
    let from_metadata = if manifest.is_file() {
        let output = Command::new("cargo")
            .current_dir(framework_root)
            .args([
                "metadata",
                "--no-deps",
                "--format-version",
                "1",
                "--manifest-path",
            ])
            .arg(&manifest)
            .output()
            .ok();
        output
            .filter(|o| o.status.success())
            .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
            .and_then(|meta| {
                meta.get("target_directory")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
            })
            .and_then(|td| pick_router_rs_under_target_dir(&td))
    } else {
        None
    };
    let exe = from_metadata.or_else(|| which::which("router-rs").ok());
    let args = vec![
        json!("browser"),
        json!("mcp-stdio"),
        json!("--repo-root"),
        json!(framework_root.to_string_lossy()),
    ];
    match exe {
        Some(path) => json!({
            "command": path.to_string_lossy().to_string(),
            "args": args,
        }),
        None => json!({
            "command": "router-rs",
            "args": args,
        }),
    }
}

fn router_rs_binary_via_cargo_metadata(repo_root: &Path) -> Option<PathBuf> {
    let manifest = repo_root.join("core/router-rs/Cargo.toml");
    if !manifest.is_file() {
        return None;
    }
    let output = Command::new("cargo")
        .current_dir(repo_root)
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let meta: Value = serde_json::from_slice(&output.stdout).ok()?;
    let td = meta.get("target_directory")?.as_str()?;
    pick_router_rs_under_target_dir(&PathBuf::from(td))
}
pub fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Closed-set host ids (2026-06). Order matches `RUNTIME_REGISTRY.json` → `host_targets.supported`.
pub const CANONICAL_HOST_IDS: &[&str] = &["cursor", "claude", "opencode", "codex"];

/// Retired host ids that must not appear in registry `supported` or `metadata`.
pub const RETIRED_HOST_IDS: &[&str] = &[
    "codex-app",
    "codex-cli",
    "claude-desktop",
    "antigravity-cli",
    "antigravity-app",
    "antigravity",
];

/// Assert `supported` is exactly the five canonical host ids (set equality).
pub fn assert_canonical_closed_set_host_ids(supported: &[&str]) {
    assert_eq!(
        supported.len(),
        CANONICAL_HOST_IDS.len(),
        "host_targets.supported must list exactly {} ids, got {}: {supported:?}",
        CANONICAL_HOST_IDS.len(),
        supported.len()
    );
    for id in CANONICAL_HOST_IDS {
        assert!(
            supported.contains(id),
            "host_targets.supported missing canonical id `{id}`: {supported:?}"
        );
    }
    for retired in RETIRED_HOST_IDS {
        assert!(
            !supported.contains(retired),
            "retired host `{retired}` must not be in host_targets.supported"
        );
    }
}

/// Verify `CANONICAL_HOST_IDS` matches `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`.
#[test]
fn canonical_host_ids_match_runtime_registry() {
    let root = project_root();
    let registry_path = root.join("configs/framework/RUNTIME_REGISTRY.json");
    let text = fs::read_to_string(&registry_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", registry_path.display()));
    let json: Value = serde_json::from_str(&text).expect("parse RUNTIME_REGISTRY.json");
    let supported: Vec<&str> = json
        .get("host_targets")
        .and_then(|ht| ht.get("supported"))
        .and_then(|s| s.as_array())
        .expect("host_targets.supported must be an array")
        .iter()
        .map(|v| v.as_str().expect("supported id must be string"))
        .collect();
    assert_canonical_closed_set_host_ids(&supported);
}

pub fn write_text(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|err| {
            panic!("failed to create {}: {err}", parent.display());
        });
    }
    fs::write(path, content).unwrap_or_else(|err| {
        panic!("failed to write {}: {err}", path.display());
    });
}

pub fn seed_framework_markers(root: &Path) {
    write_text(
        &root.join("configs/framework/RUNTIME_REGISTRY.json"),
        r#"{"schema_version": "framework-runtime-registry-v2","framework_core": {"authority": "rust","source": "framework-root-native","host_policy": "closed-set-explicit-projections"},"host_targets": {"policy": "shared-rust-core-explicit-host-projections","supported": ["cursor","claude","opencode","codex"],"shared_system_source": "skills","metadata": {"codex": {"install_tool": "codex","projection_status": "implemented","installable": true,"host_entrypoints": "AGENTS.md"},"cursor": {"install_tool": "cursor","projection_status": "implemented","installable": true,"host_entrypoints": ["AGENTS.md",".cursor/rules/*.mdc"]},"claude": {"install_tool": "claude","projection_status": "implemented","installable": true,"host_entrypoints": ["AGENTS.md",".claude/rules/framework.md",".claude/settings.json"]},"opencode": {"install_tool": "opencode","projection_status": "implemented","installable": true,"host_entrypoints": ".opencode/opencode.json"}},"host_providers": {"cursor": {"cargo_feature": "host-cursor","provider_module": "cursor_provider","provider_type": "CursorHostProvider","hooks_module": "cursor_hooks","cli_hook_subcommand": "hook","dispatch_fn": "dispatch_cursor_command"},"claude": {"cargo_feature": "host-claude","provider_module": "claude_provider","provider_type": "ClaudeHostProvider","hooks_module": "claude_hooks","cli_hook_subcommand": "hook","dispatch_fn": "dispatch_claude_command"},"opencode": {"cargo_feature": "host-opencode","provider_module": "opencode_provider","provider_type": "OpencodeHostProvider","hooks_module": "opencode_agent","cli_agent_subcommand": "agent","dispatch_fn": "dispatch_opencode_command"},"codex": {"cargo_feature": "host-codex","provider_module": "codex_provider","provider_type": "CodexHostProvider","hooks_module": "codex_hooks","cli_hook_subcommand": "hook","dispatch_fn": "dispatch_codex_command"}}},"managed_mcp_servers": {"router-rs-framework": {"server_id": "router-rs-framework"},"browser-mcp": {"server_id": "browser-mcp"},"mcp-codegraph": {"server_id": "mcp-codegraph"},"paperplain": {"server_id": "paperplain"}},"host_projections": {"codex": {"profile_id": "codex_profile","host_id": "codex","transport": "native-codex","session_supervisor_driver": "codex_driver","managed_mcp_server_ids": ["router-rs-framework","browser-mcp","mcp-codegraph","paperplain"],"harness_capabilities": ["hot_runtime_routing","l2_continuity_contract","closeout_evidence_hooks","review_gate_router_observation"],"capabilities": ["artifact_contract","mcp_servers","workspace_bootstrap","batch_execution","cron_execution","ci_runner","non_interactive_entrypoint","external_session_supervisor","rate_limit_auto_resume","host_resume_entrypoint","framework_alias_entrypoints"]},"cursor": {"profile_id": "cursor_profile","host_id": "cursor","transport": "cursor-agent","session_supervisor_driver": "unsupported","managed_mcp_server_ids": ["router-rs-framework","browser-mcp","mcp-codegraph","paperplain"],"session_supervisor_status": {"supported": false,"rationale": "Cursor host does not expose external process supervision or auto-resume."},"harness_capabilities": ["hot_runtime_routing","l2_continuity_contract","closeout_evidence_hooks","review_gate_router_observation"],"capabilities": ["artifact_contract","mcp_servers","workspace_bootstrap","interactive_agent_chat","host_resume_entrypoint","framework_alias_entrypoints"]},"claude": {"profile_id": "claude_profile","host_id": "claude","transport": "anthropic-claude","session_supervisor_driver": "unsupported","managed_mcp_server_ids": ["router-rs-framework","browser-mcp","mcp-codegraph","paperplain"],"harness_capabilities": ["hot_runtime_routing","l2_continuity_contract","closeout_evidence_hooks","review_gate_router_observation"],"capabilities": ["artifact_contract","workspace_bootstrap","interactive_agent_chat","framework_alias_entrypoints","hard_gate_hooks"]},"opencode": {"profile_id": "opencode_profile","host_id": "opencode","transport": "native-opencode","session_supervisor_driver": "unsupported","managed_mcp_server_ids": ["router-rs-framework","browser-mcp","mcp-codegraph","paperplain"],"session_supervisor_status": {"supported": false,"rationale": "Opencode host does not expose external process supervision or auto-resume."},"harness_capabilities": ["hot_runtime_routing","l2_continuity_contract","closeout_evidence_hooks","review_gate_router_observation"],"capabilities": ["artifact_contract","mcp_servers","workspace_bootstrap","interactive_agent_chat","framework_alias_entrypoints"]}}}"#,
    );
    write_text(
        &root.join("configs/framework/host_projection_narrative.json"),
        r#"{"schema_version":"framework-host-projection-narrative-v2","default_lifecycle_paragraph":"My lifecycle (test seed).","lifecycle_by_host":{"cursor":"My cursor (test).","claude":"My claude (test)."},"review_findings_only_paragraph":"Review findings-only (test seed)."}"#,
    );
    write_text(
        &root.join("core/router-rs/Cargo.toml"),
        "[package]\nname = \"router-rs-marker\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    );
    fs::create_dir_all(root.join("skills")).unwrap_or_else(|err| {
        panic!("failed to create {}: {err}", root.join("skills").display());
    });
}

pub fn write_json(path: &Path, payload: &Value) {
    let content = format!("{}\n", serde_json::to_string_pretty(payload).unwrap());
    write_text(path, &content);
}

pub fn read_text(path: &Path) -> String {
    if !path.exists() {
        let path_str = path.to_string_lossy();
        if path_str.contains("/skills/") && !path_str.contains("/skills/.archive-cold/") {
            let alternative = PathBuf::from(path_str.replace("/skills/", "/skills/.archive-cold/"));
            if alternative.exists() {
                return fs::read_to_string(&alternative).unwrap_or_else(|err| {
                    panic!(
                        "failed to read (archive fallback) {}: {err}",
                        alternative.display()
                    );
                });
            }
        }
        if path_str.contains("/src/codex_hooks.rs") {
            let alternative = PathBuf::from(
                path_str.replace("/src/codex_hooks.rs", "/src/hosts/codex_hooks/mod.rs"),
            );
            if alternative.is_file() {
                return fs::read_to_string(&alternative).unwrap_or_else(|err| {
                    panic!(
                        "failed to read (codex_hooks fallback) {}: {err}",
                        alternative.display()
                    );
                });
            }
        }
        if path_str.contains("/core/router-rs/src/hook_common.rs") {
            let alternative = PathBuf::from(path_str.replace(
                "/core/router-rs/src/hook_common.rs",
                "/core/core-policy/src/hook_common.rs",
            ));
            if alternative.is_file() {
                return fs::read_to_string(&alternative).unwrap_or_else(|err| {
                    panic!(
                        "failed to read (hook_common fallback) {}: {err}",
                        alternative.display()
                    );
                });
            }
        }
    }
    fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    })
}

pub fn read_json(path: &Path) -> Value {
    serde_json::from_str(&read_text(path)).unwrap_or_else(|err| {
        panic!("failed to parse json {}: {err}", path.display());
    })
}

pub fn output_text(output: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

pub fn assert_success(output: &Output) {
    if !output.status.success() {
        let (stdout, stderr) = output_text(output);
        panic!(
            "command failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            stdout,
            stderr
        );
    }
}

pub fn json_from_output(output: &Output) -> Value {
    assert_success(output);
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        let (stdout, stderr) = output_text(output);
        panic!("failed to parse stdout as json: {err}\nstdout:\n{stdout}\nstderr:\n{stderr}");
    })
}

pub fn run(mut command: Command) -> Output {
    command
        .output()
        .unwrap_or_else(|err| panic!("failed to run command: {err}"))
}

pub fn run_ok(command: Command) -> Output {
    let output = run(command);
    assert_success(&output);
    output
}

pub fn router_rs_command<I, S>(args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let root = project_root();
    let router_bin = router_rs_binary().unwrap_or_else(|| {
        panic!(
            "router-rs binary not found; run `cargo build --release --manifest-path {}`",
            root.join("core/router-rs/Cargo.toml").display()
        )
    });
    let mut command = Command::new(router_bin);
    command.args(args).current_dir(root);
    if std::env::var_os("ROUTER_RS_COMPUTE_THREADS").is_none() {
        command.env("ROUTER_RS_COMPUTE_THREADS", "1");
    }
    command
}

pub fn router_rs_binary() -> Option<PathBuf> {
    static CACHE: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    CACHE.get_or_init(resolve_router_rs_binary).clone()
}

/// 与仓库根 `.cargo/config.toml` 的 `[build] target-dir` 对齐，避免误用陈旧的
/// `core/router-rs/target/**/router-rs`（未继承 workspace target-dir 时的产物）。
fn cargo_target_dir_from_config(root: &Path) -> Option<PathBuf> {
    let path = root.join(".cargo/config.toml");
    let content = fs::read_to_string(path).ok()?;
    for raw in content.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if let Some(rest) = line.strip_prefix("target-dir") {
            let mut rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '=');
            rest = rest.trim();
            let val = rest
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| rest.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(rest);
            let p = PathBuf::from(val);
            return Some(if p.is_absolute() { p } else { root.join(p) });
        }
    }
    None
}

fn is_redirect_shim(candidate: &Path) -> bool {
    let Ok(out) = std::process::Command::new(candidate)
        .arg("--help")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
    else {
        return false;
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    stderr.contains("binary moved to router-rs-cli")
}

fn resolve_router_rs_binary() -> Option<PathBuf> {
    let root = project_root();
    // Session-local `CARGO_TARGET_DIR`: pick under that tree first when the binary exists (matches
    // `cargo metadata` in that session).
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR")
        && let Some(p) = pick_router_rs_under_target_dir(&PathBuf::from(td))
    {
        return Some(p);
    }
    // Same resolution path as `cargo_router_rs_executable` inside router-rs (stable vs MCP stubs).
    if let Some(p) = router_rs_binary_via_cargo_metadata(&root) {
        return Some(p);
    }
    if let Some(base) = cargo_target_dir_from_config(&root)
        && let Some(p) = pick_router_rs_under_target_dir(&base)
    {
        return Some(p);
    }
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_router-rs-cli").map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_router-rs").map(PathBuf::from)
        && path.is_file()
        && !is_redirect_shim(&path)
    {
        return Some(path);
    }
    [
        root.join("target/debug/router-rs-cli"),
        root.join("target/release/router-rs-cli"),
        root.join("core/router-rs/target/debug/router-rs-cli"),
        root.join("core/router-rs/target/release/router-rs-cli"),
        root.join("target/debug/router-rs"),
        root.join("target/release/router-rs"),
        root.join("core/router-rs/target/debug/router-rs"),
        root.join("core/router-rs/target/release/router-rs"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file() && !is_redirect_shim(candidate))
}

pub fn router_rs_json(args: &[&str]) -> Value {
    json_from_output(&run(router_rs_command(args)))
}

pub fn host_integration_json(args: &[&str]) -> Value {
    let mut full_args = vec!["framework", "host-integration"];
    full_args.extend_from_slice(args);
    router_rs_json(&full_args)
}

pub fn cargo_manifest_command(manifest: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("cargo");
    command
        .args(["run", "--quiet", "--manifest-path"])
        .arg(manifest)
        .current_dir(project_root());
    if !args.is_empty() {
        command.arg("--").args(args);
    }
    command
}

pub fn shell_command(cwd: &Path, script: &str) -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", script]).current_dir(cwd);
    command
}

pub fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}
