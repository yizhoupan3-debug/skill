//! Durable background job state store (filesystem / sqlite / memory backends).

mod control_plane;
mod persist;
mod status;
mod store;
mod types;

pub use persist::handle_background_state_operation;

#[cfg(test)]
mod tests;
