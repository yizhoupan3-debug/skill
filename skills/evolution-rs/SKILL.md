---
name: evolution-rs
description: |
  Skill evolution engine — audit usage journal, compute per-skill health scores,
  auto-prune zero-usage skills, sync feedback tables, inspect integrity,
  analyze telemetry, snapshot registry state.
  Use when asked to audit skills, check skill health, clean up unused skills,
  or maintain the skill registry.
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
trigger_hints:
  - skill audit
  - skill health
  - skill cleanup
  - skill evolution
  - evolution engine
  - telemetry journal
  - skill 审计
  - skill 健康评分
  - skill 维护
  - 注册表审计
  - 技能审计
  - 技能健康
  - 技能清理
  - 技能状态
  - 技能维护
  - 演化引擎

---

# evolution-rs

Skill evolution core: analyze telemetry, audit journal, compute health scores,
auto-prune zero-usage skills, and snapshot/sync the registry.

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

This skill is a framework-operational tool — run on demand, not at session start.
