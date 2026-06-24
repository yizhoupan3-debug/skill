//! B0 kernel DI: TokenizerProvider (B1→B0) and Telemetry MPSC aggregator.
use framework_kernel::{
    LogAggregator, LogAggregatorHandle, TokenizerProvider, install_global_telemetry_writer,
    install_tokenizer_provider,
};
use routing_engine::routing_runtime_watch;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};
use std::thread;
use std::time::Duration;

struct RouteTokenizerProvider;

impl TokenizerProvider for RouteTokenizerProvider {
    fn tokenize_query(&self, text: &str) -> Vec<String> {
        routing_engine::route::tokenize_query(text)
    }

    fn has_parallel_review_candidate_context(&self, query: &str, tokens: &[String]) -> bool {
        routing_engine::route::has_parallel_review_candidate_context(query, tokens)
    }
}

static BOOTSTRAP_ONCE: Once = Once::new();
static TELEMETRY_HANDLE: Mutex<Option<LogAggregatorHandle>> = Mutex::new(None);

/// Idempotent B0 wiring (tokenizer DI + telemetry + review context probes).
pub fn ensure_kernel_bootstrap() {
    BOOTSTRAP_ONCE.call_once(|| {
        bootstrap_core();        // tokenizer + probes (all modes need these)
        bootstrap_telemetry();   // LogAggregator + EvolutionObserver + route poller thread
    });
}

/// Light bootstrap for short-lived CLI subprocesses (hook events, etc.).
///
/// Skips LogAggregator, EvolutionObserver, and route cache poller thread —
/// these are file-backed services designed for long-lived processes.
/// A hook subprocess (~30ms lifetime) doesn't need them and the thread
/// would be killed by OS process exit before doing useful work.
pub fn ensure_kernel_bootstrap_light() {
    BOOTSTRAP_ONCE.call_once(|| {
        bootstrap_core();        // only tokenizer + probes — no threads, no file handles
    });
}

/// Core DI: tokenizer provider + review context probes.
/// Light enough for any subprocess — no threads, no file handles.
fn bootstrap_core() {
    install_tokenizer_provider(Box::new(RouteTokenizerProvider));
    core_policy::review_context_signals::install_review_context_probes(
        routing_engine::route::has_paper_context,
        routing_engine::route::has_github_pr_context,
    );
}

/// Full telemetry stack: LogAggregator, EvolutionObserver, route cache poller.
/// Only for long-lived processes (stdio JSON loop, etc.).
fn bootstrap_telemetry() {
    let journal = PathBuf::from("artifacts/telemetry/events.jsonl");
    let handle = LogAggregator::start(&journal);
    let observer = fr_exec::evolution_observer::EvolutionObserver::new(
        fr_exec::evolution_observer::EvolutionObserverConfig {
            alerts_path: PathBuf::from("artifacts/evolution/alerts.jsonl"),
            ..Default::default()
        },
    );
    let fanout = fr_exec::evolution_observer::FanoutTelemetryWriter::new(
        handle.writer(),
        observer,
    );
    install_global_telemetry_writer(Arc::new(fanout));
    let _ = TELEMETRY_HANDLE.lock().unwrap().replace(handle);
    spawn_routing_runtime_cache_invalidator();
}

/// Gracefully shut down telemetry: flush pending events and join the aggregator thread.
/// Safe to call multiple times (second call is a no-op).
pub fn shutdown_telemetry() {
    if let Some(handle) = TELEMETRY_HANDLE.lock().unwrap().take() {
        handle.shutdown();
    }
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
            let _ = routing_engine::route::invalidate_records_cache();
        }
    });
}
