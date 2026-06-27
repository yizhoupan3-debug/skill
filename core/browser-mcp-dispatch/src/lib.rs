//! Browser MCP dispatch: thin bridge between `tools/browser-mcp` and `core/host-projection`.
//!
//! Stores hook function pointers that the runtime layer (host-projection) provides,
//! so `tools/browser-mcp` can call runtime hooks without depending on host-projection at compile time.
//!
//! ## Wire-up (called in `router-rs` during init)
//! ```ignore
//! browser_mcp_dispatch::set_hooks(BrowserMcpHooks {
//!     evaluate_mcp_pre_guard: |tool, args, repo| {
//!         let v = host_projection::hooks::evaluate_mcp_pre_guard_safe(tool, args, repo);
//!         browser_mcp_dispatch::McpPreGuardVerdict { blocked: v.blocked, reason: v.reason }
//!     },
//!     attach_runtime_event_transport: host_projection::hooks::attach_runtime_event_transport,
//!     inspect_trace_stream: host_projection::hooks::inspect_trace_stream,
//! });
//! host_projection::hooks::set_browser_dispatch(browser_mcp::dispatch_browser_command);
//! ```
//!
//! ## Lint
//! This crate denies unwrap/expect in non-test code. The explicit `.expect()` panic on
//! `HOOKS` is a deliberate "must-be-initialized" assertion.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::sync::OnceLock;

/// Mirror of `host_projection::hooks::McpPreGuardVerdict`.
#[derive(Debug, Clone, Default)]
pub struct McpPreGuardVerdict {
    pub blocked: bool,
    pub reason: Option<String>,
}

/// Hook function pointers supplied by the runtime layer.
///
/// Every field is a function pointer (not a trait object) so the storage is
/// `OnceLock`-compatible with zero allocation after initialization.
pub struct BrowserMcpHooks {
    /// Evaluate MCP pre-guard for a tool call.
    pub evaluate_mcp_pre_guard: fn(
        tool_name: &str,
        arguments: &serde_json::Value,
        repo_root: &std::path::Path,
    ) -> McpPreGuardVerdict,
    /// Attach a runtime event transport.
    pub attach_runtime_event_transport: fn(serde_json::Value) -> Result<serde_json::Value, String>,
    /// Inspect a trace stream and return structured diagnostic data.
    pub inspect_trace_stream: fn(
        framework_kernel::stdio_payload_types::TraceStreamInspectRequestPayload,
    ) -> Result<
        framework_kernel::stdio_payload_types::TraceStreamInspectResponsePayload,
        String,
    >,
}

static HOOKS: OnceLock<BrowserMcpHooks> = OnceLock::new();

/// Set the hook function pointers. Must be called once at startup before any
/// `tools/browser-mcp` code runs. Panics on double-set (defense-in-depth).
pub fn set_hooks(h: BrowserMcpHooks) {
    if HOOKS.set(h).is_err() {
        tracing::warn!("BrowserMcpHooks already initialized — second call ignored");
    }
}

/// Get the hook function pointers. Panics if `set_hooks` was never called.
pub fn hooks() -> &'static BrowserMcpHooks {
    #[allow(clippy::expect_used)]
    HOOKS.get().expect(
        "BrowserMcpHooks not initialized — call browser_mcp_dispatch::set_hooks() before using browser-mcp",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_panics_when_uninitialized() {
        let _ = std::panic::catch_unwind(|| hooks());
    }

    #[test]
    fn set_hooks_accepts_valid_instance() {
        let h = BrowserMcpHooks {
            evaluate_mcp_pre_guard: |_, _, _| McpPreGuardVerdict { blocked: false, reason: None },
            attach_runtime_event_transport: |_| Ok(serde_json::json!({})),
            inspect_trace_stream: |_| {
                Ok(framework_kernel::stdio_payload_types::TraceStreamInspectResponsePayload {
                    schema_version: "1".into(),
                    authority: "test".into(),
                    path: "/test".into(),
                    source_kind: "browser".into(),
                    event_count: 0,
                    latest_event_id: None,
                    latest_event_kind: None,
                    latest_event_timestamp: None,
                    latest_cursor: None,
                    recovery: None,
                    reroute_count: 0,
                    retry_count: 0,
                })
            },
        };
        set_hooks(h);
    }
}
