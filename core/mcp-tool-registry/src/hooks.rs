//! Hook registry for mcp-tool-registry dependency injection.
//!
//! Delegates to the shared routing config hooks in `routing-core`.
//! Use `routing_core::config_hooks::register_routing_config_hooks` to register.

pub use routing_core::config_hooks::discover_tool_registry_path;
