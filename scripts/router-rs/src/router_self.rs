//! `router-rs self install|clean` — global binary install and build artifact cleanup.

use clap::{Args, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Subcommand, Debug, Clone)]
pub enum RouterSelfCommands {
    /// Copy this `router-rs` binary into a directory on your PATH (default: ~/.local/bin).
    Install(RouterSelfInstallArgs),
    /// Run `cargo clean` for this crate; optionally delete the shared Cargo target cache.
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
}

pub fn dispatch(command: RouterSelfCommands) -> Result<(), String> {
    match command {
        RouterSelfCommands::Install(args) => run_install(args.bin_dir),
        RouterSelfCommands::Clean(args) => run_clean(args.shared_target),
    }
}

fn run_install(bin_dir: Option<PathBuf>) -> Result<(), String> {
    #[cfg(not(unix))]
    {
        let _ = bin_dir;
        return Err("router-rs self install is only supported on unix hosts".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let dest_dir = bin_dir.unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
            PathBuf::from(home).join(".local/bin")
        });
        fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;
        let src = std::env::current_exe().map_err(|err| err.to_string())?;
        let dest = dest_dir.join("router-rs");
        fs::copy(&src, &dest).map_err(|err| err.to_string())?;
        let mut perms = fs::metadata(&dest)
            .map_err(|err| err.to_string())?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms).map_err(|err| err.to_string())?;
        eprintln!(
            "Installed router-rs -> {}\nAdd to PATH if needed: export PATH=\"{}:$PATH\"",
            dest.display(),
            dest_dir.display()
        );
        Ok(())
    }
}

fn run_clean(remove_shared_target: bool) -> Result<(), String> {
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
    if remove_shared_target {
        let shared = std::env::var("ROUTER_RS_SHARED_TARGET")
            .unwrap_or_else(|_| "/tmp/skill-cargo-target".to_string());
        let path = PathBuf::from(shared);
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|err| err.to_string())?;
            eprintln!("removed shared target dir {}", path.display());
        }
    }
    Ok(())
}
