---
name: skill-maintenance-codex
description: |
  Maintain Codex skill-library operational health through validation, sync checks, drift cleanup,
  and lightweight maintenance follow-ups. Use when the user wants skill 维护, sync health checks,
  registry drift cleanup, or routine skill-library housekeeping rather than framework redesign.
routing_layer: L1
routing_owner: overlay
routing_gate: none
session_start: n/a
trigger_hints:
  - skill 维护
  - sync health checks
  - registry drift cleanup
  - skill library maintenance
  - validate skills
  - skills
  - maintenance
  - sync
  - validation
  - drift
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - skills
    - maintenance
    - sync
    - validation
    - drift
risk: low
source: local
---

# skill-maintenance-codex

This skill owns routine Codex skill-library maintenance and drift cleanup.

## When to use

- The user wants validation, sync, or maintenance on the skill library
- The task is operational upkeep rather than boundary redesign

## Do not use

- Framework redesign or owner/gate policy changes -> use `$skill-developer-codex`
- Concrete skill creation or major rewrites -> use `$skill-creator`
