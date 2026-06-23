//! host-projection: host provider abstraction and entrypoint sync.
//!
//! Physical migration from runtime-core:
//! - hosts/host_provider.rs: HostProvider trait + registry
//! - hosts/host_state.rs: AgentDiskState, TouchState, review gate state
//! - hosts/generic_config.rs: GenericHostConfig (data-driven, no host names)
//! - host_integration/ → migrated to runtime-core (B layer)
//! - host_entrypoint_sync.rs: host entrypoint sync

// mcp_stdio_harness/mod.rs 的 tools/list 响应使用深度嵌套的 json!() 宏，默认 128 层不够。
#![recursion_limit = "256"]

pub mod hooks;
pub mod host_entrypoint_sync;
pub mod host_integration;
pub mod hosts;

#[cfg(test)]
pub(crate) mod test_helpers;
