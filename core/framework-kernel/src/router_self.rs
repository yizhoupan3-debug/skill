//! `router-rs-cli self install|clean` — global binary install and build artifact cleanup.

use clap::{Args, Subcommand};
use core_errors::FrameworkError;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing;

fn file_sha256(path: &Path) -> Result<[u8; 32], FrameworkError> {
    let bytes =
        fs::read(path).map_err(|err| FrameworkError::validation(format!("read {} for sha256: {err}", path.display())))?;
    Ok(Sha256::digest(&bytes).into())
}

#[derive(Subcommand, Debug, Clone)]
pub enum RouterSelfCommands {
    /// Copy this `router-rs-cli` binary into a directory on your PATH (default: ~/.local/bin).
    Install(RouterSelfInstallArgs),
    /// Run `cargo clean` for this crate; optionally delete shared/repo target caches.
    Clean(RouterSelfCleanArgs),
}

#[derive(Args, Debug, Clone)]
pub struct RouterSelfInstallArgs {
    /// Destination directory for the `router-rs-cli` binary (created if missing).
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

pub fn dispatch(command: RouterSelfCommands) -> Result<(), FrameworkError> {
    match command {
        RouterSelfCommands::Install(args) => {
            let dest = install_router_rs_to_bin_dir(args.bin_dir)?;
            tracing::info!(
                "Installed router-rs-cli -> {}\nAdd to PATH if needed: export PATH=\"{}:$PATH\"",
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
    default_router_rs_install_dir().join("router-rs-cli")
}

pub fn router_rs_desktop_mcp_dir_for_home(home: &Path) -> PathBuf {
    home.join(".local/share/skill-framework/bin")
}

pub fn router_rs_desktop_mcp_path_for_home(home: &Path) -> PathBuf {
    router_rs_desktop_mcp_dir_for_home(home).join("router-rs-cli")
}

pub fn install_router_rs_to_bin_dir(bin_dir: Option<PathBuf>) -> Result<PathBuf, FrameworkError> {
    #[cfg(not(unix))]
    {
        let _ = bin_dir;
        return Err(FrameworkError::unsupported("router-rs-cli self install is only supported on unix hosts"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let dest_dir = bin_dir.unwrap_or_else(default_router_rs_install_dir);
        fs::create_dir_all(&dest_dir).map_err(|err| FrameworkError::validation(err.to_string()))?;
        let src = std::env::current_exe().map_err(|err| FrameworkError::validation(err.to_string()))?;
        let dest = dest_dir.join("router-rs-cli");
        fs::copy(&src, &dest).map_err(|err| FrameworkError::validation(err.to_string()))?;
        let mut perms = fs::metadata(&dest)
            .map_err(|err| FrameworkError::validation(err.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms).map_err(|err| FrameworkError::validation(err.to_string()))?;
        Ok(dest)
    }
}

pub fn ensure_router_rs_installed_for_runtime() -> Result<PathBuf, FrameworkError> {
    if let Ok(path) = which::which("router-rs-cli")
        && path.is_file() && !is_ephemeral_router_rs_path(&path.to_string_lossy()) {
            return Ok(path);
        }
    let installed = default_router_rs_install_path();
    if installed.is_file() {
        return Ok(installed);
    }
    install_router_rs_to_bin_dir(None)
}

fn refresh_macos_binary_signature(path: &Path) -> Result<(), FrameworkError> {
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
            .map_err(|err| FrameworkError::validation(format!("xattr failed for {}: {err}", path.display())))?;
        let status = Command::new("codesign")
            .args(["-s", "-", "-f"])
            .arg(path)
            .status()
            .map_err(|err| FrameworkError::validation(format!("codesign failed for {}: {err}", path.display())))?;
        if !status.success() {
            return Err(FrameworkError::validation(format!(
                "codesign adhoc re-sign failed for {}: {status}",
                path.display()
            )));
        }
        Ok(())
    }
}

fn pick_router_rs_copy_source() -> Result<PathBuf, FrameworkError> {
    if let Ok(raw) = std::env::var("ROUTER_RS_BIN") {
        let path = PathBuf::from(raw.trim());
        if path.is_file() && !is_ephemeral_router_rs_path(&path.to_string_lossy()) {
            return Ok(path);
        }
    }
    if let Ok(current) = std::env::current_exe()
        && current.is_file() && !is_ephemeral_router_rs_path(&current.to_string_lossy()) {
            return Ok(current);
        }
    if let Ok(path) = which::which("router-rs-cli") {
        let text = path.to_string_lossy();
        if path.is_file() && !is_ephemeral_router_rs_path(&text) {
            return Ok(path);
        }
    }
    let local = default_router_rs_install_path();
    if local.is_file() {
        return Ok(local);
    }
    Err(FrameworkError::not_found(
        "no stable router-rs-cli binary found; build with `cargo build --release --manifest-path core/router-rs/Cargo.toml` or set ROUTER_RS_BIN"
    ))
}

/// Install Desktop MCP binary under `{home}/.local/share/skill-framework/bin/router-rs-cli`.
pub fn install_router_rs_for_desktop_mcp_at(home_account: &Path) -> Result<PathBuf, FrameworkError> {
    #[cfg(not(unix))]
    {
        let _ = home_account;
        return Err(FrameworkError::unsupported("router-rs-cli desktop MCP install is only supported on unix hosts"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let source = pick_router_rs_copy_source()?;
        let dest_dir = router_rs_desktop_mcp_dir_for_home(home_account);
        fs::create_dir_all(&dest_dir).map_err(|err| FrameworkError::validation(err.to_string()))?;
        let dest = router_rs_desktop_mcp_path_for_home(home_account);
        let needs_copy = match (fs::metadata(&source), fs::metadata(&dest)) {
            (Ok(src_meta), Ok(dest_meta)) => {
                if src_meta.len() != dest_meta.len()
                    || src_meta
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
            _ => true,
        };
        if needs_copy {
            fs::copy(&source, &dest).map_err(|err| {
                FrameworkError::validation(format!(
                    "copy router-rs for desktop MCP {} -> {}: {err}",
                    source.display(),
                    dest.display()
                ))
            })?;
        }
        let mut perms = fs::metadata(&dest)
            .map_err(|err| FrameworkError::validation(err.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms).map_err(|err| FrameworkError::validation(err.to_string()))?;
        refresh_macos_binary_signature(&dest)?;
        Ok(dest)
    }
}

pub fn validate_router_rs_binary_runnable(path: &Path) -> Result<(), FrameworkError> {
    if !path.is_file() {
        return Err(FrameworkError::validation(format!("router-rs-cli binary missing at {}", path.display())));
    }
    let status = Command::new(path)
        .arg("framework")
        .arg("--help")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|err| FrameworkError::validation(format!("failed to exec {}: {err}", path.display())))?;
    if status.success() {
        return Ok(());
    }
    Err(FrameworkError::validation(format!(
        "router-rs-cli binary at {} failed smoke test (exit {status})",
        path.display()
    )))
}

/// Resolve the `router-rs-cli` binary for subprocess e2e tests (never falls back to the test harness exe).
pub fn resolve_router_rs_test_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_router-rs-cli") {
        return PathBuf::from(path);
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_router-rs-cli") {
        return PathBuf::from(path);
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for candidate in [
        manifest.join("../../target/debug/router-rs-cli"),
        manifest.join("../target/debug/router-rs-cli"),
        PathBuf::from("/tmp/skill-cargo-target/debug/router-rs-cli"),
        manifest.join("../../target/release/router-rs-cli"),
        manifest.join("../target/release/router-rs-cli"),
        PathBuf::from("/tmp/skill-cargo-target/release/router-rs-cli"),
    ] {
        if candidate.is_file() {
            return candidate;
        }
    }
    if let Ok(ref target) = std::env::var("CARGO_TARGET_DIR") {
        let candidate = PathBuf::from(target).join("debug/router-rs-cli");
        if candidate.is_file() {
            return candidate;
        }
        let candidate = PathBuf::from(target).join("release/router-rs-cli");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!(
        "router-rs-cli test binary not found; run `cargo test -p router-rs` (which auto-builds), \
         or manually `cargo build -p router-rs-cli`"
    );
}

pub fn is_ephemeral_router_rs_path(path: &str) -> bool {
    crate::runtime_registry::EPHEMERAL_PATH_PATTERNS.iter().any(|p| path.contains(p))
        || path.starts_with("/tmp/")
}

pub fn is_repo_build_router_rs_path(path: &str, framework_root: &Path) -> bool {
    if !(path.contains("/target/release/router-rs-cli") || path.contains("/target/debug/router-rs-cli")) {
        return false;
    }
    if let Ok(root) = framework_root.canonicalize()
        && let Ok(path_buf) = Path::new(path).canonicalize() {
            for suffix in ["core/router-rs/target", "target"] {
                if path_buf.starts_with(root.join(suffix)) {
                    return true;
                }
            }
        }
    let root_text = framework_root.to_string_lossy();
    path.starts_with(root_text.as_ref())
        && (path.contains("/target/release/router-rs-cli") || path.contains("/target/debug/router-rs-cli"))
}

fn run_clean(args: RouterSelfCleanArgs) -> Result<(), FrameworkError> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let status = Command::new("cargo")
        .args(["clean", "--manifest-path"])
        .arg(&manifest)
        .status()
        .map_err(|err| FrameworkError::validation(format!("cargo clean failed to spawn: {err}")))?;
    if !status.success() {
        return Err(FrameworkError::validation(format!("cargo clean failed: {status}")));
    }
    tracing::info!("cargo clean ok for {}", manifest.display());

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
            .ok_or_else(|| {
                FrameworkError::validation("could not resolve framework root for repo target cleanup".to_string())
            })?;
        let crate_target = manifest.parent().ok_or_else(|| {
            FrameworkError::validation(format!(
                "cannot resolve crate target dir from manifest path {}",
                manifest.display()
            ))
        })?;
        remove_dir_if_exists(&crate_target.join("target"))?;
        remove_dir_if_exists(&framework_root.join("target"))?;
    }

    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> Result<(), FrameworkError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|err| FrameworkError::validation(err.to_string()))?;
        tracing::info!("removed {}", path.display());
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
        assert_eq!(default_router_rs_install_path(), dir.join("router-rs-cli"));
    }

    #[test]
    fn ephemeral_paths_detect_sandbox_and_tmp_targets() {
        assert!(is_ephemeral_router_rs_path(
            "/tmp/skill-cargo-target/debug/router-rs-cli"
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
        let debug = framework_root.join("target/debug/router-rs-cli");
        assert!(is_repo_build_router_rs_path(
            &debug.to_string_lossy(),
            framework_root
        ));
        assert!(!is_repo_build_router_rs_path(
            "/other/repo/target/debug/router-rs-cli",
            framework_root
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
        fs::write(&path, b"router-rs-cli-self").unwrap();
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
            dir.join("router-rs-cli")
        );
    }

    #[test]
    fn repo_build_paths_include_release_target() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let framework_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("framework root");
        let release = framework_root.join("target/release/router-rs-cli");
        assert!(is_repo_build_router_rs_path(
            &release.to_string_lossy(),
            framework_root
        ));
    }

    fn try_resolve_router_rs_test_bin() -> Option<PathBuf> {
        if let Some(path) = option_env!("CARGO_BIN_EXE_router-rs-cli") {
            return Some(PathBuf::from(path));
        }
        if let Ok(path) = std::env::var("CARGO_BIN_EXE_router-rs-cli") {
            return Some(PathBuf::from(path));
        }
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for candidate in [
            manifest.join("../../target/debug/router-rs-cli"),
            manifest.join("../target/debug/router-rs-cli"),
            PathBuf::from("/tmp/skill-cargo-target/debug/router-rs-cli"),
            manifest.join("../../target/release/router-rs-cli"),
            manifest.join("../target/release/router-rs-cli"),
            PathBuf::from("/tmp/skill-cargo-target/release/router-rs-cli"),
        ] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if let Ok(target) = std::env::var("CARGO_TARGET_DIR") {
            let candidate = PathBuf::from(target).join("debug/router-rs-cli");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    #[test]
    fn validate_router_rs_binary_runnable_smoke() {
        let Some(bin) = try_resolve_router_rs_test_bin() else {
            tracing::warn!("skip: router-rs-cli binary not built (per-crate test run)");
            return;
        };
        validate_router_rs_binary_runnable(&bin).expect("router-rs --help smoke");
    }

    #[test]
    fn validate_router_rs_binary_runnable_rejects_missing_file() {
        let missing = std::env::temp_dir().join("router-rs-cli-missing-binary-smoke");
        let err = validate_router_rs_binary_runnable(&missing).unwrap_err();
        assert!(
            err.to_string().contains("missing"),
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
        assert_eq!(dest, dir.join("router-rs-cli"));
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
        let crate_target = manifest_dir.join("target/debug/router-rs-cli");
        assert!(is_repo_build_router_rs_path(
            &crate_target.to_string_lossy(),
            framework_root
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
        assert_eq!(dest, router_rs_desktop_mcp_path_for_home(&home));
        assert!(dest.is_file());
        assert!(
            dest.to_string_lossy()
                .contains(".local/share/skill-framework/bin/router-rs")
        );
        let _ = fs::remove_dir_all(&home);
    }
}
