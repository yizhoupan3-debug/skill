//! runtime-core: extracted runtime modules from router-rs.
//!
//! Contains runtime_storage, background_state, trace_runtime, and runtime_envelope_ids.
//! Session supervisor lives in router-rs (native process driver).

pub mod background_state;
pub mod runtime_envelope_ids;
pub mod runtime_storage;
pub mod trace_runtime;
