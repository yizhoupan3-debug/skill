//! Browser command dispatch hook — backward-compat delegate.
//!
//! The OnceLock + setter/getter moved to `host-projection/src/hooks.rs` (L0)
//! to break the L3→L4 DAG violation (browser-mcp→runtime-core).
//! This file is kept as a backward-compat shim; new code should use
//! `host_projection::hooks::set_browser_dispatch` / `dispatch_browser_command` directly.

use framework_kernel::cli_args::BrowserSubcommand;

/// Register the browser command dispatch function (call once at startup).
/// Delegates to `host_projection::hooks::set_browser_dispatch`.
pub fn set_browser_dispatch(f: fn(BrowserSubcommand) -> Result<(), String>) {
    host_projection::hooks::set_browser_dispatch(f);
}

/// Dispatch a browser subcommand.
/// Delegates to `host_projection::hooks::dispatch_browser_command`.
pub fn dispatch_browser_command(command: BrowserSubcommand) -> Result<(), String> {
    host_projection::hooks::dispatch_browser_command(command)
}
