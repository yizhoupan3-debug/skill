use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TelemetryEvent {
    RouteDecision {
        task: String,
        skill: String,
        confidence: f32,
        reroute: bool,
    },
    GoalTransition {
        from: String,
        to: String,
        task_id: String,
    },
    ToolCall {
        tool: String,
        duration_ms: u64,
        success: bool,
    },
    RfvRound {
        round: u32,
        verdict: String,
    },
    HookFired {
        hook_name: String,
        action: String,
    },
    DevExempt {
        path: String,
        action: String,
    },
    PredictionOutcome {
        task_id: String,
        matched: bool,
        predicted_verification_status: Option<String>,
        predicted_hypothesis: Option<String>,
        actual_verification_status: String,
        checks_summary: String,
        checks: Vec<PredictionOutcomeCheck>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PredictionOutcomeCheck {
    pub rule: String,
    pub matched: bool,
    pub severity: String,
}

#[derive(Debug, Default, Clone)]
pub struct TelemetryJournal {
    pub events: Vec<TelemetryEvent>,
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
    for line in reader.lines() {
        let line = line.with_context(|| format!("read {}", path.display()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<TelemetryEvent>(trimmed) {
            events.push(event);
        }
    }
    Ok(TelemetryJournal { events })
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
}
