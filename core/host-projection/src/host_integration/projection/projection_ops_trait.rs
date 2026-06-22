//! Registry-driven projection operations trait.
//!
//! Each host implements `HostProjectionOps` for install/status/remove.
//! The dispatch functions in `mod.rs` use the registry to look up the
//! correct implementation, eliminating hardcoded `const TABLE` dispatch.

use super::ResolvedProjectionRoots;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Trait for host-specific projection operations.
/// Each host implements install/status/remove; the registry dispatches dynamically.
pub trait HostProjectionOps: Send + Sync {
    fn host_id(&self) -> &'static str;

    fn install(&self, roots: &ResolvedProjectionRoots, scope: &str) -> Result<Value, String>;

    fn status(&self, roots: &ResolvedProjectionRoots) -> Result<Value, String>;

    fn remove(
        &self,
        roots: &ResolvedProjectionRoots,
        scope: &str,
        dry_run: bool,
    ) -> Result<Value, String>;
}

static PROJECTION_OPS_REGISTRY: OnceLock<HashMap<&'static str, Box<dyn HostProjectionOps>>> =
    OnceLock::new();

fn build_projection_ops_registry() -> HashMap<&'static str, Box<dyn HostProjectionOps>> {
    let mut m: HashMap<&'static str, Box<dyn HostProjectionOps>> = HashMap::new();

    // Register hosts with custom projection ops
    m.insert(
        "cursor",
        Box::new(super::projection_host_ops::CursorProjectionOps),
    );
    m.insert(
        "claude",
        Box::new(super::projection_host_ops::ClaudeProjectionOps),
    );
    m.insert(
        "opencode",
        Box::new(super::projection_host_ops::OpencodeProjectionOps),
    );
    m.insert(
        "codex",
        Box::new(super::projection_host_ops::CodexProjectionOps),
    );

    m
}

/// Look up projection ops by tool name (install_tool spelling).
pub fn projection_ops_for_tool(tool: &str) -> Option<&'static dyn HostProjectionOps> {
    let registry = PROJECTION_OPS_REGISTRY.get_or_init(build_projection_ops_registry);
    registry.get(tool).map(|boxed| boxed.as_ref())
}
