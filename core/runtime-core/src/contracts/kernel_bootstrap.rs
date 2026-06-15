//! B0 kernel DI: TokenizerProvider (B1→B0) and Telemetry MPSC aggregator.
use framework_kernel::{
    LogAggregator, LogAggregatorHandle, TokenizerProvider, install_global_telemetry_writer,
    install_tokenizer_provider,
};
use routing_engine::routing_runtime_watch;
use std::path::PathBuf;
use std::sync::{Arc, Once, OnceLock};
use std::thread;
use std::time::Duration;

struct RouteTokenizerProvider;

impl TokenizerProvider for RouteTokenizerProvider {
    fn tokenize_query(&self, text: &str) -> Vec<String> {
        crate::route::tokenize_query(text)
    }

    fn has_parallel_review_candidate_context(&self, query: &str, tokens: &[String]) -> bool {
        crate::route::has_parallel_review_candidate_context(query, tokens)
    }
}

static BOOTSTRAP_ONCE: Once = Once::new();
static TELEMETRY_HANDLE: OnceLock<LogAggregatorHandle> = OnceLock::new();

/// Idempotent B0 wiring (tokenizer DI + telemetry + review context probes).
pub fn ensure_kernel_bootstrap() {
    BOOTSTRAP_ONCE.call_once(|| {
        install_tokenizer_provider(Box::new(RouteTokenizerProvider));
        core_policy::review_context_signals::install_review_context_probes(
            |text, tokens| crate::route::has_paper_context(text, tokens),
            |text, tokens| crate::route::has_github_pr_context(text, tokens),
        );
        let journal = PathBuf::from("artifacts/telemetry/events.jsonl");
        let handle = LogAggregator::start(&journal);
        let observer = crate::framework_runtime::evolution_observer::EvolutionObserver::new(
            crate::framework_runtime::evolution_observer::EvolutionObserverConfig {
                alerts_path: PathBuf::from("artifacts/evolution/alerts.jsonl"),
                ..Default::default()
            },
        );
        let fanout = crate::framework_runtime::evolution_observer::FanoutTelemetryWriter::new(
            handle.writer(),
            observer,
        );
        install_global_telemetry_writer(Arc::new(fanout));
        let _ = TELEMETRY_HANDLE.set(handle);
        spawn_routing_runtime_cache_invalidator();
    });
}

/// Invalidate route record cache when `SKILL_ROUTING_RUNTIME.json` changes on disk (P1-1).
/// Polls every 1s (config file changes don't need sub-second detection).
fn spawn_routing_runtime_cache_invalidator() {
    thread::spawn(|| {
        let watch = routing_runtime_watch();
        let mut rx = watch.receiver();
        let _ = rx.borrow_and_update();
        loop {
            thread::sleep(Duration::from_secs(1));
            if !matches!(rx.has_changed(), Ok(true)) {
                continue;
            }
            let _ = rx.borrow_and_update();
            let _ = crate::route::invalidate_records_cache();
        }
    });
}
