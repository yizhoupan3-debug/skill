//! Deprecated re-export shim — prefer `quality_gate` module.
//!
//! All public items from the old `rfv_loop` module are re-exported here with
//! `#[deprecated]` attributes pointing to their new names in `quality_gate`.

#[deprecated(since = "9.1.0", note = "use `quality_gate::framework_quality_gate`")]
pub use crate::quality_gate::framework_quality_gate as framework_rfv_loop;

#[deprecated(since = "9.1.0", note = "use `quality_gate::QUALITY_GATE_LOOP_SCHEMA_VERSION`")]
pub use crate::quality_gate::QUALITY_GATE_LOOP_SCHEMA_VERSION as RFV_LOOP_SCHEMA_VERSION;
