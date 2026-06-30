#![deny(clippy::unwrap_used, clippy::expect_used)]
//! # framework-runtime
//!
//! Framework runtime: execution contracts, live execute, trace, env flags, and shared utilities
//! merged from the former `fr-utils`, `fr-contracts`, and `fr-exec` crates (2026-06-29).

// ── fr-utils modules ──
pub mod constants;
pub mod io_utils;
pub mod json_io;
pub mod json_value;
pub mod process_utils;
pub mod stdio_op_registry;
pub mod types;
pub mod util;

// ── fr-contracts modules ──
pub mod execution_contract;
pub mod pre_tool_use_guard;

// ── fr-exec modules ──
pub mod live_execute;
pub mod router_env_flags;
pub mod runtime_view;
pub mod trace_attach;
pub mod trace_stream_io;
pub mod trace_transport;

#[cfg(test)]
mod tests_all {
    #[test]
    fn smoke() {
        assert!(true);
    }
}
