//! Minimal env flags for session-supervisor.
//!
//! Delegates to `core_policy::env_flags` for the canonical implementations.

const ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE_ENV: &str =
    "ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE";

/// §6.4 cat.1 real-process spawn/terminate smoke (`sleep 1` via `smoke-shell` host). Opt-in for CI.
pub fn router_rs_session_supervisor_real_process_smoke_enabled() -> bool {
    core_policy::env_flags::env_enabled_default_false(ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE_ENV)
}

#[cfg(test)]
mod tests {

    #[test]
    fn unset_means_disabled_for_default_false() {
        let key = "ROUTER_RS_UNITTEST_DEFAULT_FALSE_UNSET";
        unsafe { std::env::remove_var(key) };
        assert!(!core_policy::env_flags::env_enabled_default_false(key));
    }

    #[test]
    fn one_true_enable_default_false() {
        let key = "ROUTER_RS_UNITTEST_DEFAULT_FALSE_ONE";
        unsafe { std::env::set_var(key, "1") };
        assert!(core_policy::env_flags::env_enabled_default_false(key));
        unsafe { std::env::remove_var(key) };
    }
}
