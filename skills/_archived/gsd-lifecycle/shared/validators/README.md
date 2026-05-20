---
name: gsd-json-schema-validators
description: |
  JSON Schema validators for GSD state files.
  Validates: GOAL_STATE, EVIDENCE_INDEX, WAVE_STATE, SHIPPING_STATE, METRICS
version: "1.0"
platforms: [desktop-mcp, cli, codex, cursor]
---

# GSD JSON Schema Validators

This directory contains JSON Schema definitions for all GSD state files.

## Files

| File | Purpose |
|------|---------|
| `goal-state.schema.json` | GOAL_STATE.json validation |
| `evidence-index.schema.json` | EVIDENCE_INDEX.json validation |
| `wave-state.schema.json` | WAVE_STATE.json validation |
| `shipping-state.schema.json` | SHIPPING_STATE.json validation |
| `metrics.schema.json` | METRICS.json validation |
| `validate.sh` | CLI validation script |

## Usage

```bash
# Validate a state file
./validate.sh goal-state path/to/GOAL_STATE.json

# Validate all state files
./validate.sh all path/to/artifacts/current/<task_id>/
```

## Validation Levels

| Level | Scope | When |
|-------|-------|------|
| Layer 1: Skill | JSON syntax + required fields | On read/write |
| Layer 2: Rust | Framework validation | stdio operations |
| Layer 3: CI/CD | Full schema compliance | PR checks |

## Error Handling

| Error Type | Behavior |
|------------|----------|
| Missing required field | Reject write, show field name |
| Type mismatch | Reject write, show expected type |
| Invalid enum | Reject write, show valid options |
| Semantic conflict | Warn, do not block |
