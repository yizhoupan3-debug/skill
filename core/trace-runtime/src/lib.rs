#![deny(clippy::unwrap_used, clippy::expect_used)]
pub mod error;
pub use error::TraceError;
pub use trace_record::*;
mod trace_record;
