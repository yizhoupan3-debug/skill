//! HTTP utilities shared across framework crates.
//! Currently provides process-level cached proxy URL resolution.

use std::sync::OnceLock;

/// Returns the first non-empty proxy URL from environment variables,
/// cached at process level. Checks: `HTTPS_PROXY`, `https_proxy`,
/// `HTTP_PROXY`, `http_proxy`, `ALL_PROXY` (checked in order).
pub fn cached_proxy_url() -> Option<&'static str> {
    static PROXY: OnceLock<Option<String>> = OnceLock::new();
    PROXY
        .get_or_init(|| {
            for key in [
                "HTTPS_PROXY",
                "https_proxy",
                "HTTP_PROXY",
                "http_proxy",
                "ALL_PROXY",
            ] {
                if let Ok(url) = std::env::var(key) {
                    let trimmed = url.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
            None
        })
        .as_deref()
}
