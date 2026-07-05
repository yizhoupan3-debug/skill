use regex::Regex;

/// Result of validating compressed output against original.
#[derive(Debug)]
pub struct ValidationResult {
    pub code_blocks_match: bool,
    pub urls_preserved: bool,
    pub file_paths_preserved: bool,
    pub heading_structure_preserved: bool,
    pub inline_code_preserved: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.code_blocks_match
            && self.urls_preserved
            && self.file_paths_preserved
            && self.heading_structure_preserved
            && self.inline_code_preserved
            && self.errors.is_empty()
    }
}

/// Validate that compression preserved critical content.
pub fn validate_compression(original: &str, compressed: &str) -> ValidationResult {
    let mut result = ValidationResult {
        code_blocks_match: true,
        urls_preserved: true,
        file_paths_preserved: true,
        heading_structure_preserved: true,
        inline_code_preserved: true,
        errors: Vec::new(),
    };

    // V1: All fenced code blocks preserved exactly
    let fence_re = Regex::new(r"(?ms)^```[\s\S]*?^```").unwrap();
    let orig_fences: Vec<&str> = fence_re.find_iter(original).map(|m| m.as_str()).collect();
    let comp_fences: Vec<&str> = fence_re.find_iter(compressed).map(|m| m.as_str()).collect();

    if orig_fences.len() != comp_fences.len() {
        result.code_blocks_match = false;
        result.errors.push(format!(
            "Code block count mismatch: original={}, compressed={}",
            orig_fences.len(),
            comp_fences.len()
        ));
    } else {
        for (i, (o, c)) in orig_fences.iter().zip(comp_fences.iter()).enumerate() {
            if o != c {
                result.code_blocks_match = false;
                result.errors.push(format!("Code block #{} differs", i + 1));
                break;
            }
        }
    }

    // V2: All URLs preserved (strip trailing punctuation for comparison)
    let url_re = Regex::new(r#"https?://[^\s\)\]>"'`]+"#).unwrap();
    let strip_trail = Regex::new(r"[.,;:!?]+$").unwrap();
    let orig_urls: Vec<String> = url_re.find_iter(original)
        .map(|m| strip_trail.replace(m.as_str(), "").to_string())
        .collect();
    let comp_urls: Vec<String> = url_re.find_iter(compressed)
        .map(|m| strip_trail.replace(m.as_str(), "").to_string())
        .collect();
    if orig_urls != comp_urls {
        result.urls_preserved = false;
        result.errors.push("URLs changed after compression".to_string());
    }

    // V3: Inline code preserved
    let code_re = Regex::new(r"`[^`]+`").unwrap();
    let orig_code: Vec<&str> = code_re.find_iter(original).map(|m| m.as_str()).collect();
    let comp_code: Vec<&str> = code_re.find_iter(compressed).map(|m| m.as_str()).collect();
    if orig_code != comp_code {
        result.inline_code_preserved = false;
        result.errors.push("Inline code changed after compression".to_string());
    }

    // V4: Heading structure preserved
    let h_re = Regex::new(r"^(#{1,6})\s+.*$").unwrap();
    let orig_headings: Vec<&str> = original
        .lines()
        .filter(|l| h_re.is_match(l))
        .collect();
    let comp_headings: Vec<&str> = compressed
        .lines()
        .filter(|l| h_re.is_match(l))
        .collect();
    if orig_headings.len() != comp_headings.len() {
        result.heading_structure_preserved = false;
        result.errors.push("Heading count changed".to_string());
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_blocks_preserved() {
        let orig = "Before\n```\nfn main() {}\n```\nAfter";
        let comp = "B4\n```\nfn main() {}\n```\nAft";
        let result = validate_compression(orig, comp);
        assert!(result.code_blocks_match);
    }

    #[test]
    fn test_urls_preserved() {
        let orig = "See https://example.com/page for details.";
        let comp = "See https://example.com/page.";
        let result = validate_compression(orig, comp);
        assert!(result.urls_preserved);
    }

    #[test]
    fn test_urls_changed() {
        let orig = "See https://example.com/one for details.";
        let comp = "See https://example.com/two for details.";
        let result = validate_compression(orig, comp);
        assert!(!result.urls_preserved);
    }

    #[test]
    fn test_inline_code_preserved() {
        let orig = "Run `npm install`.";
        let comp = "Run `npm install`.";
        let result = validate_compression(orig, comp);
        assert!(result.inline_code_preserved);
    }

    #[test]
    fn test_heading_count() {
        let orig = "# Title\n\n## Section\nSome text.";
        let comp = "# Title\n\n## Section\nSome txt.";
        let result = validate_compression(orig, comp);
        assert!(result.heading_structure_preserved);
    }

    #[test]
    fn test_full_validation_with_compressed_prose() {
        let original = "Hello, this is a test file with some filler words like basically and really.\n\n```\nfn test() {}\n```";
        // This is what compress_text should produce (code block preserved, filler removed)
        let compressed = "Hello, this is a test file with some filler words like and.\n\n```\nfn test() {}\n```";
        let result = validate_compression(original, compressed);
        assert!(result.is_valid(), "Should validate: {:?}", result.errors);
    }
}
