//! B0 kernel DI: TokenizerProvider (B1→B0), review context probes, and route cache invalidator.
//!
//! Telemetry bootstrap (LogAggregator, TelemetryObserver) removed per v10 Wave 2d.
use framework_kernel::install_tokenizer_provider;
use routing_engine::routing_runtime_watch;
use std::sync::Once;
use std::thread;
use std::time::Duration;

struct RouteTokenizerProvider;

impl framework_kernel::TokenizerProvider for RouteTokenizerProvider {
    fn tokenize_query(&self, text: &str) -> Vec<String> {
        routing_engine::route::tokenize_query(text)
    }

    fn has_parallel_review_candidate_context(&self, query: &str, tokens: &[String]) -> bool {
        routing_engine::route::has_parallel_review_candidate_context(query, tokens)
    }
}

static BOOTSTRAP_ONCE: Once = Once::new();

/// Idempotent B0 wiring (tokenizer DI + probes + route cache invalidator).
pub fn ensure_kernel_bootstrap() {
    BOOTSTRAP_ONCE.call_once(|| {
        bootstrap_core();        // tokenizer + probes (all modes need these)
        spawn_routing_runtime_cache_invalidator();
    });
}

/// Light bootstrap for short-lived CLI subprocesses (hook events, etc.).
///
/// Skips route cache poller thread — a hook subprocess (~30ms lifetime) doesn't
/// need it and the thread would be killed before doing useful work.
pub fn ensure_kernel_bootstrap_light() {
    BOOTSTRAP_ONCE.call_once(|| {
        bootstrap_core();        // only tokenizer + probes — no thread
    });
}

/// Core DI: tokenizer provider + review context probes.
/// Light enough for any subprocess — no threads, no file handles.
fn bootstrap_core() {
    framework_kernel::install_tokenizer_provider(Box::new(RouteTokenizerProvider));
    core_policy::review_context_signals::install_review_context_probes(
        routing_engine::route::has_paper_context,
        routing_engine::route::has_github_pr_context,
    );
}

/// Invalidate route record cache when `SKILL_ROUTING_RUNTIME.json` changes on disk (P1-1).
/// Polls every 1s (config file changes don't need sub-second detection).
fn spawn_routing_runtime_cache_invalidator() {
    thread::spawn(move || {
        let watch = routing_runtime_watch();
        let mut rx = watch.receiver();
        #[allow(clippy::let_unit_value)]
        let _ = rx.borrow_and_update();
        loop {
            thread::sleep(Duration::from_secs(1));
            if !matches!(rx.has_changed(), Ok(true)) {
                continue;
            }
            #[allow(clippy::let_unit_value)]
            let _ = rx.borrow_and_update();
            let _ = routing_engine::route::invalidate_records_cache();
        }
    });
}
