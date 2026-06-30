//! QG Route — scene-dispatched CheckerRegistry bridge.
//!
//! Owns a singleton `CheckerRegistry` initialized at startup via `init_qg_route()`.
//! Provides `evaluate_qg_route()` as the public entry point for scene-dispatched
//! quality gate evaluation.
//!
//! Wave 4a: integration bridge between quality-gate crate and runtime-core.
//! Wave 4a-ii: old `runtime-exit-gate` crate deleted — all callers use QG Route now.

use std::sync::OnceLock;

use quality_gate::types::{CheckContext, GateVerdict};

/// Type for external checker registration functions.
type ExternCheckersFn = fn(&mut quality_gate::CheckerRegistry);

/// Singleton CheckerRegistry, initialized at startup.
static QG_ROUTE: OnceLock<quality_gate::CheckerRegistry> = OnceLock::new();

/// Optional external checker registration hook.
///
/// Set by downstream crates (e.g., router-rs) that bridge between runtime-core
/// and research-harness. Called during `init_qg_route()` to register checkers
/// from other crate boundaries. Must be set before `init_qg_route()`.
static EXTERN_CHECKERS: OnceLock<ExternCheckersFn> = OnceLock::new();

/// Register an external checker registration function.
///
/// Called by the application-level init (router-rs-cli) to bridge
/// research-harness checkers into the QG Route registry.
/// Must be called before `init_qg_route()` to take effect.
pub fn set_extern_checkers(f: ExternCheckersFn) {
    if let Err(_) = EXTERN_CHECKERS.set(f) {
        tracing::warn!("set_extern_checkers called twice — second call ignored");
    }
}

/// Initialize the QG Route registry. Called once from `init_hooks()`.
/// Registers all in-place checkers plus any external checkers (via `set_extern_checkers`).
/// Safe to call multiple times.
pub fn init_qg_route() {
    QG_ROUTE.get_or_init(|| {
        let mut registry = quality_gate::CheckerRegistry::new();
        // In-place checkers from runtime-core (generated from RUNTIME_REGISTRY.json)
        crate::checkers::register_checkers_from_registry(&mut registry);
        // External checkers from research-harness (if registered)
        if let Some(f) = EXTERN_CHECKERS.get() {
            f(&mut registry);
        }
        registry
    });
}

/// Evaluate the QG Route for a given scene and context.
///
/// Returns an aggregated `GateVerdict`. Returns a blocked verdict with
/// reason "QG Route not initialized" if `init_qg_route()` has not been called,
/// preventing silent fail-open behavior.
pub fn evaluate_qg_route(scene: &str, ctx: &CheckContext) -> GateVerdict {
    match QG_ROUTE.get() {
        Some(registry) => {
            let result = registry.evaluate(scene, ctx);
            if result.checkers_ran == 0 {
                tracing::warn!(
                    "QG route evaluated scene '{scene}' with no registered checkers — gate passed by default"
                );
            }
            result
        }
        None => {
            tracing::warn!(
                "evaluate_qg_route called before init_qg_route() — returning blocked (fail-closed)"
            );
            quality_gate::types::GateVerdict::new(false, scene, 0, vec![], vec![])
                .with_reason("QG Route not initialized — returning blocked to prevent silent fail-open")
        }
    }
}
