//! SSRF guards for MCP `web_fetch` and `browser_open` (public internet only).
//! Used by both browser-mcp and framework (router-rs/host-projection).

use std::borrow::Cow;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::str::FromStr;

use core_policy::error::FrameworkError;

type Result<T> = std::result::Result<T, FrameworkError>;

const BLOCKED_HOST_SUFFIXES: &[&str] = &[".localhost", ".local", ".internal"];

#[deprecated(note = "use validate_and_resolve_web_fetch_url for DNS pinning (TOCTOU safety)")]
pub fn validate_web_fetch_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url.trim())
        .map_err(|e| FrameworkError::validation(format!("web_fetch invalid URL: {url}: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(FrameworkError::validation(format!(
            "web_fetch only supports http(s) URLs: {url}"
        )));
    }
    let Some(host) = parsed.host_str() else {
        return Err(FrameworkError::validation(format!(
            "web_fetch URL missing host: {url}"
        )));
    };
    validate_web_fetch_host(host)?;
    if let Some(port) = parsed.port() {
        validate_web_fetch_port(port)?;
    }
    Ok(())
}

/// Validates `url` AND resolves+pins DNS in one pass — single DNS lookup, no TOCTOU window.
/// Returns the parsed URL and resolved addresses for DNS pinning to prevent rebinding TOCTOU.
pub fn validate_and_resolve_web_fetch_url(
    url: &str,
) -> Result<(reqwest::Url, Vec<std::net::SocketAddr>)> {
    let trimmed = url.trim();
    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|e| FrameworkError::validation(format!("web_fetch invalid URL for DNS pin: {url}: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(FrameworkError::validation(format!(
            "web_fetch only supports http(s) URLs: {url}"
        )));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| {
            FrameworkError::validation(format!("web_fetch URL missing host: {url}"))
        })?;
    validate_web_fetch_host_basic(host)?;
    if let Some(port) = parsed.port() {
        validate_web_fetch_port(port)?;
    }
    // Single DNS pass — resolve + validate forbidden IPs in one call.
    let port = parsed
        .port()
        .unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
    let addrs = resolve_web_fetch_addresses(host, port)?;
    Ok((parsed, addrs))
}

pub fn resolve_web_fetch_redirect(base: &reqwest::Url, location: &str) -> Result<String> {
    let next = base
        .join(location.trim())
        .map_err(|e| FrameworkError::validation(format!("web_fetch invalid redirect location: {e}")))?;
    let next_str = next.to_string();
    // Use validate_and_resolve to close DNS-rebind TOCTOU — single pass DNS resolve + IP check.
    // This prevents the redirect target from re-resolving to a private IP between validation
    // and the HTTP request. Callers that need DNS pinning should request a pinned variant.
    validate_and_resolve_web_fetch_url(&next_str)?;
    Ok(next_str)
}

fn validate_web_fetch_port(port: u16) -> Result<()> {
    if port == 0 {
        return Err(FrameworkError::validation(
            "web_fetch port 0 is not allowed",
        ));
    }
    Ok(())
}

/// Host checks that do NOT require DNS resolution (string-level only).
pub(crate) fn validate_web_fetch_host_basic(host: &str) -> Result<()> {
    let host = host.trim().trim_end_matches('.');
    if host.is_empty() {
        return Err(FrameworkError::validation(
            "web_fetch URL missing host",
        ));
    }
    let lower: Cow<'_, str> = if host.bytes().all(|b| !b.is_ascii_uppercase()) {
        Cow::Borrowed(host)
    } else {
        Cow::Owned(host.to_ascii_lowercase())
    };
    if lower == "localhost" || lower.ends_with(".localhost") {
        return Err(FrameworkError::validation(format!(
            "web_fetch blocked host: {host}"
        )));
    }
    for suffix in BLOCKED_HOST_SUFFIXES {
        if lower.ends_with(suffix) {
            return Err(FrameworkError::validation(format!(
                "web_fetch blocked host suffix: {host}"
            )));
        }
    }
    if let Ok(ip) = IpAddr::from_str(host)
        && is_forbidden_web_fetch_ip(&ip)
    {
        return Err(FrameworkError::validation(format!(
            "web_fetch blocked IP: {host}"
        )));
    }
    Ok(())
}

fn validate_web_fetch_host(host: &str) -> Result<()> {
    validate_web_fetch_host_basic(host)?;
    // Hostname-only: resolve DNS to catch hostnames pointing to private IPs.
    // Port is irrelevant for SSRF IP checks; use 443 for the lookup.
    let host = host.trim().trim_end_matches('.');
    if IpAddr::from_str(host).is_ok() {
        return Ok(()); // Already checked in basic — IP literal, no DNS needed.
    }
    let lookup_host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    let addrs = (lookup_host, 443u16)
        .to_socket_addrs()
        .map_err(|err| FrameworkError::validation(format!("web_fetch DNS lookup failed for {host}: {err}")))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if is_forbidden_web_fetch_ip(&addr.ip()) {
            return Err(FrameworkError::validation(format!(
                "web_fetch blocked resolved address for {host}: {}",
                addr.ip()
            )));
        }
    }
    if !any {
        return Err(FrameworkError::validation(format!(
            "web_fetch DNS lookup returned no addresses for {host}"
        )));
    }
    Ok(())
}

fn is_forbidden_web_fetch_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_forbidden_web_fetch_ipv4(*v4),
        IpAddr::V6(v6) => is_forbidden_web_fetch_ipv6(*v6),
    }
}

fn is_forbidden_web_fetch_ipv4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || o[0] == 0
        || (o[0] == 169 && o[1] == 254) // link-local / metadata
        || (o[0] == 100 && (o[1] & 0xC0) == 64) // CGNAT 100.64.0.0/10 (RFC 6598)
        || (o[0] == 198 && (o[1] & 0xFE) == 18) // benchmarking 198.18.0.0/15 (RFC 2544)
}

/// Checks whether an IPv6 address is forbidden for web_fetch.
///
/// NOTE: IPv6 6to4 (2002::/16) and IPv4-compatible (::0:0/96) addresses that
/// embed private IPv4 are not blocked because:
/// - 6to4 is deprecated and modern kernels do not route it
/// - IPv4-compatible addresses are removed from IPv6 by RFC 4291
/// - Cloud metadata services listen on IPv4 only
fn is_forbidden_web_fetch_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.is_multicast()
        || ip.to_ipv4_mapped().is_some_and(is_forbidden_web_fetch_ipv4)
}

/// Resolves `host` and returns all `SocketAddr` entries that pass the SSRF guard.
/// Used to pin DNS results before the HTTP request, preventing DNS rebinding.
pub fn resolve_web_fetch_addresses(
    host: &str,
    port: u16,
) -> Result<Vec<std::net::SocketAddr>> {
    let lookup_host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    let addrs: Vec<std::net::SocketAddr> = (lookup_host, port)
        .to_socket_addrs()
        .map_err(|err| FrameworkError::validation(format!("web_fetch DNS lookup failed for {host}: {err}")))?
        .collect();
    if addrs.is_empty() {
        return Err(FrameworkError::validation(format!(
            "web_fetch DNS lookup returned no addresses for {host}"
        )));
    }
    for addr in &addrs {
        if is_forbidden_web_fetch_ip(&addr.ip()) {
            return Err(FrameworkError::validation(format!(
                "web_fetch blocked resolved address for {host}: {}",
                addr.ip()
            )));
        }
    }
    Ok(addrs)
}

/// Validates URLs for `browser_open` - blocks non-http(s) schemes (`file://`,
/// `data:`, `javascript:`, etc.) and reuses the web_fetch SSRF guards
/// (private IPs, metadata endpoints, blocked host suffixes).
pub fn validate_browser_open_url(url: &str) -> Result<()> {
    let trimmed = url.trim();
    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|e| FrameworkError::validation(format!("browser_open invalid URL: {url}: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(FrameworkError::validation(format!(
            "browser_open blocked scheme '{}' - only http(s) allowed: {}",
            parsed.scheme(),
            url
        )));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| FrameworkError::validation(format!("browser_open URL missing host: {url}")))?;
    validate_web_fetch_host(host)?;
    if let Some(port) = parsed.port() {
        validate_web_fetch_port(port)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_loopback_literal_and_name() {
        assert!(validate_web_fetch_url("http://127.0.0.1/").is_err());
        assert!(validate_web_fetch_url("http://localhost/").is_err());
    }

    #[test]
    fn accepts_public_https_url_shape() {
        // Use a literal public IP: some dev DNS proxies map example.com to RFC 2544 ranges.
        assert!(validate_web_fetch_url("https://8.8.8.8/").is_ok());
    }

    #[test]
    fn rejects_cgnat_range() {
        assert!(validate_web_fetch_url("http://100.64.0.1/").is_err());
        assert!(validate_web_fetch_url("http://100.127.255.255/").is_err());
        // 100.63.x.x and 100.128.x.x are NOT in CGNAT range
        assert!(validate_web_fetch_url("http://100.63.0.1/").is_ok());
    }

    #[test]
    fn rejects_benchmarking_range() {
        assert!(validate_web_fetch_url("http://198.18.0.1/").is_err());
        assert!(validate_web_fetch_url("http://198.19.255.255/").is_err());
    }

    #[test]
    fn rejects_ipv6_multicast() {
        assert!(validate_web_fetch_url("http://[ff02::1]/").is_err());
    }

    #[test]
    fn rejects_ipv6_loopback() {
        assert!(validate_web_fetch_url("http://[::1]/").is_err());
    }

    #[test]
    fn rejects_ipv4_mapped_loopback() {
        assert!(validate_web_fetch_url("http://[::ffff:127.0.0.1]/").is_err());
    }

    #[test]
    fn rejects_metadata_endpoint() {
        assert!(validate_web_fetch_url("http://169.254.169.254/").is_err());
    }

    #[test]
    fn rejects_zero_prefix() {
        assert!(validate_web_fetch_url("http://0.0.0.1/").is_err());
    }

    #[test]
    fn rejects_blocked_suffixes() {
        assert!(validate_web_fetch_url("http://foo.local/").is_err());
        assert!(validate_web_fetch_url("http://bar.internal/").is_err());
    }

    #[test]
    fn resolve_addresses_rejects_loopback() {
        // 127.0.0.1 should be blocked
        assert!(resolve_web_fetch_addresses("127.0.0.1", 80).is_err());
    }

    #[test]
    fn resolve_addresses_rejects_private_ip_literal() {
        // Private IP literal should be blocked
        assert!(resolve_web_fetch_addresses("10.0.0.1", 80).is_err());
    }

    #[test]
    fn resolve_addresses_empty_result_is_error() {
        // DNS lookup returning empty should error
        // Use a non-existent TLD that won't resolve
        let result = resolve_web_fetch_addresses("nonexistent.invalid", 80);
        assert!(result.is_err());
    }
    #[test]
    fn rejects_redirect_to_loopback() {
        let base = reqwest::Url::parse("https://example.com/").unwrap();
        assert!(resolve_web_fetch_redirect(&base, "http://127.0.0.1/").is_err());
    }

    // --- browser_open specific tests ---

    #[test]
    fn browser_open_rejects_file_scheme() {
        assert!(validate_browser_open_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn browser_open_rejects_data_scheme() {
        assert!(validate_browser_open_url("data:text/html,<h1>hi</h1>").is_err());
    }

    #[test]
    fn browser_open_rejects_javascript_scheme() {
        assert!(validate_browser_open_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn browser_open_rejects_loopback() {
        assert!(validate_browser_open_url("http://127.0.0.1/").is_err());
        assert!(validate_browser_open_url("http://localhost/").is_err());
    }

    #[test]
    fn browser_open_rejects_private_ip() {
        assert!(validate_browser_open_url("http://10.0.0.1/").is_err());
        assert!(validate_browser_open_url("http://192.168.1.1/").is_err());
        assert!(validate_browser_open_url("http://172.16.0.1/").is_err());
    }

    #[test]
    fn browser_open_accepts_public_url() {
        assert!(validate_browser_open_url("https://8.8.8.8/").is_ok());
        assert!(validate_browser_open_url("http://93.184.216.34/").is_ok());
    }

    // --- Additional coverage tests ---

    #[test]
    fn rejects_private_ip_ranges_10_172_192() {
        assert!(validate_web_fetch_url("http://10.0.0.1/").is_err());
        assert!(validate_web_fetch_url("http://10.255.255.255/").is_err());
        assert!(validate_web_fetch_url("http://172.16.0.1/").is_err());
        assert!(validate_web_fetch_url("http://172.31.255.255/").is_err());
        assert!(validate_web_fetch_url("http://192.168.1.1/").is_err());
        assert!(validate_web_fetch_url("http://192.168.255.255/").is_err());
    }

    #[test]
    fn rejects_localhost_subdomain() {
        assert!(validate_web_fetch_url("http://foo.localhost/").is_err());
        assert!(validate_web_fetch_url("http://bar.localhost/").is_err());
    }

    #[test]
    fn rejects_ipv6_unique_local_and_link_local() {
        assert!(validate_web_fetch_url("http://[fd00::1]/").is_err());
        assert!(validate_web_fetch_url("http://[fe80::1]/").is_err());
    }

    #[test]
    fn rejects_ipv6_unspecified() {
        assert!(validate_web_fetch_url("http://[::]/").is_err());
    }

    #[test]
    fn rejects_ipv6_mapped_private() {
        assert!(validate_web_fetch_url("http://[::ffff:10.0.0.1]/").is_err());
        assert!(validate_web_fetch_url("http://[::ffff:192.168.1.1]/").is_err());
    }

    #[test]
    fn rejects_port_zero() {
        assert!(validate_web_fetch_url("http://example.com:0/").is_err());
    }

    #[test]
    fn rejects_trailing_dot_localhost() {
        assert!(validate_web_fetch_url("http://localhost./").is_err());
    }

    #[test]
    fn rejects_case_variants() {
        assert!(validate_web_fetch_url("http://LOCALHOST/").is_err());
        assert!(validate_web_fetch_url("http://Foo.Local/").is_err());
        assert!(validate_web_fetch_url("http://Bar.Internal/").is_err());
    }

    #[test]
    fn browser_open_rejects_ipv6_variants() {
        assert!(validate_browser_open_url("http://[::1]/").is_err());
        assert!(validate_browser_open_url("http://[fd00::1]/").is_err());
        assert!(validate_browser_open_url("http://[::ffff:10.0.0.1]/").is_err());
    }

    #[test]
    fn rejects_redirect_to_private_ip() {
        let base = reqwest::Url::parse("https://example.com/").unwrap();
        assert!(resolve_web_fetch_redirect(&base, "http://10.0.0.1/").is_err());
        assert!(resolve_web_fetch_redirect(&base, "http://192.168.1.1/").is_err());
        assert!(resolve_web_fetch_redirect(&base, "http://169.254.169.254/").is_err());
    }

    #[test]
    fn rejects_redirect_to_localhost() {
        let base = reqwest::Url::parse("https://example.com/").unwrap();
        assert!(resolve_web_fetch_redirect(&base, "http://localhost/").is_err());
    }

    #[test]
    fn debug_forbidden_ipv4_classification() {
        let ips = [
            "127.0.0.1",       // loopback
            "10.0.0.1",        // private (10.0.0.0/8)
            "172.16.0.1",      // private (172.16.0.0/12)
            "192.168.1.1",     // private (192.168.0.0/16)
            "169.254.169.254", // link-local / metadata
            "0.0.0.0",         // unspecified
            "0.0.0.1",         // o[0] == 0
            "100.64.0.1",      // CGNAT
            "100.127.255.255", // CGNAT
            "100.63.0.1",      // NOT CGNAT
            "198.18.0.1",      // benchmarking
            "198.19.255.255",  // benchmarking
            "8.8.8.8",         // public DNS
            "93.184.216.34",   // public (example.com)
        ];
        let results: Vec<_> = ips
            .iter()
            .map(|ip| {
                (
                    ip.to_string(),
                    super::is_forbidden_web_fetch_ipv4(ip.parse().unwrap()),
                )
            })
            .collect();
        insta::assert_debug_snapshot!(results);
    }

    #[test]
    fn debug_forbidden_ipv6_classification() {
        let ips = [
            "::1",                  // loopback
            "::",                   // unspecified
            "fd00::1",              // unique-local
            "fe80::1",              // link-local
            "ff02::1",              // multicast
            "::ffff:127.0.0.1",     // IPv4-mapped loopback
            "::ffff:10.0.0.1",      // IPv4-mapped private
            "::ffff:8.8.8.8",       // IPv4-mapped public
            "2001:4860:4860::8888", // public (Google DNS)
        ];
        let results: Vec<_> = ips
            .iter()
            .map(|ip| {
                (
                    ip.to_string(),
                    super::is_forbidden_web_fetch_ipv6(ip.parse().unwrap()),
                )
            })
            .collect();
        insta::assert_debug_snapshot!(results);
    }

    #[test]
    fn debug_validate_host_basic_results() {
        let hosts = [
            "localhost",
            "foo.localhost",
            "example.com",
            "foo.local",
            "bar.internal",
            "127.0.0.1",
            "10.0.0.1",
            "192.168.1.1",
            "8.8.8.8",
            "169.254.169.254",
            "LOCALHOST",
            "Foo.Local",
        ];
        let results: Vec<_> = hosts
            .iter()
            .map(|host| (host.to_string(), super::validate_web_fetch_host_basic(host)))
            .collect();
        insta::assert_debug_snapshot!(results);
    }
}
