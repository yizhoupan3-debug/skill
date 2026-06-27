---
name: primary-runtime
allowed_tools:
- Read
- Bash
- Agent
description: Primary runtime entry point — framework lifecycle orchestration. Spawned by the host to handle goal drive, quality gate, and session supervision. Most users should use /plan or a skill-specific entry point instead.
routing_layer: L0
routing_gate: none
metadata:
  platforms:
  - supported
  version: '1.0.0'
---

# primary-runtime

Framework-operational skill that bridges the host's runtime hooks (goal drive,
quality gate, agent orchestrator, eval route) to the skill-layer abstraction.

Intended for framework-internal use. User-facing workflows should prefer
higher-level skills (/plan, /code-review, etc.) that delegate to this runtime
when needed.
