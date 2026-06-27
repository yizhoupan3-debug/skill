//! Prompt resolver — resolves content-store references into full text.

use core_errors::FrameworkError;

use crate::content_store::{ContentNotFound, ContentStore};

/// Resolves content-store references from a compression output.
pub struct PromptResolver {
    store: ContentStore,
}

impl PromptResolver {
    pub fn new(store: ContentStore) -> Self {
        Self { store }
    }

    /// Resolve a single offloaded ref by hash — used as the runtime infra
    /// counterpart to `framework_resolve_content` stdio dispatch.
    ///
    /// Sanitizes the input: strips `[ref:` / `ref:` prefix and `]` suffix,
    /// trims whitespace — the LLM may pass a `[ref:…]` placeholder verbatim.
    pub fn resolve_one(&self, raw_hash: &str) -> Result<String, FrameworkError> {
        let hash = sanitize_hash(raw_hash);
        match self.store.get(hash) {
            Ok(content) => Ok(content),
            Err(ContentNotFound(h)) => Err(FrameworkError::not_found(format!("content not found: {h}"))),
        }
    }
}

// ── Hash sanitization ─────────────────────────────────────────────────────

/// Sanitize a content hash from LLM input.
///
/// The LLM may pass a `[ref:…]` placeholder verbatim, include a trailing `]`,
/// or have extra whitespace.  Strips those so the store lookup succeeds.
fn sanitize_hash(raw: &str) -> &str {
    raw.trim()
        .strip_prefix("[ref:")
        .or_else(|| raw.trim().strip_prefix("ref:"))
        .unwrap_or(raw.trim())
        .trim_end_matches(']')
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_store::ContentStore;

    fn make_resolver() -> (PromptResolver, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ContentStore::new(dir.path());
        let resolver = PromptResolver::new(store);
        (resolver, dir)
    }

    #[test]
    fn resolve_one_roundtrip() {
        let (resolver, dir) = make_resolver();
        let store = ContentStore::new(dir.path());
        let hash = store.put("Full offloaded content here").expect("put");
        let got = resolver.resolve_one(&hash).expect("resolve_one");
        assert_eq!(got, "Full offloaded content here");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn resolve_missing_hash() {
        let (resolver, _dir) = make_resolver();
        let err = resolver.resolve_one("nonexistent");
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("content not found"));
    }

    #[test]
    fn sanitize_bare_hash() {
        assert_eq!(sanitize_hash("abc123"), "abc123");
    }

    #[test]
    fn sanitize_ref_brackets() {
        assert_eq!(sanitize_hash("[ref:abc123]"), "abc123");
    }

    #[test]
    fn sanitize_ref_no_bracket() {
        assert_eq!(sanitize_hash("[ref:abc123"), "abc123");
    }

    #[test]
    fn sanitize_trailing_bracket() {
        assert_eq!(sanitize_hash("abc123]"), "abc123");
    }

    #[test]
    fn sanitize_whitespace() {
        assert_eq!(sanitize_hash("  abc123  "), "abc123");
    }

    #[test]
    fn sanitize_ref_with_prefix() {
        assert_eq!(sanitize_hash("ref:abc123"), "abc123");
    }
}
