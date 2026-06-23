// Re-export all modules from core-state-utils for backward compatibility.
// Downstream code using `core_state::utils::*` continues to work unchanged.
pub use core_state_utils::atomic_write;
pub use core_state_utils::json_io;
pub use core_state_utils::jsonl_maintenance;
pub use core_state_utils::path_guard;
pub use core_state_utils::read_bounded;
pub use core_state_utils::task_write_lock;

#[cfg(test)]
pub mod test_helpers;
