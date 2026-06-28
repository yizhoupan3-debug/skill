//! Research-harness hook initialization.
//!
//! Registers L5 research hooks into the L0 RuntimeHooks struct.
//! Called explicitly from the binary entry point (`router-rs-cli.rs`).
//!
//! This decouples runtime-core (L4) from research-harness (L5) — the DAG
//! violation of L4→L5 is eliminated because L5 registers its own hooks
//! directly into L0 via `modify_runtime_hooks()`, and L4 never depends on L5.

use host_projection::hooks;

/// Register all research hooks into the L0 RuntimeHooks struct.
/// Safe to call multiple times — `modify_runtime_hooks` is idempotent.
pub fn init_hooks() {
    hooks::modify_runtime_hooks(|h| {
        h.maybe_append_paper_prose_context = super::paper_prose::maybe_append_paper_prose_context;
        h.maybe_merge_paper_prose_before_submit =
            super::paper_prose::maybe_merge_paper_prose_before_submit;
        h.maybe_append_paper_adversarial_context =
            super::paper_adversarial::maybe_append_paper_adversarial_context;
        h.maybe_merge_paper_adversarial_before_submit =
            super::paper_adversarial::maybe_merge_paper_adversarial_before_submit;
        h.maybe_record_research_activity = |root, tool, summary| {
            if let Err(e) = super::activity_log::maybe_log_research_activity(tool, summary, root) {
                tracing::warn!("[research-activity-log] failed: {e}");
            }
        };
        h.research_tool_dispatch = crate::mcp_tools::handle_research_tool;
    });
}
