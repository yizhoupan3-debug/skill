//! Language detection helpers for future tree-sitter parsers (W1/W3).

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
    use super::detect_language;

    #[test]
    fn detects_rust_and_typescript() {
        assert_eq!(detect_language("src/lib.rs"), Some("rust"));
        assert_eq!(detect_language("app.tsx"), Some("typescript"));
        assert_eq!(detect_language("README"), None);
    }
}
