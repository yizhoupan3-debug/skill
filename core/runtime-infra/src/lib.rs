#![deny(clippy::unwrap_used, clippy::expect_used)]
//! runtime-infra: shared infrastructure (B0-level semantics, L4 dependencies).
//!
//! Extracted from `runtime-core/src/infrastructure/` per ADR-010 §10.3.

pub mod kernel_bootstrap;
pub mod kernel_utils;
pub mod stdio_transport;


#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert!(true);
    }
}