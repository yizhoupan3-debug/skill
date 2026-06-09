mod hook_cli;
mod repo_root;
mod stdin;

pub use hook_cli::run_cursor_hook_cli_with_timing;
pub use repo_root::resolve_cursor_hook_repo_root;
pub use stdin::read_cursor_hook_stdin_json;

pub mod subtraction;
pub mod terminal_observation_cache;

pub use subtraction::{CURSOR_HOOKS_REGISTERED_EVENTS, CURSOR_HOOKS_SUBTRACTED_EVENTS};

mod handlers;
pub use handlers::{
    parse_terminal_header, TerminalObservation,
};
pub use handlers::*;

#[cfg(test)]
pub use handlers::{
    set_force_cursor_hook_state_lock_failure, set_test_review_gate_disable_override,
};
