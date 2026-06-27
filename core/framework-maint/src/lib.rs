#![deny(clippy::unwrap_used, clippy::expect_used)]
//! framework-maint: operational maintenance tools.
//!
//! Extracted from `runtime-core/src/framework_maint/` per ADR-010 §10.3.

pub mod maint;

// Re-export the main dispatch function
pub use maint::dispatch;


#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert!(true);
    }
}