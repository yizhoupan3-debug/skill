//! Lightweight online telemetry observer.
//!
//! Subscribes to `TelemetryEvent` via a fan-out `TelemetryWriter`, maintains sliding-window
//! counters, and appends threshold alerts to `artifacts/observer/alerts.jsonl`.

use framework_kernel::{MpscTelemetryWriter, TelemetryEvent, TelemetryWriter};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use core_policy::error::FrameworkError;
type Result<T> = std::result::Result<T, FrameworkError>;

// Defaults match configs/observer/observer.toml [observer] section
const DEFAULT_WINDOW_CAPACITY: usize = 256;
const DEFAULT_REROUTE_RATE_ALERT: f32 = 0.35;
const DEFAULT_TOOL_FAILURE_RATE_ALERT: f32 = 0.25;
const DEFAULT_LOW_CONFIDENCE: f32 = 0.45;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryObserverConfig {
    pub window_capacity: usize,
    pub reroute_rate_alert: f32,
    pub tool_failure_rate_alert: f32,
    pub low_confidence_threshold: f32,
    pub alerts_path: PathBuf,
}

impl Default for TelemetryObserverConfig {
    fn default() -> Self {
        Self {
            window_capacity: DEFAULT_WINDOW_CAPACITY,
            reroute_rate_alert: DEFAULT_REROUTE_RATE_ALERT,
            tool_failure_rate_alert: DEFAULT_TOOL_FAILURE_RATE_ALERT,
            low_confidence_threshold: DEFAULT_LOW_CONFIDENCE,
            alerts_path: PathBuf::from("artifacts/observer/alerts.jsonl"),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct WindowCounters {
    route_total: u32,
    route_reroute: u32,
    route_low_confidence: u32,
    tool_total: u32,
    tool_failure: u32,
}

#[derive(Debug, Clone, Serialize)]
struct AlertEntry {
    ts: String,
    kind: String,
    metric: String,
    value: f32,
    threshold: f32,
    suggestion: String,
    window: WindowSnapshot,
}

#[derive(Debug, Clone, Serialize)]
struct WindowSnapshot {
    route_total: u32,
    route_reroute: u32,
    route_low_confidence: u32,
    tool_total: u32,
    tool_failure: u32,
}

impl From<&WindowCounters> for WindowSnapshot {
    fn from(c: &WindowCounters) -> Self {
        Self {
            route_total: c.route_total,
            route_reroute: c.route_reroute,
            route_low_confidence: c.route_low_confidence,
            tool_total: c.tool_total,
            tool_failure: c.tool_failure,
        }
    }
}

pub struct TelemetryObserver {
    config: TelemetryObserverConfig,
    events_seen: u64,
    counters: WindowCounters,
}

impl TelemetryObserver {
    pub fn new(config: TelemetryObserverConfig) -> Self {
        Self {
            config,
            events_seen: 0,
            counters: WindowCounters::default(),
        }
    }

    pub fn events_seen(&self) -> u64 {
        self.events_seen
    }

    #[cfg(test)]
    fn snapshot(&self) -> WindowSnapshot {
        WindowSnapshot::from(&self.counters)
    }

    /// Process one JSONL journal line (file-tail / offline catch-up).
    pub fn observe_jsonl_line(&mut self, line: &str) -> Result<()> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let event: TelemetryEvent =
            serde_json::from_str(trimmed)?;
        self.observe(&event)
    }

    /// Tail-append new bytes from `journal_path`; `offset` advances on success.
    pub fn tail_journal_file(
        &mut self,
        journal_path: &std::path::Path,
        offset: &mut u64,
    ) -> Result<u32> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(journal_path)?;
        let len = file.metadata()?.len();
        if *offset > len {
            *offset = 0;
        }
        file.seek(SeekFrom::Start(*offset))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        *offset = len;
        let mut seen = 0u32;
        for line in buf.lines() {
            if line.trim().is_empty() {
                continue;
            }
            self.observe_jsonl_line(line)?;
            seen += 1;
        }
        Ok(seen)
    }

    pub fn observe(&mut self, event: &TelemetryEvent) -> Result<()> {
        self.events_seen += 1;
        match event {
            TelemetryEvent::RouteDecision {
                confidence,
                reroute,
                ..
            } => {
                self.push_route_event(*reroute, *confidence);
            }
            TelemetryEvent::ToolCall { success, .. } => {
                self.push_tool_event(!success);
            }
            _ => {}
        }
        self.maybe_emit_alerts()
    }

    fn push_route_event(&mut self, reroute: bool, confidence: f32) {
        if self.counters.route_total as usize >= self.config.window_capacity {
            // FIFO reset: when window is full, reset all counters to start fresh
            self.counters = WindowCounters::default();
        }
        self.counters.route_total += 1;
        if reroute {
            self.counters.route_reroute += 1;
        }
        if confidence < self.config.low_confidence_threshold {
            self.counters.route_low_confidence += 1;
        }
    }

    fn push_tool_event(&mut self, failed: bool) {
        if self.counters.tool_total as usize >= self.config.window_capacity {
            self.counters = WindowCounters::default();
        }
        self.counters.tool_total += 1;
        if failed {
            self.counters.tool_failure += 1;
        }
    }

    fn maybe_emit_alerts(&mut self) -> Result<()> {
        let mut alerts = Vec::new();
        if self.counters.route_total >= 10 {
            let rate = self.counters.route_reroute as f32 / self.counters.route_total as f32;
            if rate >= self.config.reroute_rate_alert {
                alerts.push(self.build_alert(
                    "reroute_rate_high",
                    "reroute_rate",
                    rate,
                    self.config.reroute_rate_alert,
                ));
            }
            let struggle_proxy =
                self.counters.route_low_confidence as f32 / self.counters.route_total as f32;
            if struggle_proxy >= self.config.reroute_rate_alert {
                alerts.push(self.build_alert(
                    "low_confidence_rate_high",
                    "low_confidence_rate",
                    struggle_proxy,
                    self.config.reroute_rate_alert,
                ));
            }
        }
        if self.counters.tool_total >= 10 {
            let fail_rate = self.counters.tool_failure as f32 / self.counters.tool_total as f32;
            if fail_rate >= self.config.tool_failure_rate_alert {
                alerts.push(self.build_alert(
                    "tool_failure_rate_high",
                    "tool_failure_rate",
                    fail_rate,
                    self.config.tool_failure_rate_alert,
                ));
            }
        }
        for alert in alerts {
            self.append_alert(&alert)?;
        }
        Ok(())
    }

    fn build_alert(&self, kind: &str, metric: &str, value: f32, threshold: f32) -> AlertEntry {
        let suggestion = match kind {
            "reroute_rate_high" => "检查目标 skill 的 trigger_hints 是否与常见 query 匹配，或考虑调整路由规则的分词权重".to_string(),
            "low_confidence_rate_high" => "review n-gram 置信度阈值或补充 training data，检查最近 query 模式是否有变化".to_string(),
            "tool_failure_rate_high" => "检查对应 tool 的实现和错误处理，确认是否有配置变更或 API 降级".to_string(),
            _ => format!("{} exceeded threshold {:.2}", metric, threshold),
        };
        AlertEntry {
            ts: rfc3339_now(),
            kind: kind.to_string(),
            metric: metric.to_string(),
            value,
            threshold,
            suggestion,
            window: WindowSnapshot::from(&self.counters),
        }
    }

    fn append_alert(&self, alert: &AlertEntry) -> Result<()> {
        let path = &self.config.alerts_path;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let line = serde_json::to_string(alert)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }
}

/// Fan-out writer: aggregator journal + online observer.
pub struct FanoutTelemetryWriter {
    inner: MpscTelemetryWriter,
    observer: Arc<Mutex<TelemetryObserver>>,
}

impl FanoutTelemetryWriter {
    pub fn new(inner: MpscTelemetryWriter, observer: TelemetryObserver) -> Self {
        Self {
            inner,
            observer: Arc::new(Mutex::new(observer)),
        }
    }

    pub fn observer(&self) -> Arc<Mutex<TelemetryObserver>> {
        Arc::clone(&self.observer)
    }
}

impl TelemetryWriter for FanoutTelemetryWriter {
    fn write_event(&self, event: &TelemetryEvent) -> std::result::Result<(), FrameworkError> {
        match self.observer.lock() {
            Ok(mut guard) => {
                if let Err(e) = guard.observe(event) {
                    tracing::warn!(error = %e, "telemetry observer failed to observe event");
                }
            }
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                if let Err(e) = guard.observe(event) {
                    tracing::warn!(error = %e, "telemetry observer failed to observe event after mutex poison recovery");
                }
            }
        }
        self.inner.write_event(event)
    }
}

fn rfc3339_now() -> String {
    framework_kernel::time::now_iso()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_alerts_path() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("evo-alerts-{suffix}.jsonl"))
    }

    #[test]
    fn observer_counts_route_and_tool_events() {
        let path = temp_alerts_path();
        let mut obs = TelemetryObserver::new(TelemetryObserverConfig {
            alerts_path: path.clone(),
            window_capacity: 64,
            reroute_rate_alert: 1.0,
            tool_failure_rate_alert: 1.0,
            low_confidence_threshold: 0.5,
        });
        obs.observe(&TelemetryEvent::RouteDecision {
            task: "t".into(),
            skill: "pdf".into(),
            confidence: 0.9,
            reroute: true,
            latency_ms: 0,
            reasons: vec![],
            matched_tokens: 0,
            parity_gate: "".into(),
            candidates: vec![],
        })
        .unwrap();
        obs.observe(&TelemetryEvent::ToolCall {
            tool: "shell".into(),
            duration_ms: 10,
            success: false,
        })
        .unwrap();
        let snap = obs.snapshot();
        assert_eq!(snap.route_total, 1);
        assert_eq!(snap.route_reroute, 1);
        assert_eq!(snap.tool_total, 1);
        assert_eq!(snap.tool_failure, 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn high_reroute_rate_writes_alert() {
        let path = temp_alerts_path();
        let cfg = TelemetryObserverConfig {
            alerts_path: path.clone(),
            reroute_rate_alert: 0.3,
            window_capacity: 64,
            ..Default::default()
        };
        let mut obs = TelemetryObserver::new(cfg);
        for _ in 0..10 {
            obs.observe(&TelemetryEvent::RouteDecision {
                task: "x".into(),
                skill: "y".into(),
                confidence: 0.9,
                reroute: true,
                latency_ms: 0,
                reasons: vec![],
                matched_tokens: 0,
                parity_gate: "".into(),
                candidates: vec![],
            })
            .unwrap();
        }
        let raw = fs::read_to_string(&path).unwrap_or_default();
        assert!(
            raw.contains("reroute_rate_high"),
            "expected alert line: {raw}"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn tail_journal_file_processes_new_lines() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("evo-tail-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        let journal = dir.join("events.jsonl");
        let alerts = dir.join("alerts.jsonl");
        fs::write(
            &journal,
            r#"{"kind":"route_decision","task":"a","skill":"pdf","confidence":0.8,"reroute":false,"latency_ms":100,"reasons":[],"matched_tokens":0,"parity_gate":"","candidates":[]}
"#,
        )
        .unwrap();
        let mut obs = TelemetryObserver::new(TelemetryObserverConfig {
            alerts_path: alerts,
            reroute_rate_alert: 1.0,
            ..TelemetryObserverConfig::default()
        });
        let mut offset = 0u64;
        let n = obs.tail_journal_file(&journal, &mut offset).unwrap();
        assert_eq!(n, 1);
        assert_eq!(obs.snapshot().route_total, 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn fanout_writer_forwards_to_inner() {
        use framework_kernel::LogAggregator;
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("evo-fanout-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        let journal = dir.join("events.jsonl");
        let alerts = dir.join("alerts.jsonl");
        let handle = LogAggregator::start(&journal);
        let observer = TelemetryObserver::new(TelemetryObserverConfig {
            alerts_path: alerts.clone(),
            ..TelemetryObserverConfig::default()
        });
        {
            let fanout = FanoutTelemetryWriter::new(handle.writer(), observer);
            fanout
                .write_event(&TelemetryEvent::HookFired {
                    hook_name: "stop".into(),
                    action: "allow".into(),
                })
                .unwrap();
        }
        handle.shutdown();
        let journal_raw = fs::read_to_string(&journal).unwrap();
        assert!(journal_raw.contains("hook_fired"));
        let _ = fs::remove_dir_all(dir);
    }
}
