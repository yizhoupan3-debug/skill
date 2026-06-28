//! Hook registry for mcp-tool-registry dependency injection.
//!
//! This module re-exports `discover_tool_registry_path` from the shared
//! `routing-core` config hooks.  The module is intentionally kept as a thin
//! re-export layer so that all DI wiring stays in `routing-core` while
//! `mcp-tool-registry` (and its consumers) only depend on the narrowed
//! interface they need.
//!
//! If more hooks are added in the future (e.g. injectable path resolvers,
//! environment overrides), add them here and delegate to `routing-core`.

pub use routing_core::config_hooks::discover_tool_registry_path;
