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

pub fn router_rs_desktop_mcp_dir_for_home(home: &Path) -> PathBuf {
    home.join(".local/share/skill-framework/bin")
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

/// Detect the redirect shim stub left after v5 migration (prints "moved" and exits 1).
fn is_redirect_shim(candidate: &std::path::Path) -> bool {
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

/// Resolve the `router-rs-cli` binary for subprocess e2e tests (never falls back to the test harness exe).
/// Falls back to `router-rs` for backward compatibility. Skips redirect shims.
pub fn resolve_router_rs_test_bin() -> PathBuf {
    // router-rs binary has been moved to router-rs-cli
    if let Some(path) = option_env!("CARGO_BIN_EXE_router-rs-cli") {
        return PathBuf::from(path);
    }
    if let Some(path) = option_env!("CARGO_BIN_EXE_router-rs") {
        return PathBuf::from(path);
    }
    for bin_name in &["router-rs-cli", "router-rs"] {
        if let Ok(path) = std::env::var(format!("CARGO_BIN_EXE_{bin_name}")) {
            return PathBuf::from(path);
        }
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for candidate in [
            manifest.join(format!("../../target/debug/{bin_name}")),
            manifest.join(format!("../target/debug/{bin_name}")),
            PathBuf::from(format!("/tmp/skill-cargo-target/debug/{bin_name}")),
            manifest.join(format!("../../target/release/{bin_name}")),
            manifest.join(format!("../target/release/{bin_name}")),
            PathBuf::from(format!("/tmp/skill-cargo-target/release/{bin_name}")),
        ] {
            if candidate.is_file() && !is_redirect_shim(&candidate) {
                return candidate;
            }
        }
        if let Ok(ref target) = std::env::var("CARGO_TARGET_DIR") {
            let candidate = PathBuf::from(target).join(format!("debug/{bin_name}"));
            if candidate.is_file() && !is_redirect_shim(&candidate) {
                return candidate;
            }
            let candidate = PathBuf::from(target).join(format!("release/{bin_name}"));
            if candidate.is_file() && !is_redirect_shim(&candidate) {
                return candidate;
            }
        }
    }
    panic!(
        "router-rs-cli test binary not found; run `cargo test -p router-rs` (which auto-builds), \
         or manually `cargo build -p router-rs-cli`"
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_install_paths_use_home_local_bin() {
        let dir = default_router_rs_install_dir();
        assert!(dir.ends_with(".local/bin"));
        assert_eq!(
            default_router_rs_install_path(),
            dir.join("router-rs")
        );
    }

    #[test]
    fn ephemeral_paths_detect_sandbox_and_tmp_targets() {
        assert!(is_ephemeral_router_rs_path(
            "/tmp/skill-cargo-target/debug/router-rs"
        ));
        assert!(is_ephemeral_router_rs_path(
            "/var/folders/xx/cursor-sandbox-cache/yy/router-rs"
        ));
        assert!(!is_ephemeral_router_rs_path(
            "/Users/joe/.local/bin/router-rs"
        ));
    }

    #[test]
    fn repo_build_paths_match_framework_target_layout() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let framework_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("framework root");
        let debug = framework_root.join("target/debug/router-rs");
        assert!(is_repo_build_router_rs_path(
            &debug.to_string_lossy(),
            &framework_root
        ));
        assert!(!is_repo_build_router_rs_path(
            "/other/repo/target/debug/router-rs",
            &framework_root
        ));
    }

    #[test]
    fn file_sha256_is_stable_for_known_content() {
        let dir = std::env::temp_dir().join(format!(
            "router-self-sha-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("probe.bin");
        fs::write(&path, b"router-rs-self").unwrap();
        let digest = file_sha256(&path).expect("sha256");
        assert_eq!(digest.len(), 32);
        assert_eq!(digest, file_sha256(&path).expect("repeat"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn desktop_mcp_paths_under_skill_framework_share() {
        let home = Path::new("/tmp/test-home");
        let dir = router_rs_desktop_mcp_dir_for_home(home);
        assert!(dir.ends_with(".local/share/skill-framework/bin"));
        assert_eq!(
            router_rs_desktop_mcp_path_for_home(home),
            dir.join("router-rs")
        );
    }

    #[test]
    fn repo_build_paths_include_release_target() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let framework_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("framework root");
        let release = framework_root.join("target/release/router-rs");
        assert!(is_repo_build_router_rs_path(
            &release.to_string_lossy(),
            &framework_root
        ));
    }

    fn is_redirect_shim(candidate: &std::path::Path) -> bool {
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

    fn try_resolve_router_rs_test_bin() -> Option<PathBuf> {
        // router-rs binary has been moved to router-rs-cli
        if let Some(path) = option_env!("CARGO_BIN_EXE_router-rs-cli") {
            return Some(PathBuf::from(path));
        }
        if let Some(path) = option_env!("CARGO_BIN_EXE_router-rs") {
            return Some(PathBuf::from(path));
        }
        for bin_name in &["router-rs-cli", "router-rs"] {
            if let Ok(path) = std::env::var(format!("CARGO_BIN_EXE_{bin_name}")) {
                return Some(PathBuf::from(path));
            }
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            for candidate in [
                manifest.join(format!("../../target/debug/{bin_name}")),
                manifest.join(format!("../target/debug/{bin_name}")),
                PathBuf::from(format!("/tmp/skill-cargo-target/debug/{bin_name}")),
                manifest.join(format!("../../target/release/{bin_name}")),
                manifest.join(format!("../target/release/{bin_name}")),
                PathBuf::from(format!("/tmp/skill-cargo-target/release/{bin_name}")),
            ] {
                if candidate.is_file() && !is_redirect_shim(&candidate) {
                    return Some(candidate);
                }
            }
            if let Ok(target) = std::env::var("CARGO_TARGET_DIR") {
                let candidate = PathBuf::from(target).join(format!("debug/{bin_name}"));
                if candidate.is_file() && !is_redirect_shim(&candidate) {
                    return Some(candidate);
                }
            }
        }
        None
    }

    #[test]
    fn validate_router_rs_binary_runnable_smoke() {
        let Some(bin) = try_resolve_router_rs_test_bin() else {
            eprintln!("skip: router-rs binary not built (per-crate test run)");
            return;
        };
        // Detect redirect shim (post-migration stub that prints "moved" and exits 1).
        let probe = Command::new(&bin)
            .arg("--help")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();
        if let Ok(out) = probe {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            if combined.contains("moved") || combined.contains("router-rs-cli") {
                eprintln!("skip: router-rs binary is a redirect shim; smoke test requires the real binary");
                return;
            }
        }
        validate_router_rs_binary_runnable(&bin).expect("router-rs --help smoke");
    }

    #[test]
    fn validate_router_rs_binary_runnable_rejects_missing_file() {
        let missing = std::env::temp_dir().join("router-rs-missing-binary-smoke");
        let err = validate_router_rs_binary_runnable(&missing).unwrap_err();
        assert!(
            err.contains("missing"),
            "expected missing-file error, got: {err}"
        );
    }

    #[test]
    fn is_ephemeral_bare_tmp_prefix_paths() {
        assert!(is_ephemeral_router_rs_path("/tmp/router-rs"));
        assert!(is_ephemeral_router_rs_path("/tmp/foo/bar/router-rs"));
    }

    #[cfg(unix)]
    #[test]
    fn install_router_rs_to_bin_dir_copies_current_exe() {
        let dir = std::env::temp_dir().join(format!(
            "router-self-install-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let dest = install_router_rs_to_bin_dir(Some(dir.clone())).expect("install");
        assert!(dest.is_file());
        assert_eq!(dest, dir.join("router-rs"));
        let src = std::env::current_exe().expect("current_exe");
        assert_eq!(
            fs::metadata(&dest).expect("dest meta").len(),
            fs::metadata(&src).expect("src meta").len()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repo_build_paths_include_crate_local_target() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let framework_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("framework root");
        let crate_target = manifest_dir.join("target/debug/router-rs");
        assert!(is_repo_build_router_rs_path(
            &crate_target.to_string_lossy(),
            &framework_root
        ));
    }

    #[cfg(unix)]
    #[test]
    fn install_router_rs_for_desktop_mcp_at_copies_into_skill_framework_bin() {
        let home = std::env::temp_dir().join(format!(
            "router-self-desktop-mcp-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&home).unwrap();
        let dest = install_router_rs_for_desktop_mcp_at(&home).expect("desktop mcp install");
        assert_eq!(
            dest,
            router_rs_desktop_mcp_path_for_home(&home)
        );
        assert!(dest.is_file());
        assert!(
            dest.to_string_lossy()
                .contains(".local/share/skill-framework/bin/router-rs")
        );
        let _ = fs::remove_dir_all(&home);
    }
}
