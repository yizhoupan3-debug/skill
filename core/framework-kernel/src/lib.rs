#![deny(clippy::unwrap_used, clippy::expect_used)]
//! B0 root: shared traits for dependency inversion and kernel utilities.

// ── B0 core modules ──
pub mod constants;
pub mod framework_host_targets;
pub mod json_value;
pub mod repo_roots;
pub mod router_self;
pub mod runtime_registry;
pub mod time;
pub mod tokenizer;

// ── leaf modules migrated from runtime-core ──
pub mod cli_args;
pub mod skill_lint;
pub mod skill_repo;
pub mod stdio_payload_types;

// ── migrated from framework-profile crate ──
pub mod framework_profile;

// ── runtime hooks (migrated from framework-runtime-hooks crate) ──
// Placed in L0 to break circular deps — all consumers already depend on framework-kernel.
pub mod runtime_hooks;

pub use json_value::*;
pub use time::current_local_timestamp;
pub use tokenizer::{
    TokenizerProvider, install_tokenizer_provider, tokenize_query,
};

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn smoke() {
        assert!(true);
    }
}
