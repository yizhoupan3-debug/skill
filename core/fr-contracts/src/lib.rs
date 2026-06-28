#![deny(clippy::unwrap_used, clippy::expect_used)]
pub mod execution_contract;
pub mod pre_tool_use_guard;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn smoke() {
        assert!(true);
    }
}
