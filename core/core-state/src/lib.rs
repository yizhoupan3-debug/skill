/// No-op initialization hook (deprecated).
///
/// Historical placeholder retained for backward compatibility with callers
/// that expect a crate-level `init()` entry point.  The core-state crate
/// requires no explicit initialization — all subsystems use `OnceLock` / lazy
/// statics for deferred setup.
#[deprecated(
    since = "0.1.0",
    note = "core-state requires no explicit initialization; all subsystems use OnceLock/lazy statics. Remove this call."
)]
pub fn init() {}

pub mod goal_prediction;
pub mod rfv_loop;
pub mod state_manager;
pub mod step_ledger;
pub mod task_ledger;
pub mod task_state;
pub mod task_state_aggregate;
pub mod utils;
