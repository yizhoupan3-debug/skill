//! Optional per-hook timing lines on stderr (`ROUTER_RS_HOOK_TIMING=1`).

use std::cell::Cell;
use std::time::Instant;

thread_local! {
    static HOOK_STARTED: Cell<Option<Instant>> = const { Cell::new(None) };
    static LOCK_WAIT_MS: Cell<u64> = const { Cell::new(0) };
    static CARGO_CHECK_MS: Cell<u64> = const { Cell::new(0) };
}

pub fn mark_hook_start() {
    if !crate::router_env_flags::router_rs_hook_timing_enabled() {
        return;
    }
    HOOK_STARTED.with(|c| c.set(Some(Instant::now())));
}

pub fn add_lock_wait_ms(ms: u64) {
    if ms == 0 || !crate::router_env_flags::router_rs_hook_timing_enabled() {
        return;
    }
    LOCK_WAIT_MS.with(|c| c.set(c.get().saturating_add(ms)));
}

pub fn add_cargo_check_ms(ms: u64) {
    if ms == 0 || !crate::router_env_flags::router_rs_hook_timing_enabled() {
        return;
    }
    CARGO_CHECK_MS.with(|c| c.set(c.get().saturating_add(ms)));
}

pub fn emit_hook_timing_line(event: &str) {
    if !crate::router_env_flags::router_rs_hook_timing_enabled() {
        return;
    }
    let duration_ms = HOOK_STARTED
        .with(|c| c.get().map(|t| t.elapsed().as_millis() as u64))
        .unwrap_or(0);
    let lock_wait_ms = LOCK_WAIT_MS.with(|c| c.get());
    let cargo_check_ms = CARGO_CHECK_MS.with(|c| c.get());
    eprintln!(
        "hook_timing event={event} duration_ms={duration_ms} lock_wait_ms={lock_wait_ms} cargo_check_ms={cargo_check_ms}"
    );
    let _ = std::io::Write::flush(&mut std::io::stderr());
    crate::telemetry_emit::emit_hook_timing_telemetry(
        event,
        duration_ms,
        lock_wait_ms,
        cargo_check_ms,
    );
    HOOK_STARTED.with(|c| c.set(None));
    LOCK_WAIT_MS.with(|c| c.set(0));
    CARGO_CHECK_MS.with(|c| c.set(0));
}

#[cfg(test)]
mod tests {
    use crate::router_self::resolve_router_rs_test_bin;
    use crate::test_env_sync::process_env_lock;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    #[test]
    fn hook_timing_env_enabled_and_emits_on_stderr() {
        let _g = process_env_lock();
        std::env::set_var("ROUTER_RS_HOOK_TIMING", "1");
        assert!(crate::router_env_flags::router_rs_hook_timing_enabled());

        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let repo = repo.canonicalize().expect("repo root");
        let bin = resolve_router_rs_test_bin();

        // Skip if the binary is a redirect shim (post-migration stub).
        let probe = Command::new(&bin)
            .arg("--help")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        if let Ok(out) = probe {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            if combined.contains("moved") || combined.contains("router-rs-cli") {
                eprintln!("skip: router-rs binary is a redirect shim; hook timing e2e test requires the real binary");
                return;
            }
        }

        let out = Command::new(bin)
            .args([
                "host",
                "cursor",
                "hook",
                "--event=beforeSubmitPrompt",
                &format!("--repo-root={}", repo.display()),
            ])
            .env("ROUTER_RS_HOOK_TIMING", "1")
            .env("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn")
            .wait_with_output()
            .expect("wait");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("hook_timing") && stderr.contains("duration_ms"),
            "expected hook_timing on stderr, got: {stderr:?}"
        );
    }
}
