//! Browser command dispatch hook (decouples runtime-core from browser-mcp crate).
//!
//! The actual `dispatch_browser_command` implementation lives in the `browser-mcp` crate.
//! At startup, the binary or router-rs registers the implementation via `set_browser_dispatch`.

use crate::cli::args::BrowserSubcommand;
use std::sync::OnceLock;

type BrowserDispatchFn = fn(BrowserSubcommand) -> Result<(), String>;

static BROWSER_DISPATCH: OnceLock<BrowserDispatchFn> = OnceLock::new();

/// Register the browser command dispatch function (call once at startup).
pub fn set_browser_dispatch(f: BrowserDispatchFn) {
    let _ = BROWSER_DISPATCH.set(f);
}

/// Dispatch a browser subcommand. Returns `Err` if no dispatch function was registered.
pub fn dispatch_browser_command(command: BrowserSubcommand) -> Result<(), String> {
    match BROWSER_DISPATCH.get() {
        Some(f) => f(command),
        None => Err(
            "browser-mcp dispatch not registered; call set_browser_dispatch() at startup"
                .to_string(),
        ),
    }
}
