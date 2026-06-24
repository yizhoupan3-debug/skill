//! Research-harness hook initialization.
//!
//! Registers L5 research hooks into the L0 function-pointer registry.
//! Called explicitly from the binary entry point (`router-rs-cli.rs`).
//!
//! This decouples runtime-core (L4) from research-harness (L5) — the DAG
//! violation of L4→L5 is eliminated because L5 registers its own hooks
//! into L0, and L4 never needs to depend on L5.
//!
//! Safe to call multiple times — internal `OnceLock` guards make repeated
//! registration calls no-ops.

use host_projection::hooks;

/// Register all research hooks into the L0 function-pointer registry.
pub fn init_hooks() {
    // ── Paper prose / adversarial hooks ──
    // Closures are required because L0 expects `&'static str` for the host
    // parameter while the study functions use `&str`.
    hooks::register_paper_hooks(
        |root, prompt, lines, host| {
            super::paper_prose::maybe_append_paper_prose_context(root, prompt, lines, host)
        },
        |root, output, prompt, followup, host| {
            super::paper_prose::maybe_merge_paper_prose_before_submit(
                root, output, prompt, followup, host,
            )
        },
        |root, prompt, lines, host| {
            super::paper_adversarial::maybe_append_paper_adversarial_context(
                root, prompt, lines, host,
            )
        },
        |root, output, prompt, followup, host| {
            super::paper_adversarial::maybe_merge_paper_adversarial_before_submit(
                root, output, prompt, followup, host,
            )
        },
    );

    // ── Research activity log hook ──
    hooks::register_research_activity_hook(
        |root, tool, summary| {
            if let Err(e) = super::activity_log::maybe_log_research_activity(tool, summary, root) {
                eprintln!("[research-activity-log] failed: {e}");
            }
        },
    );

    // ── Research tool dispatch ──
    hooks::register_research_tool_dispatch(crate::mcp_tools::handle_research_tool);

    // ── Research mode inference (deep/quick) ──
    crate::research_mode::register_research_mode_inference();
}
