//! Structured metric counter/gauge with labels.
//!
//! Provides `MetricCounter` — a builder-style counter that emits
//! `TelemetryEvent::MetricEvent` into the JSONL telemetry journal.
//! Unlike raw `HookFired` events, `MetricEvent` carries a numeric `value`
//! and structured `labels` suitable for aggregation queries.
//!
//! # Example
//!
//! ```rust,ignore
//! let counter = MetricCounter::new("agent_starts")
//!     .with_label("host", "claude");
//! counter.increment();     // emits MetricEvent { value: 1.0, ... }
//! counter.emit(123.0);     // emits MetricEvent { value: 123.0, ... }
//! ```

use std::collections::HashMap;
use crate::TelemetryEvent;

/// A named metric counter with optional labels.
///
/// Each `emit()` / `increment()` call writes a `TelemetryEvent::MetricEvent`
/// into the telemetry journal (via `framework_kernel::emit_telemetry`).
#[derive(Debug, Clone)]
pub struct MetricCounter {
    name: String,
    labels: HashMap<String, String>,
}

impl MetricCounter {
    /// Create a new counter with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            labels: HashMap::new(),
        }
    }

    /// Add a label key-value pair.
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// The counter name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Look up a label value by key.
    pub fn label_value(&self, key: &str) -> Option<&str> {
        self.labels.get(key).map(|s| s.as_str())
    }

    /// Build a `TelemetryEvent::MetricEvent` from this counter (without emitting).
    pub fn build_event(&self, value: f64) -> TelemetryEvent {
        TelemetryEvent::MetricEvent {
            metric_name: self.name.clone(),
            value,
            labels: self.labels.clone(),
        }
    }

    /// Emit a metric value to the telemetry journal.
    pub fn emit(&self, value: f64) {
        let event = self.build_event(value);
        crate::emit_telemetry(&event);
    }

    /// Emit an increment (value = 1.0).
    pub fn increment(&self) {
        self.emit(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_new_and_labels() {
        let c = MetricCounter::new("requests").with_label("host", "prod");
        assert_eq!(c.name(), "requests");
        assert_eq!(c.label_value("host"), Some("prod"));
        assert_eq!(c.label_value("nonexistent"), None);
    }

    #[test]
    fn multiple_labels() {
        let c = MetricCounter::new("db_query")
            .with_label("db", "sqlite")
            .with_label("table", "skills");
        assert_eq!(c.label_value("db"), Some("sqlite"));
        assert_eq!(c.label_value("table"), Some("skills"));
    }

    #[test]
    fn build_event_constructs_correct_variant() {
        let c = MetricCounter::new("latency").with_label("op", "search");
        let event = c.build_event(42.5);
        match event {
            TelemetryEvent::MetricEvent { ref metric_name, value, ref labels } => {
                assert_eq!(metric_name, "latency");
                assert!((value - 42.5).abs() < f64::EPSILON);
                assert_eq!(labels.get("op"), Some(&"search".to_string()));
            }
            _ => panic!("expected MetricEvent variant"),
        }
    }

    #[test]
    fn counter_emit_does_not_panic() {
        let c = MetricCounter::new("safe_test");
        // Should not panic even without LogAggregator running
        c.emit(1.0);
        c.increment();
    }
}
