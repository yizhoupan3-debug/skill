//! GateChecker trait — a pluggable quality gate checker.
//!
//! Every checker implements this trait. Checkers are registered into
//! `CheckerRegistry` via `register()` at startup. The trait is Send + Sync
//! so checkers can be shared across threads safely.

use crate::types::{CheckContext, CheckResult};

/// A single quality gate checker.
///
/// Implementations are in-place (see roadmap D007): each checker lives
/// in the module where its logic naturally belongs, then wraps itself
/// with `impl GateChecker` for registration.
///
/// Scene dispatch is handled by `CheckerRegistry` via `register(scene, checker)`.
/// The checker itself does not declare its scene — the registration call determines
/// which scene(s) the checker runs under.
///
/// # Sub-scene filtering (Wave 6)
///
/// Override [`sub_scene_affinity`](Self::sub_scene_affinity) to limit this
/// checker to a specific sub-scene. The checker only runs when the evaluation
/// context's `sub_scene` matches. `None` (default) = runs for all sub-scenes.
pub trait GateChecker: Send + Sync {
    /// Unique, stable identifier for this checker (e.g. "adversarial").
    fn id(&self) -> &'static str;
    /// Human-readable description of what this checker validates.
    fn description(&self) -> &'static str;
    /// Run the check against the given context.
    ///
    /// Implementations MUST be synchronous. For async work (HTTP calls,
    /// file I/O, etc.), use `ctx.runtime_handle` with `block_in_place`
    /// or `Handle::block_on`.
    fn check(&self, ctx: &CheckContext) -> CheckResult;

    /// Optional sub-scene affinity (Wave 6).
    ///
    /// Return `Some("sub_scene_name")` to limit this checker to a specific
    /// sub-scene. Return `None` (default) to run for all sub-scenes.
    fn sub_scene_affinity(&self) -> Option<&'static str> {
        None
    }
}
