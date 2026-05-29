//! SSRF guards for Claude Desktop MCP `web_fetch` (public internet only).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::str::FromStr;

const BLOCKED_HOST_SUFFIXES: &[&str] = &[".localhost", ".local", ".internal"];

pub(crate) fn validate_web_fetch_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url.trim())
        .map_err(|_| format!("web_fetch invalid URL: {url}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("web_fetch only supports http(s) URLs: {url}"));
    }
    let Some(host) = parsed.host_str() else {
        return Err(format!("web_fetch URL missing host: {url}"));
    };
    validate_web_fetch_host(host)?;
    if let Some(port) = parsed.port() {
        validate_web_fetch_port(port)?;
    }
    Ok(())
}

pub(crate) fn resolve_web_fetch_redirect(
    base: &reqwest::Url,
    location: &str,
) -> Result<String, String> {
    let next = base
        .join(location.trim())
        .map_err(|err| format!("web_fetch invalid redirect location: {err}"))?;
    let next_str = next.to_string();
    validate_web_fetch_url(&next_str)?;
    Ok(next_str)
}

fn validate_web_fetch_port(port: u16) -> Result<(), String> {
    if port == 0 {
        return Err("web_fetch port 0 is not allowed".to_string());
    }
    Ok(())
}

fn validate_web_fetch_host(host: &str) -> Result<(), String> {
    let host = host.trim().trim_end_matches('.');
    if host.is_empty() {
        return Err("web_fetch URL missing host".to_string());
    }
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return Err(format!("web_fetch blocked host: {host}"));
    }
    for suffix in BLOCKED_HOST_SUFFIXES {
        if lower.ends_with(suffix) {
            return Err(format!("web_fetch blocked host suffix: {host}"));
        }
    }
    if let Ok(ip) = IpAddr::from_str(host) {
        if is_forbidden_web_fetch_ip(&ip) {
            return Err(format!("web_fetch blocked IP: {host}"));
        }
        return Ok(());
    }
    let port = if lower.contains(':') && !lower.starts_with('[') {
        host.rsplit_once(':')
            .and_then(|(_, p)| p.parse::<u16>().ok())
            .unwrap_or(443)
    } else {
        443
    };
    let lookup_host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    let addrs = (lookup_host, port)
        .to_socket_addrs()
        .map_err(|err| format!("web_fetch DNS lookup failed for {host}: {err}"))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if is_forbidden_web_fetch_ip(&addr.ip()) {
            return Err(format!(
                "web_fetch blocked resolved address for {host}: {}",
                addr.ip()
            ));
        }
    }
    if !any {
        return Err(format!("web_fetch DNS lookup returned no addresses for {host}"));
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
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.octets()[0] == 0
        || ip == Ipv4Addr::new(169, 254, 0, 0) // metadata range start heuristic
        || (ip.octets()[0] == 169 && ip.octets()[1] == 254)
}

fn is_forbidden_web_fetch_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.to_ipv4_mapped().is_some_and(is_forbidden_web_fetch_ipv4)
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
        assert!(validate_web_fetch_url("https://example.com/").is_ok());
    }
}
