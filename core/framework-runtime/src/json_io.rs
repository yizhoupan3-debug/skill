//! CLI-specific JSON / text I/O helpers — stdout-dependent, not in core_state_utils.
//!
//! For canonical I/O primitives (`read_json_strict`, `write_json_if_changed`, etc.)
//! use `core_state_utils::json_io::*` directly.

use core_errors::FrameworkError;
use serde::Serialize;
use std::io::Write;

pub fn print_json_value<T: Serialize>(payload: &T) -> Result<(), FrameworkError> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    serde_json::to_writer(&mut out, payload)?;
    writeln!(out)?;
    Ok(())
}

pub fn parse_json_input<T>(raw: &str, context: &str) -> Result<T, FrameworkError>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(raw).map_err(|err| FrameworkError::Validation {
        message: format!("parse {context} input failed: {err}"),
    })
}
