//! JSON / text I/O — delegates to core-state (B0) canonical implementation.
//!
//! ADR-010 §9: `core_state::utils::json_io` is the single source of truth.
//! This module re-exports for backward compatibility and adds CLI-specific helpers.

use serde::Serialize;


// ── Canonical re-exports from B0 ──
pub use core_state::utils::json_io::{
    read_json_if_exists, read_json_strict, read_text_if_exists, write_json_if_changed,
    write_text_if_changed,
};

// ── CLI-specific helpers (not B0 — depend on stdout) ──

pub fn print_json_value<T: Serialize>(payload: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(payload).map_err(|err| format!("serialize output failed: {err}"))?
    );
    Ok(())
}

pub fn parse_json_input<T>(raw: &str, context: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(raw).map_err(|err| format!("parse {context} input failed: {err}"))
}
