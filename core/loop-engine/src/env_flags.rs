//! Centralized environment variable access for loop-engine.
//!
//! All `ROUTER_RS_*` env var reads for this crate go through this module.
//! String/integer values are parsed here; boolean flags (e.g. `_ENABLED`)
//! delegate to [`core_policy::env_flags`] for consistent true/false semantics.

use std::sync::OnceLock;

/// Default max concurrent subagent processes (default: 4, min: 1).
const DEFAULT_MAX_CONCURRENT_PROCS: u32 = 4;

/// Default hard upper limit for `tokens_per_run` budget enforcement.
const DEFAULT_TOKENS_PER_RUN_HARD_LIMIT: u64 = 10_000_000;

/// Max concurrent subagent OS processes (`ROUTER_RS_SUBAGENT_MAX_CONCURRENT`).
/// Panics if set to 0 (minimum 1).
pub fn max_concurrent_procs() -> u32 {
    static VALUE: OnceLock<u32> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ROUTER_RS_SUBAGENT_MAX_CONCURRENT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_CONCURRENT_PROCS)
            .max(1)
    })
}

/// Resolve the subagent binary path (`ROUTER_RS_SUBAGENT_BIN`).
/// Returns `Err` if the env var is not set or is empty.
pub fn subagent_binary() -> Result<String, String> {
    std::env::var("ROUTER_RS_SUBAGENT_BIN")
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "subagent binary not found. Set ROUTER_RS_SUBAGENT_BIN.".to_string())
}

/// Resolve the autoresearch binary path (`ROUTER_RS_AUTORESEARCH_BIN`).
/// Returns empty string if the env var is not set (caller falls back to `cargo run`).
pub fn autoresearch_binary() -> String {
    std::env::var("ROUTER_RS_AUTORESEARCH_BIN")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_default()
}

/// Hard upper limit for `tokens_per_run` budget enforcement
/// (`ROUTER_RS_LOOP_MAX_TOKENS_PER_RUN`, default: 10M).
pub fn max_tokens_per_run_hard_limit() -> u64 {
    static VALUE: OnceLock<u64> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ROUTER_RS_LOOP_MAX_TOKENS_PER_RUN")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_TOKENS_PER_RUN_HARD_LIMIT)
    })
}
