//! RuntimeCore hooks: callback-based indirection for host-provider dependencies.
//!
//! The runtime-core crate must call `register()` before any session-supervisor operations

use std::sync::OnceLock;

static HOOKS: OnceLock<SessionSupervisorHooks> = OnceLock::new();

/// Access the registered hooks, if any.
pub fn hooks() -> Option<&'static SessionSupervisorHooks> {
    HOOKS.get()
}

/// Register hooks. Must be called before any session-supervisor operations that
/// need host-provider lookups (build_driver_command, driver_id_for_host).
/// Safe to call multiple times; only the first call takes effect.
pub fn register(h: SessionSupervisorHooks) {
    HOOKS.get_or_init(|| h);
}

use crate::types::DriverCommandSpec;

/// Hooks into runtime-core for host-provider dependencies.
pub struct SessionSupervisorHooks {
    /// Build a driver command for the given host via the provider registry.
    ///
    /// `cwd` is the **effective** working directory (worktree-resolved).
    /// Return `Some(Ok(spec))` for known hosts, `Some(Err(e))` for errors,
    /// or `None` to fall through to the built-in fallback logic (smoke-shell).
    #[allow(clippy::type_complexity)]
    pub build_driver_command: fn(
        host: &str,
        effective_cwd: &str,
        prompt: Option<String>,
        resume_target: Option<String>,
        resume_mode: &str,
        resume_only: bool,
        worktree_name: Option<String>,
        worktree_path: Option<String>,
    ) -> Option<Result<DriverCommandSpec, String>>,

    /// Look up the driver ID string for a host.
    pub driver_id_for_host: fn(host: &str) -> Option<&'static str>,
}
