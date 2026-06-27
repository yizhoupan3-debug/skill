#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod closeout_validation;
pub mod transition_validation;
pub mod error;
pub mod goal_prediction;
pub mod state_manager;
pub mod step_ledger;
pub mod task_ledger;
pub mod task_state;
pub mod utils;
#[cfg(test)]
mod proptests;
pub use error::StateError;
