//! Per-query signal memoization for routing hot paths.
//!
//! Uses per-thread caching so `rayon` parallel scoring threads each maintain
//! their own cache, eliminating the global Mutex bottleneck that previously
//! serialized all parallel scoring threads.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

struct SignalCacheState {
    query_key: u64,
    hits: HashMap<&'static str, bool>,
}

thread_local! {
    static SIGNAL_CACHE: std::cell::RefCell<Option<SignalCacheState>> =
        const { std::cell::RefCell::new(None) };
}

fn query_fingerprint(query_text: &str, query_token_list: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();
    query_text.hash(&mut hasher);
    for token in query_token_list {
        token.hash(&mut hasher);
    }
    hasher.finish()
}

/// Reset the query cache (optional; `cached_signal` auto-resets on query change).
#[cfg(test)]
pub fn signal_cache_reset() {
    SIGNAL_CACHE.with(|cache| {
        *cache.borrow_mut() = None;
    });
}

/// Memoize a boolean routing signal for the current query on this thread.
pub fn cached_signal(
    name: &'static str,
    query_text: &str,
    query_token_list: &[String],
    mut eval: impl FnMut() -> bool,
) -> bool {
    let key = query_fingerprint(query_text, query_token_list);
    SIGNAL_CACHE.with(|cache| {
        let mut guard = cache.borrow_mut();
        if guard.as_ref().map(|state| state.query_key) != Some(key) {
            *guard = Some(SignalCacheState {
                query_key: key,
                hits: HashMap::new(),
            });
        }
        let state = guard.as_mut().expect("cache state just initialized");
        if let Some(&hit) = state.hits.get(name) {
            return hit;
        }
        let value = eval();
        state.hits.insert(name, value);
        value
    })
}

#[cfg(test)]
mod tests {
    use super::super::text::{normalize_text, tokenize_route_text};
    use super::*;

    #[test]
    fn cached_signal_deduplicates_within_same_query() {
        signal_cache_reset();
        let tokens = tokenize_route_text("workflow orchestration sidecar");
        let q = normalize_text("workflow orchestration sidecar");
        let mut calls = 0u32;
        let mut eval = || {
            calls += 1;
            true
        };
        assert!(cached_signal("probe", &q, &tokens, &mut eval));
        assert!(cached_signal("probe", &q, &tokens, &mut eval));
        assert_eq!(calls, 1, "second lookup must hit cache");
        signal_cache_reset();
    }

    #[test]
    fn cached_signal_resets_when_query_changes() {
        signal_cache_reset();
        let t1 = tokenize_route_text("alpha");
        let t2 = tokenize_route_text("beta");
        let q1 = normalize_text("alpha");
        let q2 = normalize_text("beta");
        let mut calls = 0u32;
        let mut eval = || {
            calls += 1;
            true
        };
        assert!(cached_signal("probe", &q1, &t1, &mut eval));
        assert!(cached_signal("probe", &q2, &t2, &mut eval));
        assert_eq!(calls, 2, "different query must re-evaluate");
        signal_cache_reset();
    }
}
