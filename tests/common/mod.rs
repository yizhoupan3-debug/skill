#![allow(dead_code)]

use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

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

fn router_rs_binary() -> Option<PathBuf> {
    static ROUTER_BIN: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROUTER_BIN.get_or_init(resolve_router_rs_binary).clone()
}

fn resolve_router_rs_binary() -> Option<PathBuf> {
    let root = project_root();
    for candidate in [
        root.join("scripts/router-rs/target/release/router-rs"),
        root.join("scripts/router-rs/target/debug/router-rs"),
        root.join("target/release/router-rs"),
        root.join("target/debug/router-rs"),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_router-rs").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
        let base = PathBuf::from(td);
        for candidate in [base.join("release/router-rs"), base.join("debug/router-rs")] {
            if candidate.is_file() {
                return Some(candidate);
            }
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
