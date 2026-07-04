//! Shared subprocess resource limits (pre_exec safe).

/// Apply process resource limits via setrlimit in the forked child (pre_exec).
/// Prevents runaway subprocesses from exhausting system resources.
///
/// Some limits (e.g. RLIMIT_AS on macOS) may not be honored by the platform.
/// In those cases, the failure is logged and skipped rather than aborting the
/// child process — a best-effort defense is better than no defense at all.
///
/// SAFETY: setrlimit is async-signal-safe; pre_exec runs in a single-threaded forked child.
#[cfg(unix)]
pub fn apply_subprocess_rlimits() -> Result<(), std::io::Error> {
    use libc::{
        RLIMIT_AS, RLIMIT_CPU, RLIMIT_FSIZE, RLIMIT_NOFILE, RLIMIT_NPROC, rlimit, setrlimit,
    };

    fn try_setrlimit(resource: i32, rlim: &rlimit, name: &str) {
        // SAFETY: setrlimit is async-signal-safe; called from pre_exec in a
        // single-threaded forked child. The rlim pointer is stack-allocated.
        if unsafe { setrlimit(resource, rlim) } != 0 {
            let err = std::io::Error::last_os_error();
            // eprintln is async-signal-safe and useful for pre_exec diagnostics.
            eprintln!("WARNING: setrlimit({name}) failed: {err} — skipping");
        }
    }

    // RLIMIT_CPU: 600s soft, 1200s hard
    try_setrlimit(
        RLIMIT_CPU,
        &rlimit {
            rlim_cur: 600,
            rlim_max: 1200,
        },
        "RLIMIT_CPU",
    );
    // RLIMIT_AS: 2 GiB soft, 4 GiB hard (may fail on macOS — skip gracefully)
    try_setrlimit(
        RLIMIT_AS,
        &rlimit {
            rlim_cur: 2 * 1024 * 1024 * 1024,
            rlim_max: 4 * 1024 * 1024 * 1024,
        },
        "RLIMIT_AS",
    );
    // RLIMIT_FSIZE: 100 MiB soft, 1 GiB hard
    try_setrlimit(
        RLIMIT_FSIZE,
        &rlimit {
            rlim_cur: 100 * 1024 * 1024,
            rlim_max: 1024 * 1024 * 1024,
        },
        "RLIMIT_FSIZE",
    );
    // RLIMIT_NOFILE: 256 soft, 1024 hard
    try_setrlimit(
        RLIMIT_NOFILE,
        &rlimit {
            rlim_cur: 256,
            rlim_max: 1024,
        },
        "RLIMIT_NOFILE",
    );
    // RLIMIT_NPROC: 64 soft, 256 hard
    try_setrlimit(
        RLIMIT_NPROC,
        &rlimit {
            rlim_cur: 64,
            rlim_max: 256,
        },
        "RLIMIT_NPROC",
    );
    Ok(())
}

#[cfg(not(unix))]
pub fn apply_subprocess_rlimits() -> Result<(), std::io::Error> {
    Ok(())
}
