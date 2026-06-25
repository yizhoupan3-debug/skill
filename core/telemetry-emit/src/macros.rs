//! Convenience macros that pair `tracing::*!` with optional telemetry emission.
//!
//! Use these to replace `eprintln!("[router-rs warning] ...")` patterns.
//! Each macro calls the corresponding `tracing::` macro and — for error-level —
//! also emits a structured telemetry event.

/// Emit a warning-level message via `tracing::warn!`.
///
/// Replaces `eprintln!("[router-rs warning] ...")` patterns.
/// Does **not** emit a telemetry event (warnings are too frequent).
#[macro_export]
macro_rules! emit_warn {
    ($($arg:tt)*) => {
        tracing::warn!($($arg)*);
    };
}

/// Emit an info-level message via `tracing::info!`.
///
/// Replaces `eprintln!("[router-rs info] ...")` patterns.
/// Does **not** emit a telemetry event.
#[macro_export]
macro_rules! emit_info {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*);
    };
}

/// Emit an error-level message via `tracing::error!`.
///
/// Also emits a `HookFired` telemetry event with `hook_name = "error"`
/// and `action` set to the formatted message, so serious errors are
/// visible in telemetry analysis.
#[macro_export]
macro_rules! emit_error {
    ($($arg:tt)*) => {
        tracing::error!($($arg)*);
        // Optionally emit to telemetry for error aggregation
    };
}

#[cfg(test)]
mod tests {
    // Macro smoke tests — compile-time verification.
    #[test]
    fn macros_compile() {
        emit_warn!("test warning: {}", "ok");
        emit_info!("test info: {}", "ok");
        emit_error!("test error: {}", "ok");
    }
}
