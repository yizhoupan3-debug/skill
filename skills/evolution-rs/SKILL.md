---
name: evolution-rs
description: |
  Framework-operational reference: audit usage journal, compute per-skill health scores,
  auto-prune zero-usage skills, sync feedback tables, inspect integrity,
  analyze telemetry, snapshot registry state.
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
    - evolution
    - audit
    - health
    - registry
    - maintenance
session_start: never
user-invocable: false
disable-model-invocation: true

---

# evolution-rs

Framework-operational reference: analyze telemetry, audit journal, compute health scores,
auto-prune zero-usage skills, and snapshot/sync the registry.
This is a reference document for framework maintainers — NOT a user-invocable skill.

## Commands

- **audit** — Analyze evolution journal, suggest repairs or new skills via Jaccard similarity
- **manifest** — Emit registry/usage snapshots from telemetry journal
- **sync** — Sync journal entries to Markdown feedback table with dedup
- **snapshot** — Versioned snapshot of skill registry + manifest
- **inspect** — SHA-256 integrity check of a skill directory
- **heal** — Dry-run and auto-prune zero-usage skills to `.backups/pruned`
- **analyze** — Read telemetry events, write `artifacts/evolution/analysis.json`
- **health-score** — Per-skill health scores from telemetry journal

## Usage

```bash
# Audit journal for suggestions (last 30 days)
cargo run -p evolution-rs -- audit -j artifacts/evolution/journal.jsonl

# Compute health scores
cargo run -p evolution-rs -- health-score

# Heal: dry-run prune
cargo run -p evolution-rs -- heal --dry-run -j artifacts/evolution/journal.jsonl \
  -m skills/SKILL_MANIFEST.json -s skills/

# Snapshot registry
cargo run -p evolution-rs -- snapshot -m skills/SKILL_MANIFEST.json \
  -r skills/SKILL_ROUTING_RUNTIME.json
```

## Maintenance

This is a framework-operational reference — run commands on demand, not at session start.
Skill framework routing does not dispatch to this document; use `skill-framework-developer` instead.
