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
    if lower.ends_with(".md") && should_index_markdown(path) {
        return Some("markdown");
    }
    None
}

/// Only index .md files under docs/ or skills/ — skip README.md, CHANGELOG.md, LICENSE.md etc.
fn should_index_markdown(path: &str) -> bool {
    let normal = path.replace('\\', "/");
    let in_scope = normal.contains("/docs/")
        || normal.contains("/skills/")
        || normal.starts_with("docs/")
        || normal.starts_with("skills/");
    if !in_scope {
        return false;
    }
    // Exclude files whose headings are noise (not code-relevant)
    let fname = normal.rsplit('/').next().unwrap_or(&normal);
    !matches!(
        fname.to_ascii_lowercase().as_str(),
        "readme.md" | "changelog.md" | "license.md" | "contributing.md" | "index.md"
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
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

    #[test]
    fn should_index_markdown_in_docs() {
        assert_eq!(detect_language("docs/architecture.md"), Some("markdown"));
        assert_eq!(detect_language("docs/guide.md"), Some("markdown"));
        assert_eq!(
            detect_language("skills/code-review-deep/SKILL.md"),
            Some("markdown")
        );
    }

    #[test]
    fn should_not_index_markdown_outside_docs_skills() {
        assert_eq!(detect_language("README.md"), None);
        assert_eq!(detect_language("CHANGELOG.md"), None);
        assert_eq!(detect_language(".claude/CLAUDE.md"), None);
        assert_eq!(detect_language("node_modules/pkg/README.md"), None);
    }

    #[test]
    fn should_skip_noise_markdown_files() {
        // README.md, CHANGELOG.md under docs/ are excluded
        assert_eq!(detect_language("docs/README.md"), None);
        assert_eq!(detect_language("docs/CHANGELOG.md"), None);
        assert_eq!(detect_language("skills/primary-runtime/README.md"), None);
    }

    #[test]
    fn should_index_subdir_skill_markdown() {
        assert_eq!(
            detect_language("skills/paper-workbench/SKILL.md"),
            Some("markdown")
        );
    }
}
