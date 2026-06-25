//! Unsafe wrappers for `std::env::set_var` / `std::env::remove_var`.
//!
//! # Safety
//! These functions must only be called in single-threaded contexts (e.g. tests)
//! or before the runtime starts. In multi-threaded contexts, `set_var` / `remove_var`
//! is undefined behavior per the Rust reference.

/// Set an environment variable. Safe wrapper over `std::env::set_var`.
///
/// # Safety
/// Must not be called while other threads are reading/writing env vars.
pub unsafe fn set_env(key: impl AsRef<std::ffi::OsStr>, value: impl AsRef<std::ffi::OsStr>) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe {
        std::env::set_var(key, value);
    }
}

/// Remove an environment variable. Safe wrapper over `std::env::remove_var`.
///
/// # Safety
/// Must not be called while other threads are reading/writing env vars.
pub unsafe fn remove_env(key: impl AsRef<std::ffi::OsStr>) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe {
        std::env::remove_var(key);
    }
}
