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
    fn tokenize_query(&self, _text: &str) -> Vec<String> {
        panic!(
            "TokenizerProvider not installed — call install_tokenizer_provider() during kernel bootstrap"
        );
    }

    fn has_parallel_review_candidate_context(&self, _query: &str, _tokens: &[String]) -> bool {
        panic!(
            "TokenizerProvider not installed — call install_tokenizer_provider() during kernel bootstrap"
        );
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
        match provider_cell().write() {
            Ok(mut guard) => *guard = provider,
            Err(poisoned) => {
                tracing::warn!("[router-rs] tokenizer: recovering write-lock from poisoned state");
                *poisoned.into_inner() = provider;
            }
        }
    } else {
        let _ = PROVIDER.set(RwLock::new(provider));
    }
}

pub fn tokenize_query(text: &str) -> Vec<String> {
    match provider_cell().read() {
        Ok(guard) => guard.tokenize_query(text),
        Err(poisoned) => {
            // Recover from poisoned lock: extract the inner guard and continue.
            // A previous write-lock holder panicked, but the provider is still usable.
            tracing::warn!("[router-rs] tokenizer: recovering from poisoned RwLock");
            poisoned.into_inner().tokenize_query(text)
        }
    }
}

pub fn has_parallel_review_candidate_context(query: &str, tokens: &[String]) -> bool {
    match provider_cell().read() {
        Ok(guard) => guard.has_parallel_review_candidate_context(query, tokens),
        Err(poisoned) => {
            tracing::warn!("[router-rs] tokenizer: recovering from poisoned RwLock");
            poisoned
                .into_inner()
                .has_parallel_review_candidate_context(query, tokens)
        }
    }
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
        assert!(has_parallel_review_candidate_context(
            "review scope",
            &tokens
        ));
    }
}
