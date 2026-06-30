use std::path::PathBuf;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::Registry;

use crate::hooks;
use crate::ObservabilityError;

/// Configuration for the observability subscriber.
///
/// # Default
/// ```
/// # use observability_core::ObservabilityConfig;
/// let cfg = ObservabilityConfig::default();
/// assert_eq!(cfg.default_level, "warn");
/// assert!(cfg.stderr);
/// assert!(!cfg.with_target);
/// assert!(cfg.log_dir.is_none());
/// ```
#[derive(Clone, Debug)]
pub struct ObservabilityConfig {
    /// Directory for rolling log files. `None` = no file output.
    pub log_dir: Option<PathBuf>,
    /// Default log level when `RUST_LOG` is unset (e.g. `"warn"`, `"info"`).
    pub default_level: String,
    /// Whether to write logs to stderr. Default: `true`.
    pub stderr: bool,
    /// Whether to include the module target in log output. Default: `false`.
    pub with_target: bool,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_dir: None,
            default_level: "warn".to_string(),
            stderr: true,
            with_target: false,
        }
    }
}

/// Build an `EnvFilter` from `RUST_LOG` or fall back to `default_level`.
fn build_filter(default_level: &str) -> EnvFilter {
    EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level))
}

/// Initialise the global tracing subscriber and install a panic hook.
///
/// Call this **once** at process startup, before any `tracing::*!` macro.
/// Subsequent calls are no-ops (tracing-subscriber's init is idempotent
/// from the perspective of the global subscriber — a second init panics
/// with "global tracing subscriber already set", so this function sets
/// a `OnceLock` guard to avoid that).
pub fn init(config: ObservabilityConfig) -> Result<(), ObservabilityError> {
    // Guard: only init once.
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    if INIT.set(()).is_err() {
        tracing::warn!("observability_core::init called more than once — skipping");
        return Ok(());
    }

    if config.stderr {
        let stderr_layer = tracing_subscriber::fmt::layer()
            .with_target(config.with_target)
            .with_filter(build_filter(&config.default_level));

        if let Some(dir) = &config.log_dir {
            std::fs::create_dir_all(dir).map_err(ObservabilityError::Io)?;

            // Rolling file layer — no individual filter so it captures
            // everything that reaches the subscriber (the global env_filter
            // on stderr doesn't gate the whole subscriber, only its layer).
            let file_appender =
                tracing_appender::rolling::daily(dir, "framework.log");
            let (writer, guard) = tracing_appender::non_blocking(file_appender);
            // Keep the guard alive for the entire process lifetime so the
            // non‑blocking worker thread does not shut down early.
            Box::leak(Box::new(guard));

            let file_layer = tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_target(config.with_target)
                .with_ansi(false);

            Registry::default()
                .with(stderr_layer)
                .with(file_layer)
                .init();
        } else {
            Registry::default().with(stderr_layer).init();
        }
    } else if let Some(dir) = &config.log_dir {
        // File only (no stderr)
        std::fs::create_dir_all(dir).map_err(ObservabilityError::Io)?;

        let file_appender = tracing_appender::rolling::daily(dir, "framework.log");
        let (writer, guard) = tracing_appender::non_blocking(file_appender);
        Box::leak(Box::new(guard));

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_target(config.with_target)
            .with_ansi(false)
            .with_filter(build_filter(&config.default_level));

        Registry::default().with(file_layer).init();
    } else {
        // No outputs configured — register a subscriber that silently
        // discards events so tracing macros don't panic.
        tracing_subscriber::registry()
            .init();
    }

    hooks::install_panic_hook();
    Ok(())
}
