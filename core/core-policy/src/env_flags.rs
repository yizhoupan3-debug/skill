//! Review-gate and cross-host `ROUTER_RS_*` env readers (B0 core-policy subset).
//!
//! Canonical env names take precedence; legacy per-host `ROUTER_RS_{CURSOR,CODEX,CLAUDE}_*` aliases
//! remain honored for explicit opt-in / disable (backward compatibility).
//! Operator table: see below `// §5` comments in this file.

use std::env;

const ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE_ENV: &str = "ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE";
const ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE_ENV: &str =
    "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";

const ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";
const ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";
const ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";
const ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";

const ROUTER_RS_REVIEW_GATE_DISABLE_ENV: &str = "ROUTER_RS_REVIEW_GATE_DISABLE";
const ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE_ENV: &str = "ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE";
const ROUTER_RS_CODEX_REVIEW_GATE_DISABLE_ENV: &str = "ROUTER_RS_CODEX_REVIEW_GATE_DISABLE";
const ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE_ENV: &str = "ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE";
const ROUTER_RS_MIMO_REVIEW_GATE_DISABLE_ENV: &str = "ROUTER_RS_MIMO_REVIEW_GATE_DISABLE";
const ROUTER_RS_OPENCODE_REVIEW_GATE_DISABLE_ENV: &str = "ROUTER_RS_OPENCODE_REVIEW_GATE_DISABLE";

const ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES_ENV: &str = "ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES";
const ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV: &str =
    "ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES";

const ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV: &str = "ROUTER_RS_REVIEW_PENDING_CYCLE_MAX";
const ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV: &str =
    "ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX";

/// Shared default-true env parse (`0`/`false`/`off`/`no` disable; unset = on).
pub fn env_enabled_default_true(var_name: &str) -> bool {
    match env::var(var_name) {
        Ok(value) => {
            let token = value.trim().to_ascii_lowercase();
            !(token == "0" || token == "false" || token == "off" || token == "no")
        }
        Err(_) => true,
    }
}

/// Shared default-false env parse (only `1`/`true`/`yes`/`on` enable; unset = off).
pub fn env_enabled_default_false(var_name: &str) -> bool {
    match env::var(var_name) {
        Ok(value) => {
            let token = value.trim().to_ascii_lowercase();
            matches!(token.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

fn env_explicitly_enabled(var_name: &str) -> bool {
    env_enabled_default_false(var_name)
}

pub fn router_rs_review_spawn_first_nudge_enabled() -> bool {
    env_enabled_default_true(ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE_ENV)
}

pub fn router_rs_cursor_subagent_model_inherit_nudge_enabled() -> bool {
    env_enabled_default_true(ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE_ENV)
}

/// Per-host `REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` env var mapping.
const FORK_CONTEXT_INFER_FALSE_BY_HOST: &[&str] = &[
    ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV,
    ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV,
    ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV,
];

/// Cross-host: missing `fork_context` on a reviewer lane may infer independent fork (`false`).
/// Canonical `ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`; legacy host env honored only when explicitly enabled. **Unset = off** (Claude semantics).
pub fn router_rs_review_fork_context_missing_infer_false_enabled() -> bool {
    if env_explicitly_enabled(ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV) {
        return true;
    }
    FORK_CONTEXT_INFER_FALSE_BY_HOST
        .iter()
        .any(|env| env_explicitly_enabled(env))
}

/// Per-host review gate disable env var mapping.
/// Legacy `ROUTER_RS_{HOST}_REVIEW_GATE_DISABLE` names are part of the operator
/// contract (docs §5) — do not rename. Add new hosts by appending rows.
const REVIEW_GATE_DISABLE_BY_HOST: &[(&str, &str)] = &[
    ("cursor", ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE_ENV),
    ("codex", ROUTER_RS_CODEX_REVIEW_GATE_DISABLE_ENV),
    ("claude", ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE_ENV),
    ("opencode", ROUTER_RS_OPENCODE_REVIEW_GATE_DISABLE_ENV),
    ("mimo", ROUTER_RS_MIMO_REVIEW_GATE_DISABLE_ENV),
];

/// Emergency review-gate disable for hook hosts (`cursor` / `codex` / `claude`).
///
/// Canonical `ROUTER_RS_REVIEW_GATE_DISABLE` applies to all; legacy per-host env still honored.
/// Per-host match arms are intentional: operators need granular emergency disable per host without
/// affecting others. These env var names are part of the operator contract (docs §5) and cannot be
/// replaced by registry queries without breaking existing CI/operator scripts.
pub fn router_rs_review_gate_disabled_for_host(host_id: &str) -> bool {
    if env_enabled_default_false(ROUTER_RS_REVIEW_GATE_DISABLE_ENV) {
        return true;
    }
    REVIEW_GATE_DISABLE_BY_HOST
        .iter()
        .find(|(id, _)| *id == host_id)
        .map(|(_, env)| {
            let disabled = env_enabled_default_false(env);
            if disabled {
                static WARNED: std::sync::Once = std::sync::Once::new();
                WARNED.call_once(|| {
                    eprintln!(
                        "[router-rs] deprecate: {env} is a legacy per-host env var; \
                         use ROUTER_RS_REVIEW_GATE_DISABLE=1 to disable for all hosts"
                    );
                });
            }
            disabled
        })
        .unwrap_or(false)
}

/// Max entries in `review_subagent_pending_cycle_keys` (default **32**).
pub fn router_rs_review_pending_cycle_max() -> usize {
    parse_usize_clamped(
        ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV,
        ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV,
        32,
        1,
        256,
    )
}

/// Stop `REVIEW_GATE` full-line cap before soft_nag downgrade (Cursor Stop today).
///
/// - **未设置**（非 test）：默认 **8**。
/// - `=0` / `false` / `off` / `no`：**关闭**降频（严格、每轮完整硬行）。
/// - **单测**：未设置变量时返回 **`None`**。
pub fn router_rs_review_gate_stop_max_nudges_cap() -> Option<u32> {
    let raw = env::var(ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES_ENV)
        .ok()
        .or_else(|| env::var(ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV).ok());
    parse_review_gate_stop_max_nudges_cap(raw.as_deref())
}

fn parse_review_gate_stop_max_nudges_cap(raw: Option<&str>) -> Option<u32> {
    let Some(raw) = raw else {
        #[cfg(test)]
        {
            return None;
        }
        #[cfg(not(test))]
        {
            return Some(8);
        }
    };
    let t = raw.trim().to_ascii_lowercase();
    if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
        return None;
    }
    if let Some(n) = t.parse::<u32>().ok().filter(|v| *v >= 1) {
        return Some(n);
    }
    eprintln!("[core-policy] invalid review gate stop max nudges={raw:?}; using default cap 8");
    Some(8)
}

fn parse_usize_clamped(
    canonical_key: &'static str,
    legacy_key: &'static str,
    default_val: usize,
    min_allowed: usize,
    max_allowed: usize,
) -> usize {
    let raw = env::var(canonical_key)
        .ok()
        .or_else(|| env::var(legacy_key).ok());
    match raw {
        None => default_val,
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return default_val;
            }
            match trimmed.parse::<usize>() {
                Ok(n) => n.clamp(min_allowed, max_allowed),
                Err(_) => {
                    eprintln!(
                        "[core-policy] invalid {canonical_key} (or legacy {legacy_key})={raw:?}; using default {default_val} (clamp {min_allowed}..{max_allowed})"
                    );
                    default_val
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env_sync::process_env_lock;

    #[test]
    fn review_gate_disable_canonical_or_legacy_host_env() {
        let _g = process_env_lock();
        let keys = [
            ROUTER_RS_REVIEW_GATE_DISABLE_ENV,
            ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE_ENV,
            ROUTER_RS_CODEX_REVIEW_GATE_DISABLE_ENV,
            ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE_ENV,
        ];
        let prev: Vec<_> = keys.iter().map(|k| (*k, env::var_os(k))).collect();
        for key in keys {
            unsafe { env::remove_var(key) };
        }
        assert!(!router_rs_review_gate_disabled_for_host("cursor"));
        unsafe { env::set_var(ROUTER_RS_REVIEW_GATE_DISABLE_ENV, "1") };
        assert!(router_rs_review_gate_disabled_for_host("codex"));
        unsafe { env::remove_var(ROUTER_RS_REVIEW_GATE_DISABLE_ENV) };
        unsafe { env::set_var(ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE_ENV, "true") };
        assert!(router_rs_review_gate_disabled_for_host("claude"));
        for (key, val) in prev {
            match val {
                Some(v) => unsafe { env::set_var(key, v) },
                None => unsafe { env::remove_var(key) },
            }
        }
    }

    #[test]
    fn review_pending_cycle_max_canonical_and_legacy() {
        let _g = process_env_lock();
        let prev_canon = env::var_os(ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV);
        let prev_legacy = env::var_os(ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV);
        unsafe { env::remove_var(ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV) };
        unsafe { env::remove_var(ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV) };
        assert_eq!(router_rs_review_pending_cycle_max(), 32);
        unsafe { env::set_var(ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV, "64") };
        assert_eq!(router_rs_review_pending_cycle_max(), 64);
        unsafe { env::set_var(ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV, "48") };
        assert_eq!(router_rs_review_pending_cycle_max(), 48);
        match prev_canon {
            Some(v) => unsafe { env::set_var(ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV, v) },
            None => unsafe { env::remove_var(ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV) },
        }
        match prev_legacy {
            Some(v) => unsafe { env::set_var(ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV, v) },
            None => unsafe { env::remove_var(ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV) },
        }
    }

    #[test]
    fn review_gate_stop_max_nudges_unset_in_tests_means_strict_none() {
        let _g = process_env_lock();
        let keys = [
            ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES_ENV,
            ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV,
        ];
        let prev: Vec<_> = keys.iter().map(|k| (*k, env::var_os(k))).collect();
        for key in keys {
            unsafe { env::remove_var(key) };
        }
        assert!(router_rs_review_gate_stop_max_nudges_cap().is_none());
        unsafe { env::set_var(ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV, "3") };
        assert_eq!(router_rs_review_gate_stop_max_nudges_cap(), Some(3));
        unsafe { env::remove_var(ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV) };
        unsafe { env::set_var(ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES_ENV, "5") };
        assert_eq!(router_rs_review_gate_stop_max_nudges_cap(), Some(5));
        for (key, val) in prev {
            match val {
                Some(v) => unsafe { env::set_var(key, v) },
                None => unsafe { env::remove_var(key) },
            }
        }
    }
}
