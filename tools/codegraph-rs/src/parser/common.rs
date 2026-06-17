//! Language detection helpers for future tree-sitter parsers (W1/W3).

/// Encode bytes as hex string using a lookup table (zero intermediate allocations).
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX_TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX_TABLE[(b >> 4) as usize] as char);
        out.push(HEX_TABLE[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn detect_language(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".rs") {
        return Some("rust");
    }
    if lower.ends_with(".ts") || lower.ends_with(".tsx") {
        return Some("typescript");
    }
    if lower.ends_with(".py") {
        return Some("python");
    }
    if lower.ends_with(".go") {
        return Some("go");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_and_typescript() {
        assert_eq!(detect_language("src/lib.rs"), Some("rust"));
        assert_eq!(detect_language("app.tsx"), Some("typescript"));
        assert_eq!(detect_language("README"), None);
    }

    #[test]
    fn hex_encode_empty() {
        assert_eq!(hex_encode(b""), "");
    }

    #[test]
    fn hex_encode_known_values() {
        assert_eq!(hex_encode(b"\x00\xff"), "00ff");
        assert_eq!(hex_encode(b"Hello"), "48656c6c6f");
    }
}
