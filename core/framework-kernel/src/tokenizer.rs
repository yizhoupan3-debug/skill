//! Dependency-inversion surface: B0 policy code tokenizes via injected B1 provider.
use std::sync::{OnceLock, RwLock};

static PROVIDER: OnceLock<RwLock<Box<dyn TokenizerProvider>>> = OnceLock::new();

/// Tokenization + parallel-review markers supplied by the routing engine (B1).
pub trait TokenizerProvider: Send + Sync {
    fn tokenize_query(&self, text: &str) -> Vec<String>;
    fn has_parallel_review_candidate_context(&self, query: &str, tokens: &[String]) -> bool;
}

struct PanicTokenizer;

impl TokenizerProvider for PanicTokenizer {
    fn tokenize_query(&self, text: &str) -> Vec<String> {
        panic!(
            "TokenizerProvider not installed; call install_tokenizer_provider before routing/policy tokenization (query_len={})",
            text.len()
        );
    }

    fn has_parallel_review_candidate_context(&self, _query: &str, _tokens: &[String]) -> bool {
        panic!("TokenizerProvider not installed; call install_tokenizer_provider first");
    }
}

fn provider_cell() -> &'static RwLock<Box<dyn TokenizerProvider>> {
    PROVIDER.get_or_init(|| RwLock::new(Box::new(PanicTokenizer)))
}

/// B1 calls during process startup (idempotent).
pub fn install_tokenizer_provider(provider: Box<dyn TokenizerProvider>) {
    set_tokenizer_provider(provider);
}

/// Replace provider (tests / late binding).
pub fn set_tokenizer_provider(provider: Box<dyn TokenizerProvider>) {
    if PROVIDER.get().is_some() {
        if let Ok(mut guard) = provider_cell().write() {
            *guard = provider;
        }
    } else {
        let _ = PROVIDER.set(RwLock::new(provider));
    }
}

pub fn tokenize_query(text: &str) -> Vec<String> {
    provider_cell()
        .read()
        .expect("tokenizer provider lock poisoned")
        .tokenize_query(text)
}

pub fn has_parallel_review_candidate_context(query: &str, tokens: &[String]) -> bool {
    provider_cell()
        .read()
        .expect("tokenizer provider lock poisoned")
        .has_parallel_review_candidate_context(query, tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub;

    impl TokenizerProvider for Stub {
        fn tokenize_query(&self, text: &str) -> Vec<String> {
            vec![text.to_ascii_lowercase()]
        }

        fn has_parallel_review_candidate_context(&self, _query: &str, tokens: &[String]) -> bool {
            tokens.iter().any(|t| t == "review")
        }
    }

    #[test]
    fn install_and_query_tokenizer() {
        set_tokenizer_provider(Box::new(Stub));
        let tokens = tokenize_query("Review");
        assert_eq!(tokens, vec!["review"]);
        assert!(has_parallel_review_candidate_context("review scope", &tokens));
    }
}
