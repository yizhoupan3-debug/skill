#![allow(dead_code)]

use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
    let router_root = root.join("scripts/router-rs");
    let mut command = Command::new(router_root.join("run_router_rs.sh"));
    command
        .arg(router_root.join("Cargo.toml"))
        .args(args)
        .current_dir(root);
    command
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
