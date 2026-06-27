---

name: observer-rs
scene: code_review
description: |
  Framework-operational reference: analyze telemetry, audit usage journal,
  compute per-skill health scores, inspect integrity, snapshot registry state.
  NOT a user-invocable skill — reference document for framework maintainers.
routing_layer: L0
routing_owner: none
routing_gate: none
routing_priority: P3
trigger_hints: []
metadata:
  version: "1.0.0"
  platforms: [supported]
  category: framework
  risk: medium
  tags:
    - observability
    - audit
    - health
    - registry
    - maintenance
session_start: never
user-invocable: false
disable-model-invocation: true

---

# observer-rs

Framework-operational reference: analyze telemetry, audit journal, compute health
scores, and snapshot/sync the registry. **Observation only** — this module detects
anomalies and produces actionable guidance but does not auto-heal.
This is a reference document for framework maintainers — NOT a user-invocable skill.

## Commands

- **analyze** — Read telemetry events, write `artifacts/observer/analysis.json` with recommendations
- **audit** — Analyze telemetry journal, suggest repairs or new skills via Jaccard similarity
- **manifest** — Emit registry/usage snapshots from telemetry journal
- **health-score** — Per-skill health scores from telemetry journal
- **inspect** — SHA-256 integrity check of a skill directory
- **sync** — Sync journal entries to Markdown feedback table with dedup
- **snapshot** — Versioned snapshot of skill registry + manifest

## Usage

```bash
# Analyze telemetry (last 30 days)
cargo run -p observer-rs -- analyze

# Audit journal for suggestions
cargo run -p observer-rs -- audit -j artifacts/telemetry/events.jsonl

# Compute health scores
cargo run -p observer-rs -- health-score

# Snapshot registry
cargo run -p observer-rs -- snapshot -m skills/SKILL_MANIFEST.json \
  -r skills/SKILL_ROUTING_RUNTIME.json
```

## Outputs

- `artifacts/observer/analysis.json` — aggregate metrics + per-skill stats + recommendations
- `artifacts/observer/health-score.json` — per-skill blended health scores
- `artifacts/observer/alerts.jsonl` — online threshold breach alerts (TelemetryObserver removed per Wave 2d; output no longer actively generated)

## Maintenance

This is a framework-operational reference — run commands on demand, not at session start.
Skill framework routing does not dispatch to this document; use `skill-framework-developer` instead.
