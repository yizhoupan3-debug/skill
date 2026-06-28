//! Dependency-inversion surface: B0 policy code tokenizes via injected B1 provider.
use std::sync::{OnceLock, RwLock};

/// Tokenization supplied by the routing engine (B1).
pub trait TokenizerProvider: Send + Sync {
    fn tokenize_query(&self, text: &str) -> Vec<String>;
}

static PANIC_TOKENIZER: PanicTokenizer = PanicTokenizer;

fn provider_ref(cell: &Option<Box<dyn TokenizerProvider>>) -> &dyn TokenizerProvider {
    cell.as_deref().unwrap_or(&PANIC_TOKENIZER)
}

struct PanicTokenizer;

impl TokenizerProvider for PanicTokenizer {
    fn tokenize_query(&self, _text: &str) -> Vec<String> {
        panic!(
            "TokenizerProvider not installed — call install_tokenizer_provider() during kernel bootstrap"
        );
    }
}

fn provider_cell() -> &'static RwLock<Option<Box<dyn TokenizerProvider>>> {
    static CELL: OnceLock<RwLock<Option<Box<dyn TokenizerProvider>>>> = OnceLock::new();
    CELL.get_or_init(|| RwLock::new(None))
}

/// B1 calls during process startup (idempotent).
pub fn install_tokenizer_provider(provider: Box<dyn TokenizerProvider>) {
    set_tokenizer_provider(provider);
}

/// Replace provider (tests / late binding).
pub fn set_tokenizer_provider(provider: Box<dyn TokenizerProvider>) {
    *provider_cell().write().unwrap_or_else(|e| e.into_inner()) = Some(provider);
}

pub fn tokenize_query(text: &str) -> Vec<String> {
    match provider_cell().read() {
        Ok(guard) => provider_ref(&guard).tokenize_query(text),
        Err(poisoned) => {
            tracing::warn!("[router-rs] tokenizer: recovering from poisoned RwLock");
            provider_ref(&poisoned.into_inner()).tokenize_query(text)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    struct Stub;

    impl TokenizerProvider for Stub {
        fn tokenize_query(&self, text: &str) -> Vec<String> {
            vec![text.to_ascii_lowercase()]
        }
    }

    #[test]
    fn install_and_query_tokenizer() {
        set_tokenizer_provider(Box::new(Stub));
        let tokens = tokenize_query("Review");
        assert_eq!(tokens, vec!["review"]);
    }
}
