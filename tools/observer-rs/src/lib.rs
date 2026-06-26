#![deny(clippy::unwrap_used, clippy::expect_used)]
pub mod analyze;
pub mod config;
pub mod health_score;
pub mod telemetry_journal;

pub use analyze::run_analyze;
pub use config::{ObserverConfig, blended_health_score, default_observer_config_path, load_config};
pub use health_score::run_health_score;
pub use telemetry_journal::{
    AuditJournalEntry, TelemetryEvent, TelemetryJournal, TimestampedTelemetryEvent,
    default_observer_output_dir, default_telemetry_journal_path, load_audit_journal_entries,
    load_telemetry_journal,
};
