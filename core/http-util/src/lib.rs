//! HTTP utilities shared across framework crates.
//! Provides proxy URL resolution from environment variables.

/// Returns the first non-empty proxy URL from environment variables,
/// checking: `HTTPS_PROXY`, `https_proxy`, `HTTP_PROXY`, `http_proxy`,
/// `ALL_PROXY` (checked in order). Reads env vars each call — no caching,
/// so env var changes between sessions are correctly reflected.
pub fn cached_proxy_url() -> Option<String> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// cached_proxy_url returns Some(trimmed) when a proxy env var is set.
    #[test]
    fn cached_proxy_url_returns_some_when_env_set() {
        unsafe { core_state_utils::env_sync::set_env("HTTPS_PROXY", "http://proxy.test:8080") };
        let result = cached_proxy_url();
        assert_eq!(result, Some("http://proxy.test:8080".to_string()));
        unsafe { core_state_utils::env_sync::remove_env("HTTPS_PROXY") };
    }

    /// cached_proxy_url returns None when no proxy env var is set.
    #[test]
    fn cached_proxy_url_returns_none_when_no_env() {
        unsafe {
            core_state_utils::env_sync::remove_env("HTTPS_PROXY");
            core_state_utils::env_sync::remove_env("https_proxy");
            core_state_utils::env_sync::remove_env("HTTP_PROXY");
            core_state_utils::env_sync::remove_env("http_proxy");
            core_state_utils::env_sync::remove_env("ALL_PROXY");
        }
        assert_eq!(cached_proxy_url(), None);
    }

    /// cached_proxy_url returns the first non-empty proxy in priority order.
    #[test]
    fn cached_proxy_url_prioritizes_https_proxy() {
        unsafe {
            core_state_utils::env_sync::set_env("HTTPS_PROXY", "http://https-proxy:8080");
            core_state_utils::env_sync::set_env("HTTP_PROXY", "http://http-proxy:8080");
        }
        let result = cached_proxy_url();
        assert_eq!(result, Some("http://https-proxy:8080".to_string()));
        unsafe {
            core_state_utils::env_sync::remove_env("HTTPS_PROXY");
            core_state_utils::env_sync::remove_env("HTTP_PROXY");
        }
    }

    /// cached_proxy_url trims whitespace from the proxy URL.
    #[test]
    fn cached_proxy_url_trims_whitespace() {
        unsafe { core_state_utils::env_sync::set_env("HTTPS_PROXY", "  http://proxy.test:8080  ") };
        let result = cached_proxy_url();
        assert_eq!(result, Some("http://proxy.test:8080".to_string()));
        unsafe { core_state_utils::env_sync::remove_env("HTTPS_PROXY") };
    }

    /// cached_proxy_url reads env vars fresh each call — no caching.
    #[test]
    fn cached_proxy_url_reads_fresh_each_call() {
        unsafe { core_state_utils::env_sync::set_env("HTTPS_PROXY", "http://first:8080") };
        let first = cached_proxy_url();
        unsafe { core_state_utils::env_sync::set_env("HTTPS_PROXY", "http://second:8080") };
        let second = cached_proxy_url();
        assert_eq!(first, Some("http://first:8080".to_string()));
        assert_eq!(second, Some("http://second:8080".to_string()));
        unsafe { core_state_utils::env_sync::remove_env("HTTPS_PROXY") };
    }
}
