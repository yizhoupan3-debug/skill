//! Thin wrapper: forwards to `router-rs host antigravity-cli …` (or `router-rs antigravity-cli …`).

use std::env;
use std::process::{Command, ExitCode, Stdio};

fn resolve_router_rs_bin() -> String {
    if let Ok(bin) = env::var("ROUTER_RS_BIN") {
        if !bin.trim().is_empty() {
            return bin;
        }
    }
    for candidate in [
        "router-rs",
        "core/router-rs/target/release/router-rs",
        "core/router-rs/target/debug/router-rs",
    ] {
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
