#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod constants;
pub mod env_flags;
pub mod io_utils;
pub mod json_io;
pub mod json_value;
pub mod process_utils;
pub mod stdio_op_registry;
pub mod types;
pub mod util;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn smoke() {
        assert!(true);
    }
}
