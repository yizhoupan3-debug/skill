//! Optional per-hook timing lines on stderr (`ROUTER_RS_HOOK_TIMING=1`).

use std::cell::Cell;
use std::time::Instant;

use crate::router_rs_hook_timing_enabled;

thread_local! {
    static HOOK_STARTED: Cell<Option<Instant>> = const { Cell::new(None) };
    static LOCK_WAIT_MS: Cell<u64> = const { Cell::new(0) };
    static CARGO_CHECK_MS: Cell<u64> = const { Cell::new(0) };
}

pub fn mark_hook_start() {
    if !router_rs_hook_timing_enabled() {
        return;
    }
    HOOK_STARTED.with(|c| c.set(Some(Instant::now())));
}

pub fn emit_hook_timing_line(event: &str) {
    if !router_rs_hook_timing_enabled() {
        return;
    }
    let duration_ms = HOOK_STARTED
        .with(|c| c.get().map(|t| t.elapsed().as_millis() as u64))
        .unwrap_or(0);
    let lock_wait_ms = LOCK_WAIT_MS.with(|c| c.get());
    let cargo_check_ms = CARGO_CHECK_MS.with(|c| c.get());
    tracing::debug!(
        event,
        duration_ms,
        lock_wait_ms,
        cargo_check_ms,
        "hook timing"
    );
    HOOK_STARTED.with(|c| c.set(None));
    LOCK_WAIT_MS.with(|c| c.set(0));
    CARGO_CHECK_MS.with(|c| c.set(0));
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::router_rs_hook_timing_enabled;
    use crate::router_self::resolve_router_rs_test_bin;
    use crate::test_env_sync::process_env_lock;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    #[test]
    fn hook_timing_env_enabled_and_emits_on_stderr() {
        let _g = process_env_lock();
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_HOOK_TIMING", "1") };
        assert!(router_rs_hook_timing_enabled());

        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let repo = repo.canonicalize().expect("repo root");
        let bin = resolve_router_rs_test_bin();

        let out = Command::new(bin)
            .args([
                "host",
                "hook",
                "cursor",
                "--event=beforeSubmitPrompt",
                &format!("--repo-root={}", repo.display()),
            ])
            .env("ROUTER_RS_HOOK_TIMING", "1")
            .env("ROUTER_RS_REVIEW_GATE_DISABLE", "1")
            .env("RUST_LOG", "runtime_core=debug")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn")
            .wait_with_output()
            .expect("wait");
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            (stderr.contains("hook timing") || stdout.contains("hook timing"))
                && (stderr.contains("duration_ms") || stdout.contains("duration_ms")),
            "expected hook timing on stderr or stdout, got stderr: {stderr:?} stdout: {stdout:?}"
        );
    }
}
