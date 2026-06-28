#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod goal_prediction;
pub mod task_state_types;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn smoke() {
        assert!(true);
    }
}
