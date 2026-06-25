#![deny(clippy::unwrap_used, clippy::expect_used)]
//! Runtime-core contracts: pure-data / trait-only modules extracted from runtime-core.
//!
//! This crate MUST NOT depend on implementation crates (framework-runtime,
//! host-projection). Only leaf crates (core-state, core-policy, framework-kernel)
//! or external crates are allowed.

pub mod formal_toolchain;
pub mod harness_context_signals;
pub mod harness_contract;
pub mod hook_event_routing;
pub mod hook_observation_rules;
pub mod hook_outbound_protect;
pub mod mcp_pre_guard;
pub mod web_fetch_guard;
