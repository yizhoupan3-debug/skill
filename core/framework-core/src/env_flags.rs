//! Review-gate and cross-host `ROUTER_RS_*` env readers (B0 core-policy subset).
//!
//! Canonical env names take precedence; legacy per-host `ROUTER_RS_{CURSOR,CODEX,CLAUDE}_*` aliases
//! remain honored for explicit opt-in / disable (backward compatibility).
//! Operator table: see below `// §5` comments in this file.

use std::cell::Cell;
use std::env;

// ────────────────────────────────────────────────────────────────
// §0  Test-only thread-local env overrides (shared by all hosts)
// ────────────────────────────────────────────────────────────────

thread_local! {
    /// Test-only override for `router_rs_review_gate_disabled_for_host`.
    /// When set (via [`set_test_review_gate_disabled_override`]), bypasses real env lookup.
    static TEST_REVIEW_GATE_DISABLED_OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Set a test-only override for review-gate disable check (all hosts).
/// Pass `None` to clear. Only effective under `#[cfg(test)]` or with `test-sync` feature.
#[cfg(any(test, feature = "test-sync"))]
pub fn set_test_review_gate_disabled_override(value: Option<bool>) {
    TEST_REVIEW_GATE_DISABLED_OVERRIDE.with(|c| c.set(value));
}

/// Returns the test override value if set, otherwise `None`.
/// Available in both test and non-test builds for call-site compatibility;
/// the setter is `#[cfg(test)]` only.
pub fn test_review_gate_disabled_override() -> Option<bool> {
    TEST_REVIEW_GATE_DISABLED_OVERRIDE.with(|c| c.get())
}

const ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE_ENV: &str = "ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE";
const ROUTER_RS_SUBAGENT_MODEL_INHERIT_NUDGE_ENV: &str = "ROUTER_RS_SUBAGENT_MODEL_INHERIT_NUDGE";
const ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE_ENV: &str =
    "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
const ROUTER_RS_TASK_LEDGER_FLOCK_ENV: &str = "ROUTER_RS_TASK_LEDGER_FLOCK";

const ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";
const ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";
const ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";
const ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";
const ROUTER_RS_OPENCODE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV: &str =
    "ROUTER_RS_OPENCODE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";

/// Canonical review-gate disable env (all hosts).
pub(crate) const ROUTER_RS_REVIEW_GATE_DISABLE_ENV: &str = "ROUTER_RS_REVIEW_GATE_DISABLE";

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

/// Canonical `ROUTER_RS_SUBAGENT_MODEL_INHERIT_NUDGE`; legacy `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE` still honored.
/// Canonical explicitly set wins over legacy; unset falls through to legacy.
pub fn router_rs_subagent_model_inherit_nudge_enabled() -> bool {
    if std::env::var(ROUTER_RS_SUBAGENT_MODEL_INHERIT_NUDGE_ENV).is_ok() {
        return env_enabled_default_true(ROUTER_RS_SUBAGENT_MODEL_INHERIT_NUDGE_ENV);
    }
    env_enabled_default_true(ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE_ENV)
}

/// `ROUTER_RS_TASK_LEDGER_FLOCK`（default ON）：是否对任务账本写入使用 flock sentinel。
/// `0`/`false`/`off`/`no` 禁用；未设置 = 启用。
pub fn router_rs_task_ledger_flock_enabled() -> bool {
    env_enabled_default_true(ROUTER_RS_TASK_LEDGER_FLOCK_ENV)
}

/// Per-host `REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` env var mapping.
const FORK_CONTEXT_INFER_FALSE_BY_HOST: &[&str] = &[
    ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV,
    ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV,
    ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV,
    ROUTER_RS_OPENCODE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV,
    ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE_ENV, // opencode uses canonical
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

/// Emergency review-gate disable for hook hosts.
///
/// Canonical `ROUTER_RS_REVIEW_GATE_DISABLE` applies to all; legacy per-host env still honored.
/// Per-host env var names are part of the operator contract (docs §5) and cannot be replaced by
/// registry queries without breaking existing CI/operator scripts.
///
/// Host→env mapping is generated from `RUNTIME_REGISTRY.json host_targets.metadata.*.review_gate_disable_env`.
pub fn router_rs_review_gate_disabled_for_host(host_id: &str) -> bool {
    #[cfg(test)]
    if let Some(v) = test_review_gate_disabled_override() {
        return v;
    }
    if env_enabled_default_false(ROUTER_RS_REVIEW_GATE_DISABLE_ENV) {
        return true;
    }
    let env = crate::runtime_registry::review_gate_disable_env(host_id);
    if env.is_empty() {
        return false;
    }
    let disabled = env_enabled_default_false(env);
    if disabled {
        static WARNED: std::sync::Once = std::sync::Once::new();
        WARNED.call_once(|| {
            tracing::warn!(
                "[router-rs] deprecate: {env} is a legacy per-host env var; \
                 use ROUTER_RS_REVIEW_GATE_DISABLE=1 to disable for all hosts"
            );
        });
    }
    disabled
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

/// Stop `REVIEW_GATE` full-line cap before soft_nag downgrade (all hosts).
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
    tracing::warn!(
        "[core-policy] invalid review gate stop max nudges={raw:?}; using default cap 8"
    );
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
                    tracing::warn!(
                        "[core-policy] invalid {canonical_key} (or legacy {legacy_key})={raw:?}; using default {default_val} (clamp {min_allowed}..{max_allowed})"
                    );
                    default_val
                }
            }
        }
    }
}

// ── L0/L4 shared env readers (consolidated from fr-exec / host-projection hooks.rs) ──

/// Thin alias for fr-exec / hooks.rs re-export chains.
pub fn router_rs_env_enabled_default_true(var_name: &str) -> bool {
    env_enabled_default_true(var_name)
}

/// Thin alias for fr-exec / hooks.rs re-export chains.
pub fn router_rs_env_enabled_default_false(var_name: &str) -> bool {
    env_enabled_default_false(var_name)
}

pub fn router_rs_operator_inject_globally_enabled() -> bool {
    env_enabled_default_true("ROUTER_RS_OPERATOR_INJECT")
}

/// `ROUTER_RS_PRE_GOAL_ENABLED` — default false (opt-in).
pub fn router_rs_pre_goal_enabled() -> bool {
    env_enabled_default_false("ROUTER_RS_PRE_GOAL_ENABLED")
}

/// Canonical `ROUTER_RS_HOOK_SILENT`; legacy `ROUTER_RS_CURSOR_HOOK_SILENT` — default false.
pub fn router_rs_hook_silent_enabled() -> bool {
    #[cfg(not(test))]
    {
        use std::sync::OnceLock;
        static CACHE: OnceLock<bool> = OnceLock::new();
        return *CACHE.get_or_init(|| {
            env_enabled_default_false("ROUTER_RS_HOOK_SILENT")
                || env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_SILENT")
        });
    }
    #[cfg(test)]
    {
        env_enabled_default_false("ROUTER_RS_HOOK_SILENT")
            || env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_SILENT")
    }
}

/// Hook outbound context max chars (clamped [1024, 65536], default 8192).
/// Canonical `ROUTER_RS_HOOK_OUTBOUND_CONTEXT_MAX_CHARS`; legacy `ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS`.
pub fn router_rs_hook_outbound_context_max_bytes() -> usize {
    #[cfg(not(test))]
    {
        use std::sync::OnceLock;
        static CACHE: OnceLock<usize> = OnceLock::new();
        return *CACHE.get_or_init(|| {
            parse_usize_clamped(
                "ROUTER_RS_HOOK_OUTBOUND_CONTEXT_MAX_CHARS",
                "ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS",
                8192,
                1024,
                65536,
            )
        });
    }
    #[cfg(test)]
    {
        parse_usize_clamped(
            "ROUTER_RS_HOOK_OUTBOUND_CONTEXT_MAX_CHARS",
            "ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS",
            8192,
            1024,
            65536,
        )
    }
}

/// Canonical `ROUTER_RS_PRE_GOAL_STRICT_DISK`; legacy `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK`.
///
/// Canonical explicitly set → evaluate only canonical (default_true: set to "0"/"false" = disabled);
/// canonical unset → fall through to legacy (default_true: unset = enabled).
/// This preserves backward compatibility: `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` alone works.
pub fn router_rs_pre_goal_strict_disk_enabled() -> bool {
    if std::env::var("ROUTER_RS_PRE_GOAL_STRICT_DISK").is_ok() {
        return env_enabled_default_true("ROUTER_RS_PRE_GOAL_STRICT_DISK");
    }
    env_enabled_default_true("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK")
}

/// Canonical `ROUTER_RS_HOOK_STATE_FAIL_OPEN`; legacy `ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN`.
pub fn router_rs_hook_state_fail_open_enabled() -> bool {
    #[cfg(not(test))]
    {
        use std::sync::OnceLock;
        static CACHE: OnceLock<bool> = OnceLock::new();
        return *CACHE.get_or_init(|| {
            env_enabled_default_false("ROUTER_RS_HOOK_STATE_FAIL_OPEN")
                || env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN")
        });
    }
    #[cfg(test)]
    {
        env_enabled_default_false("ROUTER_RS_HOOK_STATE_FAIL_OPEN")
            || env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN")
    }
}

fn parse_env_u32(key: &str) -> Option<u32> {
    std::env::var(key).ok().and_then(|v| v.trim().parse().ok())
}

/// Canonical `ROUTER_RS_HOOK_STATE_LOCK_RETRIES`; legacy `ROUTER_RS_CURSOR_HOOK_STATE_LOCK_RETRIES` (default 100).
pub fn router_rs_hook_state_lock_retries() -> u32 {
    #[cfg(not(test))]
    {
        use std::sync::OnceLock;
        static CACHE: OnceLock<u32> = OnceLock::new();
        return *CACHE.get_or_init(|| {
            parse_env_u32("ROUTER_RS_HOOK_STATE_LOCK_RETRIES")
                .or_else(|| parse_env_u32("ROUTER_RS_CURSOR_HOOK_STATE_LOCK_RETRIES"))
                .unwrap_or(100)
        });
    }
    #[cfg(test)]
    {
        parse_env_u32("ROUTER_RS_HOOK_STATE_LOCK_RETRIES")
            .or_else(|| parse_env_u32("ROUTER_RS_CURSOR_HOOK_STATE_LOCK_RETRIES"))
            .unwrap_or(100)
    }
}

/// Canonical `ROUTER_RS_HOOK_STATE_FILE_SYNC`; legacy `ROUTER_RS_CURSOR_HOOK_STATE_FILE_SYNC`.
pub fn router_rs_hook_state_file_sync_enabled() -> bool {
    #[cfg(not(test))]
    {
        use std::sync::OnceLock;
        static CACHE: OnceLock<bool> = OnceLock::new();
        return *CACHE.get_or_init(|| {
            env_enabled_default_false("ROUTER_RS_HOOK_STATE_FILE_SYNC")
                || env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FILE_SYNC")
        });
    }
    #[cfg(test)]
    {
        env_enabled_default_false("ROUTER_RS_HOOK_STATE_FILE_SYNC")
            || env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FILE_SYNC")
    }
}

/// Canonical `ROUTER_RS_HOOK_STATE_DIR_SYNC`; legacy `ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC`.
pub fn router_rs_hook_state_dir_sync_enabled() -> bool {
    #[cfg(not(test))]
    {
        use std::sync::OnceLock;
        static CACHE: OnceLock<bool> = OnceLock::new();
        return *CACHE.get_or_init(|| {
            env_enabled_default_false("ROUTER_RS_HOOK_STATE_DIR_SYNC")
                || env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC")
        });
    }
    #[cfg(test)]
    {
        env_enabled_default_false("ROUTER_RS_HOOK_STATE_DIR_SYNC")
            || env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC")
    }
}

/// Canonical `ROUTER_RS_CARGO_CHECK_SYNC`; legacy `ROUTER_RS_CURSOR_CARGO_CHECK_SYNC`.
pub fn router_rs_cargo_check_sync_enabled() -> bool {
    env_enabled_default_false("ROUTER_RS_CARGO_CHECK_SYNC")
        || env_enabled_default_false("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC")
}

/// Canonical `ROUTER_RS_HOOK_STATE_LEGACY_FULL_SWEEP`; legacy `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP`.
pub fn router_rs_hook_state_legacy_full_sweep_enabled() -> bool {
    env_enabled_default_false("ROUTER_RS_HOOK_STATE_LEGACY_FULL_SWEEP")
        || env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP")
}

fn parse_env_u64(key: &str) -> Option<u64> {
    std::env::var(key).ok().and_then(|v| v.trim().parse().ok())
}

/// Canonical `ROUTER_RS_HOOK_STATE_STALE_SWEEP_DAYS`; legacy `ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS` (default 7).
pub fn router_rs_hook_state_stale_sweep_days() -> u64 {
    parse_env_u64("ROUTER_RS_HOOK_STATE_STALE_SWEEP_DAYS")
        .or_else(|| parse_env_u64("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS"))
        .unwrap_or(7)
}

/// Canonical `ROUTER_RS_HOOK_LEGACY_SUBTRACTED_EVENTS`; legacy `ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS`.
pub fn router_rs_hook_legacy_subtracted_events_enabled() -> bool {
    env_enabled_default_false("ROUTER_RS_HOOK_LEGACY_SUBTRACTED_EVENTS")
        || env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS")
}

// ── Safe env var wrappers (eliminates ~100 unsafe blocks across crates) ──

/// Set an environment variable. Wraps `std::env::set_var` (unsafe due to
/// thread-safety concern) into a single call site, reducing repeated unsafe blocks.
pub fn set_env(key: &str, val: &str) {
    unsafe { core_state_utils::env_sync::set_env(key, val) }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::test_env_sync::process_env_lock;

    #[test]
    fn review_gate_disable_canonical_or_legacy_host_env() {
        let _g = process_env_lock();
        let keys = [
            ROUTER_RS_REVIEW_GATE_DISABLE_ENV,
            "ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE",
            "ROUTER_RS_CODEX_REVIEW_GATE_DISABLE",
            "ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE",
            "ROUTER_RS_OPENCODE_REVIEW_GATE_DISABLE",
        ];
        let prev: Vec<_> = keys.iter().map(|k| (*k, env::var_os(k))).collect();
        for key in keys {
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            unsafe { env::remove_var(key) };
        }
        assert!(!router_rs_review_gate_disabled_for_host("cursor"));
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(ROUTER_RS_REVIEW_GATE_DISABLE_ENV, "1") };
        assert!(router_rs_review_gate_disabled_for_host("codex"));
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(ROUTER_RS_REVIEW_GATE_DISABLE_ENV) };
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", "true") };
        assert!(router_rs_review_gate_disabled_for_host("claude"));
        for (key, val) in prev {
            match val {
                // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
                Some(v) => unsafe { env::set_var(key, v) },
                // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
                None => unsafe { env::remove_var(key) },
            }
        }
    }

    #[test]
    fn review_pending_cycle_max_canonical_and_legacy() {
        let _g = process_env_lock();
        let prev_canon = env::var_os(ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV);
        let prev_legacy = env::var_os(ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV);
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV) };
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV) };
        assert_eq!(router_rs_review_pending_cycle_max(), 32);
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV, "64") };
        assert_eq!(router_rs_review_pending_cycle_max(), 64);
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV, "48") };
        assert_eq!(router_rs_review_pending_cycle_max(), 48);
        match prev_canon {
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            Some(v) => unsafe { env::set_var(ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV, v) },
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            None => unsafe { env::remove_var(ROUTER_RS_REVIEW_PENDING_CYCLE_MAX_ENV) },
        }
        match prev_legacy {
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            Some(v) => unsafe { env::set_var(ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX_ENV, v) },
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
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
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            unsafe { env::remove_var(key) };
        }
        assert!(router_rs_review_gate_stop_max_nudges_cap().is_none());
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV, "3") };
        assert_eq!(router_rs_review_gate_stop_max_nudges_cap(), Some(3));
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::remove_var(ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES_ENV) };
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { env::set_var(ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES_ENV, "5") };
        assert_eq!(router_rs_review_gate_stop_max_nudges_cap(), Some(5));
        for (key, val) in prev {
            match val {
                // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
                Some(v) => unsafe { env::set_var(key, v) },
                // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
                None => unsafe { env::remove_var(key) },
            }
        }
    }
}
