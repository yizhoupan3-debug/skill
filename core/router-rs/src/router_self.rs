//! `router-rs self install|clean` — global binary install and build artifact cleanup.

use clap::{Args, Subcommand};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn file_sha256(path: &Path) -> Result<[u8; 32], String> {
    let bytes = fs::read(path).map_err(|err| format!("read {} for sha256: {err}", path.display()))?;
    Ok(Sha256::digest(&bytes).into())
}

#[derive(Subcommand, Debug, Clone)]
pub enum RouterSelfCommands {
    /// Copy this `router-rs` binary into a directory on your PATH (default: ~/.local/bin).
    Install(RouterSelfInstallArgs),
    /// Run `cargo clean` for this crate; optionally delete shared/repo target caches.
    Clean(RouterSelfCleanArgs),
}

#[derive(Args, Debug, Clone)]
pub struct RouterSelfInstallArgs {
    /// Destination directory for the `router-rs` binary (created if missing).
    #[arg(long)]
    pub bin_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct RouterSelfCleanArgs {
    /// Remove `ROUTER_RS_SHARED_TARGET` if set, otherwise `/tmp/skill-cargo-target`.
    #[arg(long, default_value_t = false)]
    pub shared_target: bool,
    /// Remove repo-local `core/router-rs/target` and framework-root `target/` (after `cargo clean`).
    #[arg(long, default_value_t = false)]
    pub repo_targets: bool,
}

pub fn dispatch(command: RouterSelfCommands) -> Result<(), String> {
    match command {
        RouterSelfCommands::Install(args) => {
            let dest = install_router_rs_to_bin_dir(args.bin_dir)?;
            eprintln!(
                "Installed router-rs -> {}\nAdd to PATH if needed: export PATH=\"{}:$PATH\"",
                dest.display(),
                dest.parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "~/.local/bin".to_string())
            );
            Ok(())
        }
        RouterSelfCommands::Clean(args) => run_clean(args),
    }
}

pub fn default_router_rs_install_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    PathBuf::from(home).join(".local/bin")
}

pub fn default_router_rs_install_path() -> PathBuf {
    default_router_rs_install_dir().join("router-rs")
}

/// Stable MCP binary for Claude Desktop (survives `/tmp` cargo target cleanup; re-signed on macOS).
pub fn default_router_rs_desktop_mcp_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    router_rs_desktop_mcp_dir_for_home(Path::new(&home))
}

pub fn router_rs_desktop_mcp_dir_for_home(home: &Path) -> PathBuf {
    home.join(".local/share/skill-framework/bin")
}

pub fn default_router_rs_desktop_mcp_path() -> PathBuf {
    default_router_rs_desktop_mcp_dir().join("router-rs")
}

pub fn router_rs_desktop_mcp_path_for_home(home: &Path) -> PathBuf {
    router_rs_desktop_mcp_dir_for_home(home).join("router-rs")
}

pub fn install_router_rs_to_bin_dir(bin_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    #[cfg(not(unix))]
    {
        let _ = bin_dir;
        return Err("router-rs self install is only supported on unix hosts".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let dest_dir = bin_dir.unwrap_or_else(default_router_rs_install_dir);
        fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;
        let src = std::env::current_exe().map_err(|err| err.to_string())?;
        let dest = dest_dir.join("router-rs");
        fs::copy(&src, &dest).map_err(|err| err.to_string())?;
        let mut perms = fs::metadata(&dest)
            .map_err(|err| err.to_string())?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms).map_err(|err| err.to_string())?;
        Ok(dest)
    }
}

pub fn ensure_router_rs_installed_for_runtime() -> Result<PathBuf, String> {
    if let Ok(path) = which::which("router-rs") {
        if path.is_file() && !is_ephemeral_router_rs_path(&path.to_string_lossy()) {
            return Ok(path);
        }
    }
    let installed = default_router_rs_install_path();
    if installed.is_file() {
        return Ok(installed);
    }
    install_router_rs_to_bin_dir(None)
}

fn refresh_macos_binary_signature(path: &Path) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("xattr")
            .args(["-cr"])
            .arg(path)
            .status()
            .map_err(|err| format!("xattr failed for {}: {err}", path.display()))?;
        let status = Command::new("codesign")
            .args(["-s", "-", "-f"])
            .arg(path)
            .status()
            .map_err(|err| format!("codesign failed for {}: {err}", path.display()))?;
        if !status.success() {
            return Err(format!(
                "codesign adhoc re-sign failed for {}: {status}",
                path.display()
            ));
        }
        Ok(())
    }
}

fn pick_router_rs_copy_source() -> Result<PathBuf, String> {
    if let Ok(raw) = std::env::var("ROUTER_RS_BIN") {
        let path = PathBuf::from(raw.trim());
        if path.is_file() && !is_ephemeral_router_rs_path(&path.to_string_lossy()) {
            return Ok(path);
        }
    }
    if let Ok(current) = std::env::current_exe() {
        if current.is_file() && !is_ephemeral_router_rs_path(&current.to_string_lossy()) {
            return Ok(current);
        }
    }
    if let Ok(path) = which::which("router-rs") {
        let text = path.to_string_lossy();
        if path.is_file() && !is_ephemeral_router_rs_path(&text) {
            return Ok(path);
        }
    }
    let local = default_router_rs_install_path();
    if local.is_file() {
        return Ok(local);
    }
    Err(
        "no stable router-rs binary found; build with `cargo build --release --manifest-path core/router-rs/Cargo.toml` or set ROUTER_RS_BIN"
            .to_string(),
    )
}

/// Install a codesign-safe router-rs for Claude Desktop MCP (not ephemeral `/tmp` paths).
pub fn install_router_rs_for_desktop_mcp() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    install_router_rs_for_desktop_mcp_at(Path::new(&home))
}

/// Install Desktop MCP binary under `{home}/.local/share/skill-framework/bin/router-rs`.
pub fn install_router_rs_for_desktop_mcp_at(home_account: &Path) -> Result<PathBuf, String> {
    #[cfg(not(unix))]
    {
        let _ = home_account;
        return Err("router-rs desktop MCP install is only supported on unix hosts".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let source = pick_router_rs_copy_source()?;
        let dest_dir = router_rs_desktop_mcp_dir_for_home(home_account);
        fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;
        let dest = router_rs_desktop_mcp_path_for_home(home_account);
        let needs_copy = match (fs::metadata(&source), fs::metadata(&dest)) {
            (Ok(src_meta), Ok(dest_meta)) => {
                if src_meta.len() != dest_meta.len() {
                    true
                } else if src_meta
                    .modified()
                    .ok()
                    .zip(dest_meta.modified().ok())
                    .is_some_and(|(src, dest)| src > dest)
                {
                    true
                } else {
                    file_sha256(&source).ok() != file_sha256(&dest).ok()
                }
            }
            (Ok(_), Err(_)) => true,
            _ => true,
        };
        if needs_copy {
            fs::copy(&source, &dest).map_err(|err| {
                format!(
                    "copy router-rs for desktop MCP {} -> {}: {err}",
                    source.display(),
                    dest.display()
                )
            })?;
        }
        let mut perms = fs::metadata(&dest)
            .map_err(|err| err.to_string())?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms).map_err(|err| err.to_string())?;
        refresh_macos_binary_signature(&dest)?;
        Ok(dest)
    }
}

pub fn validate_router_rs_binary_runnable(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!(
            "router-rs binary missing at {}",
            path.display()
        ));
    }
    let status = Command::new(path)
        .arg("framework")
        .arg("--help")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|err| format!("failed to exec {}: {err}", path.display()))?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "router-rs binary at {} failed smoke test (exit {status})",
        path.display()
    ))
}

pub fn is_ephemeral_router_rs_path(path: &str) -> bool {
    path.contains("cursor-sandbox-cache")
        || path.contains("/tmp/skill-cargo-target")
        || path.starts_with("/tmp/")
}

pub fn is_repo_build_router_rs_path(path: &str, framework_root: &Path) -> bool {
    if !(path.contains("/target/release/router-rs") || path.contains("/target/debug/router-rs")) {
        return false;
    }
    if let Ok(root) = framework_root.canonicalize() {
        if let Ok(path_buf) = Path::new(path).canonicalize() {
            for suffix in ["core/router-rs/target", "target"] {
                if path_buf.starts_with(root.join(suffix)) {
                    return true;
                }
            }
        }
    }
    let root_text = framework_root.to_string_lossy();
    path.starts_with(root_text.as_ref())
        && (path.contains("/target/release/router-rs") || path.contains("/target/debug/router-rs"))
}

fn run_clean(args: RouterSelfCleanArgs) -> Result<(), String> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let status = Command::new("cargo")
        .args(["clean", "--manifest-path"])
        .arg(&manifest)
        .status()
        .map_err(|err| format!("cargo clean failed to spawn: {err}"))?;
    if !status.success() {
        return Err(format!("cargo clean failed: {status}"));
    }
    eprintln!("cargo clean ok for {}", manifest.display());

    if args.shared_target {
        let shared = std::env::var("ROUTER_RS_SHARED_TARGET")
            .unwrap_or_else(|_| "/tmp/skill-cargo-target".to_string());
        remove_dir_if_exists(&PathBuf::from(shared))?;
    }

    if args.repo_targets {
        let framework_root = manifest
            .parent()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
            .ok_or_else(|| "could not resolve framework root for repo target cleanup".to_string())?;
        remove_dir_if_exists(&manifest.parent().unwrap().join("target"))?;
        remove_dir_if_exists(&framework_root.join("target"))?;
    }

    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|err| err.to_string())?;
        eprintln!("removed {}", path.display());
    }
    Ok(())
}
