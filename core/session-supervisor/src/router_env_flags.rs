//! Minimal env flags for session-supervisor.
//!
//! Delegates to `framework_core::env_flags` for the canonical implementations.

const ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE_ENV: &str =
    "ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE";

/// §6.4 cat.1 real-process spawn/terminate smoke (`sleep 1` via `smoke-shell` host). Opt-in for CI.
pub fn router_rs_session_supervisor_real_process_smoke_enabled() -> bool {
    framework_core::env_flags::env_enabled_default_false(
        ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE_ENV,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    #[test]
    fn unset_means_disabled_for_default_false() {
        let key = "ROUTER_RS_UNITTEST_DEFAULT_FALSE_UNSET";
        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe { core_state_utils::env_sync::remove_env(key) };
        assert!(!framework_core::env_flags::env_enabled_default_false(key));
    }

    #[test]
    fn one_true_enable_default_false() {
        let key = "ROUTER_RS_UNITTEST_DEFAULT_FALSE_ONE";
        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe { core_state_utils::env_sync::set_env(key, "1") };
        assert!(framework_core::env_flags::env_enabled_default_false(key));
        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe { core_state_utils::env_sync::remove_env(key) };
    }
}
