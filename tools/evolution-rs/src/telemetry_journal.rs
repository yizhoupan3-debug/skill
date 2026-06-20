use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::Path;

// Re-export from the single source of truth in telemetry-types.
pub use telemetry_types::{PredictionOutcomeCheck, TelemetryEvent};

#[derive(Debug, Clone, PartialEq)]
pub struct TimestampedTelemetryEvent {
    pub ts: Option<String>,
    pub event: TelemetryEvent,
}

#[derive(Debug, Default, Clone)]
pub struct TelemetryJournal {
    pub events: Vec<TimestampedTelemetryEvent>,
}

#[derive(Debug, Clone, Deserialize)]
struct JournalLine {
    #[serde(default)]
    ts: Option<String>,
    #[serde(flatten)]
    event: TelemetryEvent,
}

/// Route-decision row for `audit` / `manifest` (legacy journal field layout).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AuditJournalEntry {
    pub ts: String,
    pub task: String,
    pub init: String,
    pub final_skill: String,
    #[serde(default)]
    pub conf: f32,
    #[serde(default)]
    pub diff: i32,
    #[serde(default)]
    pub reroute: bool,
    #[serde(default)]
    pub struggle: i32,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub failed_trigger: String,
    #[serde(default)]
    pub notes: String,
}

/// Legacy journal entry with single-char JSON keys (`t`, `tk`, `i`, `f`, …).
/// Kept for backward-compatible reads in [`load_audit_journal_entries`].
#[derive(Debug, Deserialize, Clone)]
struct LegacyShortKeyEntry {
    #[serde(rename = "t")]
    ts: String,
    #[serde(rename = "tk")]
    task: String,
    #[serde(rename = "i")]
    init: String,
    #[serde(rename = "f")]
    final_skill: String,
    #[serde(rename = "c", default)]
    conf: f32,
    #[serde(rename = "d", default)]
    diff: i32,
    #[serde(rename = "r", default)]
    reroute: bool,
    #[serde(rename = "s", default)]
    struggle: i32,
    #[serde(rename = "re", default)]
    reason: String,
    #[serde(rename = "ft", default)]
    failed_trigger: String,
    #[serde(rename = "n", default)]
    notes: String,
}

/// Legacy journal entry with full-length JSON keys and `"final"` instead of `"final_skill"`.
/// Kept for backward-compatible reads in [`load_audit_journal_entries`].
#[derive(Debug, Deserialize)]
struct LegacyFullKeyEntry {
    ts: String,
    task: String,
    init: String,
    #[serde(rename = "final")]
    final_skill: String,
    #[serde(default)]
    conf: f32,
    #[serde(default)]
    diff: i32,
    #[serde(default)]
    reroute: bool,
    #[serde(default)]
    struggle: i32,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    failed_trigger: String,
    #[serde(default)]
    notes: String,
}

pub fn load_telemetry_journal(path: &Path) -> anyhow::Result<TelemetryJournal> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Ok(TelemetryJournal::default());
        }
        Err(err) => return Err(err.into()),
    };
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    let mut parse_errors = 0u32;
    for line in reader.lines() {
        let line = line.with_context(|| format!("read {}", path.display()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<JournalLine>(trimmed) {
            Ok(parsed) => {
                events.push(TimestampedTelemetryEvent {
                    ts: parsed.ts,
                    event: parsed.event,
                });
            }
            Err(_) => {
                parse_errors += 1;
            }
        }
    }
    if parse_errors > 0 {
        eprintln!(
            "Warning: {} lines in {} failed to parse as TelemetryEvent",
            parse_errors,
            path.display()
        );
    }
    Ok(TelemetryJournal { events })
}

/// Unified audit loader: `TelemetryEvent` lines first, then deprecated legacy journal rows.
pub fn load_audit_journal_entries(path: &Path) -> anyhow::Result<Vec<AuditJournalEntry>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line.with_context(|| format!("read {}", path.display()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<JournalLine>(trimmed)
            && let Some(entry) = audit_entry_from_telemetry(&parsed) {
                entries.push(entry);
                continue;
            }
        if let Ok(legacy) = serde_json::from_str::<LegacyShortKeyEntry>(trimmed) {
            entries.push(AuditJournalEntry {
                ts: legacy.ts,
                task: legacy.task,
                init: legacy.init,
                final_skill: legacy.final_skill,
                conf: legacy.conf,
                diff: legacy.diff,
                reroute: legacy.reroute,
                struggle: legacy.struggle,
                reason: legacy.reason,
                failed_trigger: legacy.failed_trigger,
                notes: legacy.notes,
            });
            continue;
        }
        if let Ok(legacy) = serde_json::from_str::<LegacyFullKeyEntry>(trimmed) {
            entries.push(AuditJournalEntry {
                ts: legacy.ts,
                task: legacy.task,
                init: legacy.init,
                final_skill: legacy.final_skill,
                conf: legacy.conf,
                diff: legacy.diff,
                reroute: legacy.reroute,
                struggle: legacy.struggle,
                reason: legacy.reason,
                failed_trigger: legacy.failed_trigger,
                notes: legacy.notes,
            });
        }
    }
    Ok(entries)
}

fn audit_entry_from_telemetry(line: &JournalLine) -> Option<AuditJournalEntry> {
    match &line.event {
        TelemetryEvent::RouteDecision {
            task,
            skill,
            confidence,
            reroute,
            ..
        } => Some(AuditJournalEntry {
            ts: line.ts.clone().unwrap_or_default(),
            task: task.clone(),
            init: "none".to_string(),
            final_skill: skill.clone(),
            conf: *confidence,
            diff: 0,
            reroute: *reroute,
            struggle: 0,
            reason: String::new(),
            failed_trigger: String::new(),
            notes: String::new(),
        }),
        _ => None,
    }
}

pub fn event_within_window(ts: Option<&str>, cutoff: DateTime<Utc>) -> bool {
    match ts {
        Some(raw) if !raw.is_empty() => DateTime::parse_from_rfc3339(raw)
            .map(|parsed| parsed.with_timezone(&Utc) >= cutoff)
            .unwrap_or(false), // Reject malformed timestamps
        _ => true, // Events without timestamps are included (backward compat)
    }
}

pub fn default_telemetry_journal_path() -> &'static str {
    "artifacts/telemetry/events.jsonl"
}

pub fn default_evolution_output_dir() -> &'static str {
    "artifacts/evolution"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_parses_prediction_outcome_lines() {
        let dir = std::env::temp_dir().join(format!(
            "evo-pred-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"kind":"prediction_outcome","task_id":"t-1","matched":false,"predicted_verification_status":"passed","predicted_hypothesis":null,"actual_verification_status":"failed","checks_summary":"prediction_verification_status_mismatch:false:warn","checks":[{{"rule":"prediction_verification_status_mismatch","matched":false,"severity":"warn"}}]}}"#
        )
        .unwrap();
        let journal = load_telemetry_journal(&path).unwrap();
        assert_eq!(journal.events.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn load_parses_route_decision_lines() {
        let dir = std::env::temp_dir().join(format!(
            "evo-tel-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"kind":"route_decision","task":"pdf task","skill":"pdf","confidence":0.9,"reroute":false}}"#
        )
        .unwrap();
        let journal = load_telemetry_journal(&path).unwrap();
        assert_eq!(journal.events.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn load_parses_timestamped_route_decision_lines() {
        let dir = std::env::temp_dir().join(format!(
            "evo-tel-ts-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"ts":"2026-06-01T12:00:00Z","kind":"route_decision","task":"pdf","skill":"pdf","confidence":0.9,"reroute":false}}"#
        )
        .unwrap();
        let journal = load_telemetry_journal(&path).unwrap();
        assert_eq!(journal.events.len(), 1);
        assert_eq!(
            journal.events[0].ts.as_deref(),
            Some("2026-06-01T12:00:00Z")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn audit_loader_accepts_telemetry_and_legacy_formats() {
        let dir = std::env::temp_dir().join(format!(
            "evo-audit-mix-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"ts":"2026-06-01T12:00:00Z","kind":"route_decision","task":"telemetry task","skill":"pdf","confidence":0.8,"reroute":true}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"t":"2026-06-02T12:00:00Z","tk":"legacy task","i":"none","f":"csv","c":0.7,"r":false}}"#
        )
        .unwrap();
        let entries = load_audit_journal_entries(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].task, "telemetry task");
        assert_eq!(entries[0].final_skill, "pdf");
        assert_eq!(entries[1].task, "legacy task");
        assert_eq!(entries[1].final_skill, "csv");
        let _ = std::fs::remove_dir_all(dir);
    }
}
