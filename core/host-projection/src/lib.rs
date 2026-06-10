//! host-projection: host provider abstraction, host integration, and entrypoint sync.
//!
//! Physical migration from runtime-core:
//! - hosts/host_provider.rs: HostProvider trait + registry
//! - hosts/hook_state_common.rs: hook state version adapter
//! - host_integration/: install/status/remove/roots/artifacts
//! - host_entrypoint_sync.rs: host entrypoint sync

pub mod host_entrypoint_sync;
pub mod host_integration;
pub mod hooks;
pub mod hosts;
