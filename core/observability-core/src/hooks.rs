use std::panic;

/// Install a panic hook that logs panics via `tracing::error!` before
/// forwarding to the previous hook.
///
/// This ensures panics appear in the structured log output (both stderr
/// and rolling file) rather than only on stderr via the default hook.
///
/// Safe to call multiple times — only the first call takes effect.
pub fn install_panic_hook() {
    static HOOK_INSTALLED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    if HOOK_INSTALLED.set(()).is_err() {
        return;
    }

    let prev = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        tracing::error!(target: "panic", "{panic_info}");
        prev(panic_info);
    }));
}

#[cfg(test)]
mod tests {
    #[test]
    fn install_panic_hook_is_idempotent() {
        // Calling twice should not panic or deadlock.
        super::install_panic_hook();
        super::install_panic_hook();
    }
}
