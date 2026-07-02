//! Hook registry for runtime-core dependency injection.
//!
//! Routing-engine is a leaf crate with zero internal path dependencies.
//! The functions in this module allow runtime-core (or any host crate) to register
//! callbacks that the routing engine needs at runtime (host aliases, kernel bootstrap,
//! review prompt detection, skill repo discovery, parallel review markers).
//!
//! All hooks are **optional** — unregistered hooks return safe defaults.
//! The host crate should call `register_hooks` once at startup.

use std::path::PathBuf;
use std::sync::OnceLock;

use core_errors::FrameworkError;

// ---------------------------------------------------------------------------
// ParallelReviewMarkers (mirrors framework_core::review_routing_signals)
// ---------------------------------------------------------------------------

/// Markers used by `has_parallel_review_candidate_context`.
/// Mirrors the structure from `core-policy::review_routing_signals`.
pub struct ParallelReviewMarkers {
    pub review_markers: &'static [String],
    pub breadth_markers: &'static [String],
    pub scope_markers: &'static [String],
}

// ---------------------------------------------------------------------------
// Function-pointer hook registry
// ---------------------------------------------------------------------------

type IsReviewPromptFn = fn(&str) -> bool;
type HostProviderRoutingAliasesFn = fn(&str) -> Vec<String>;
type DiscoverSkillRepoRootFn = fn() -> Option<PathBuf>;
type SkillRoutingRuntimeJsonFn = fn(&std::path::Path) -> PathBuf;
type ParallelReviewMarkersFn = fn() -> ParallelReviewMarkers;

struct RoutingHooks {
    is_review_prompt: IsReviewPromptFn,
    host_provider_routing_aliases: HostProviderRoutingAliasesFn,
    discover_skill_repo_root: DiscoverSkillRepoRootFn,
    skill_routing_runtime_json: SkillRoutingRuntimeJsonFn,
    parallel_review_markers: ParallelReviewMarkersFn,
}

static HOOKS: OnceLock<RoutingHooks> = OnceLock::new();

/// Register all routing hooks. Should be called once from runtime-core at startup.
/// Returns `Err` if hooks were already registered.
pub fn register_hooks(
    is_review_prompt: IsReviewPromptFn,
    host_provider_routing_aliases: HostProviderRoutingAliasesFn,
    discover_skill_repo_root: DiscoverSkillRepoRootFn,
    skill_routing_runtime_json: SkillRoutingRuntimeJsonFn,
    parallel_review_markers: ParallelReviewMarkersFn,
) -> Result<(), FrameworkError> {
    HOOKS
        .set(RoutingHooks {
            is_review_prompt,
            host_provider_routing_aliases,
            discover_skill_repo_root,
            skill_routing_runtime_json,
            parallel_review_markers,
        })
        .map_err(|_| FrameworkError::Hook {
            message: "routing hooks already registered".into(),
        })
}

// ---------------------------------------------------------------------------
// Public accessors (safe defaults when hooks are not registered)
// ---------------------------------------------------------------------------

/// Check if the query text looks like a review prompt.
/// Default: always false.
pub fn is_review_prompt(query_text: &str) -> bool {
    HOOKS
        .get()
        .map(|h| (h.is_review_prompt)(query_text))
        .unwrap_or(false)
}

/// Get host provider routing aliases for a host ID.
/// Default: returns just the host_id itself lowercased.
pub fn host_provider_routing_aliases(host_id: &str) -> Vec<String> {
    HOOKS
        .get()
        .map(|h| (h.host_provider_routing_aliases)(host_id))
        .unwrap_or_else(|| vec![host_id.trim().to_ascii_lowercase()])
}

/// Discover the skill policy repository root.
/// Default: None.
pub fn discover_skill_repo_root() -> Option<PathBuf> {
    HOOKS.get().and_then(|h| (h.discover_skill_repo_root)())
}

/// Resolve the skill routing runtime JSON path from a repo root.
/// Default: root / "skills" / "SKILL_ROUTING_RUNTIME.json"
pub fn skill_routing_runtime_json(root: &std::path::Path) -> PathBuf {
    HOOKS
        .get()
        .map(|h| (h.skill_routing_runtime_json)(root))
        .unwrap_or_else(|| root.join("skills").join("SKILL_ROUTING_RUNTIME.json"))
}

/// Get parallel review candidate markers.
///
/// Returns empty marker lists when hooks are not registered — all parallel review
/// detection is disabled. Callers should treat empty markers as "no parallel review
/// candidates exist" rather than assuming registration has already occurred.
/// Default: returns empty marker lists (no parallel review detection).
pub fn parallel_review_candidate_markers() -> ParallelReviewMarkers {
    HOOKS
        .get()
        .map(|h| (h.parallel_review_markers)())
        .unwrap_or_else(|| ParallelReviewMarkers {
            review_markers: &[],
            breadth_markers: &[],
            scope_markers: &[],
        })
}

/// Reset routing hooks for test isolation.
///
/// # Safety
/// Only safe in single-threaded test contexts. Replaces the global
/// `OnceLock` in-place; concurrent readers will see permanently unset
/// accessor defaults after this returns.
#[cfg(test)]
pub fn unregister_routing_hooks() {
    #[allow(invalid_reference_casting)]
    // SAFETY: #[cfg(test)] only — replaces the OnceLock interior without
    // dropping the old value. No concurrent access because cargo test
    // serializes within a binary.
    unsafe {
        let ptr = &HOOKS as *const OnceLock<RoutingHooks> as *mut OnceLock<RoutingHooks>;
        std::ptr::drop_in_place(ptr);
        std::ptr::write(ptr, OnceLock::new());
    }
}
