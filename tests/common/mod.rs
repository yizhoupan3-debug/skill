#![allow(dead_code)]

use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

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

fn newest_mtime(path: &Path) -> Option<SystemTime> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.is_file() {
        return metadata.modified().ok();
    }
    if !metadata.is_dir() {
        return None;
    }
    let mut newest = metadata.modified().ok();
    for entry in fs::read_dir(path).ok()? {
        let child = entry.ok()?.path();
        if let Some(mtime) = newest_mtime(&child) {
            if newest.map_or(true, |current| mtime > current) {
                newest = Some(mtime);
            }
        }
    }
    newest
}

fn freshest_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths
        .iter()
        .filter(|path| path.is_file())
        .max_by_key(|path| {
            (
                path.metadata()
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH),
                path.to_string_lossy().to_string(),
            )
        })
        .cloned()
}

pub fn ensure_router_rs_binary_fresh() {
    let root = project_root();
    let router_root = root.join("scripts/router-rs");
    if !router_root.exists() {
        return;
    }
    let latest_source = [router_root.join("Cargo.toml"), router_root.join("src")]
        .iter()
        .filter_map(|path| newest_mtime(path))
        .max();
    let latest_binary = freshest_existing(&[
        router_root.join("target/release/router-rs"),
        router_root.join("target/debug/router-rs"),
    ])
    .and_then(|path| {
        path.metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    if latest_binary.is_some() && latest_source <= latest_binary {
        return;
    }
    let output = Command::new("cargo")
        .args(["build", "--manifest-path"])
        .arg(router_root.join("Cargo.toml"))
        .current_dir(root)
        .output()
        .expect("failed to build router-rs");
    assert_success(&output);
}

pub fn router_rs_command<I, S>(args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    ensure_router_rs_binary_fresh();
    let root = project_root();
    let router_root = root.join("scripts/router-rs");
    if let Some(binary) = freshest_existing(&[
        router_root.join("target/release/router-rs"),
        router_root.join("target/debug/router-rs"),
    ]) {
        let mut command = Command::new(binary);
        command.args(args);
        command.current_dir(root);
        return command;
    }

    let mut command = Command::new("cargo");
    command
        .args(["run", "--quiet", "--manifest-path"])
        .arg(router_root.join("Cargo.toml"))
        .args(["--release", "--"])
        .args(args)
        .current_dir(root);
    command
}

pub fn router_rs_json(args: &[&str]) -> Value {
    json_from_output(&run(router_rs_command(args)))
}

pub fn host_integration_json(args: &[&str]) -> Value {
    let mut full_args = vec!["--host-integration"];
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
