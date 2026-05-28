//! Thin wrapper: forwards to `router-rs host antigravity-cli …` (or `router-rs antigravity-cli …`).

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

fn find_router_rs_in_tree(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        for rel in [
            "core/router-rs/target/release/router-rs",
            "core/router-rs/target/debug/router-rs",
        ] {
            let candidate = d.join(rel);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        dir = d.parent();
    }
    None
}

fn router_rs_bin_usable(path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() {
        return false;
    }
    let candidate = Path::new(path);
    if !candidate.is_file() {
        return false;
    }
    Command::new(candidate)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn resolve_router_rs_bin() -> String {
    if let Ok(bin) = env::var("ROUTER_RS_BIN") {
        if router_rs_bin_usable(&bin) {
            return bin.trim().to_string();
        }
    }
    if let Ok(cwd) = env::current_dir() {
        if let Some(bin) = find_router_rs_in_tree(&cwd) {
            return bin.to_string_lossy().into_owned();
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            if let Some(bin) = find_router_rs_in_tree(parent) {
                return bin.to_string_lossy().into_owned();
            }
        }
    }
    for candidate in ["router-rs"] {
        if Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return candidate.to_string();
        }
    }
    "router-rs".to_string()
}

fn main() -> ExitCode {
    let bin = resolve_router_rs_bin();
    let mut cmd = Command::new(&bin);
    cmd.arg("host").arg("antigravity-cli");
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        cmd.arg("--help");
    } else {
        cmd.args(&args);
    }
    let status = cmd.status().unwrap_or_else(|err| {
        eprintln!("antigravity-cli: failed to run {bin}: {err}");
        std::process::exit(1);
    });
    ExitCode::from(status.code().unwrap_or(1) as u8)
}
