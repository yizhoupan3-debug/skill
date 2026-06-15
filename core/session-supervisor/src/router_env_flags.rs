//! Minimal env flags for session-supervisor (standalone copy from runtime-core).
//!
//! Contains only the flags needed by the session-supervisor crate.
//! The canonical source remains `core/runtime-core/src/contracts/router_env_flags.rs`.

use std::env;

const ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE_ENV: &str =
    "ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE";

fn env_enabled_default_false(var_name: &str) -> bool {
    match env::var(var_name) {
        Err(_) => false,
        Ok(raw) => {
            let t = raw.trim().to_ascii_lowercase();
            !matches!(t.as_str(), "0" | "false" | "off" | "no" | "")
        }
    }
}

/// §6.4 cat.1 real-process spawn/terminate smoke (`sleep 1` via `smoke-shell` host). Opt-in for CI.
pub fn router_rs_session_supervisor_real_process_smoke_enabled() -> bool {
    env_enabled_default_false(ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE_ENV)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_means_disabled_for_default_false() {
        let key = "ROUTER_RS_UNITTEST_DEFAULT_FALSE_UNSET";
        unsafe { env::remove_var(key) };
        assert!(!env_enabled_default_false(key));
    }

    #[test]
    fn one_true_enable_default_false() {
        let key = "ROUTER_RS_UNITTEST_DEFAULT_FALSE_ONE";
        unsafe { env::set_var(key, "1") };
        assert!(env_enabled_default_false(key));
        unsafe { env::remove_var(key) };
    }
}
