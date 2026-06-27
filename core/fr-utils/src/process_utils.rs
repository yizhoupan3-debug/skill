//! Shared subprocess resource limits (pre_exec safe).

/// Apply process resource limits via setrlimit in the forked child (pre_exec).
/// Prevents runaway subprocesses from exhausting system resources.
///
/// SAFETY: setrlimit is async-signal-safe; pre_exec runs in a single-threaded forked child.
#[cfg(unix)]
pub fn apply_subprocess_rlimits() -> Result<(), std::io::Error> {
    use libc::{
        rlimit, setrlimit, RLIMIT_AS, RLIMIT_CPU, RLIMIT_FSIZE, RLIMIT_NOFILE, RLIMIT_NPROC,
    };
    // RLIMIT_CPU: 600s soft, 1200s hard
    let rlim_cpu = rlimit { rlim_cur: 600, rlim_max: 1200 };
    if unsafe { setrlimit(RLIMIT_CPU, &rlim_cpu) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // RLIMIT_AS: 2 GiB soft, 4 GiB hard
    let rlim_as = rlimit { rlim_cur: 2 * 1024 * 1024 * 1024, rlim_max: 4 * 1024 * 1024 * 1024 };
    if unsafe { setrlimit(RLIMIT_AS, &rlim_as) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // RLIMIT_FSIZE: 100 MiB soft, 1 GiB hard
    let rlim_fsize = rlimit { rlim_cur: 100 * 1024 * 1024, rlim_max: 1024 * 1024 * 1024 };
    if unsafe { setrlimit(RLIMIT_FSIZE, &rlim_fsize) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // RLIMIT_NOFILE: 256 soft, 1024 hard
    let rlim_nofile = rlimit { rlim_cur: 256, rlim_max: 1024 };
    if unsafe { setrlimit(RLIMIT_NOFILE, &rlim_nofile) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // RLIMIT_NPROC: 64 soft, 256 hard
    let rlim_nproc = rlimit { rlim_cur: 64, rlim_max: 256 };
    if unsafe { setrlimit(RLIMIT_NPROC, &rlim_nproc) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn apply_subprocess_rlimits() -> Result<(), std::io::Error> {
    Ok(())
}
