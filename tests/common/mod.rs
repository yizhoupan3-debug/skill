#![allow(dead_code)]

use serde_json::{json, Value};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn pick_router_rs_under_target_dir(base: &Path) -> Option<PathBuf> {
    for candidate in [base.join("debug/router-rs"), base.join("release/router-rs")] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Align with `host_integration::cargo_router_rs_executable`: same `cargo metadata` target dir
/// as the running `router-rs` uses for MCP payload generation (avoids stale `target-dir` picks).
/// Same shape as `host_integration::cursor_mcp_server_payload` for pre-seeding `mcp.json` in
/// tests (matches `cargo_router_rs_executable` + `which::which(\"router-rs\")` fallback).
pub fn browser_mcp_server_payload_like_host(framework_root: &Path) -> Value {
    let manifest = framework_root.join("scripts/router-rs/Cargo.toml");
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
    let manifest = repo_root.join("scripts/router-rs/Cargo.toml");
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
        r#"{"schema_version":"framework-runtime-registry-v1","framework_core":{"authority":"rust","source":"framework-root-native","host_policy":"closed-set-explicit-projections"},"host_targets":{"policy":"shared-rust-core-explicit-host-projections","supported":["codex-cli","cursor"],"shared_system_source":"skills","metadata":{"codex-cli":{"install_tool":"codex","host_entrypoints":"AGENTS.md"},"cursor":{"install_tool":"cursor","host_entrypoints":["AGENTS.md",".cursor/rules/*.mdc"]}}},"host_projections":{"codex-cli":{"profile_id":"codex_profile"},"cursor":{"profile_id":"cursor_profile"}}}"#,
    );
    write_text(
        &root.join("scripts/router-rs/Cargo.toml"),
        "[package]\nname = \"router-rs-marker\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    );
    // `ensure_codex_skill_surface` may `read_dir` the skills source root when pinned/runtime
    // slugs yield an empty desired set — the directory must exist.
    fs::create_dir_all(root.join("skills")).unwrap_or_else(|err| {
        panic!("failed to create {}: {err}", root.join("skills").display());
    });
}

pub fn write_json(path: &Path, payload: &Value) {
    let content = format!("{}\n", serde_json::to_string_pretty(payload).unwrap());
    write_text(path, &content);
}

pub fn read_text(path: &Path) -> String {
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
            root.join("scripts/router-rs/Cargo.toml").display()
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
    // 不使用 OnceLock：测试进程内各用例顺序不定，若首次解析落到陈旧
    // `scripts/router-rs/target/**` 会污染后续用例。
    resolve_router_rs_binary()
}

/// 与仓库根 `.cargo/config.toml` 的 `[build] target-dir` 对齐，避免误用陈旧的
/// `scripts/router-rs/target/**/router-rs`（未继承 workspace target-dir 时的产物）。
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

fn resolve_router_rs_binary() -> Option<PathBuf> {
    let root = project_root();
    // Session-local `CARGO_TARGET_DIR`: pick under that tree first when the binary exists (matches
    // `cargo metadata` in that session).
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
        if let Some(p) = pick_router_rs_under_target_dir(&PathBuf::from(td)) {
            return Some(p);
        }
    }
    // Same resolution path as `cargo_router_rs_executable` inside router-rs (stable vs MCP stubs).
    if let Some(p) = router_rs_binary_via_cargo_metadata(&root) {
        return Some(p);
    }
    if let Some(base) = cargo_target_dir_from_config(&root) {
        if let Some(p) = pick_router_rs_under_target_dir(&base) {
            return Some(p);
        }
    }
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_router-rs").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }
    for candidate in [
        root.join("scripts/router-rs/target/debug/router-rs"),
        root.join("scripts/router-rs/target/release/router-rs"),
        root.join("target/debug/router-rs"),
        root.join("target/release/router-rs"),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn router_rs_json(args: &[&str]) -> Value {
    json_from_output(&run(router_rs_command(args)))
}

pub fn host_integration_json(args: &[&str]) -> Value {
    let mut full_args = vec!["codex", "host-integration"];
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
