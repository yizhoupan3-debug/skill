---
name: gsd-verify-work
description: |
  Verify work results with evidence-driven approach.
  Use after /gsd-execute-phase completes. Runs verification commands, checks coverage,
  detects schema drift, and produces EVIDENCE_INDEX entries.
routing_layer: L1
routing_owner: owner
routing_gate: evidence
routing_gate_evidence: "WAVE_STATE.json shows global_status=completed"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /gsd-verify-work
  - gsd verify
  - gsd test
  - verify results
  - run tests
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: [gsd, verification, evidence, testing]
---

# gsd-verify-work

Verify all work results with evidence-driven approach.

## Pre-Conditions

1. All waves completed (WAVE_STATE.json global_status=completed)
2. GOAL_STATE.json exists
3. Task directory is writable

## Verification Categories

### 1. Test Coverage

| Check | Command | Threshold |
|-------|---------|-----------|
| Unit tests | `cargo test --lib` | 100% pass |
| Integration tests | `cargo test --test '*'` | 100% pass |
| Coverage | `cargo tarpaulin` | ≥80% |
| E2E tests | `e2e test` | 100% pass |

### 2. Code Quality

| Check | Command | Threshold |
|-------|---------|-----------|
| Lint | `cargo clippy -- -D warnings` | 0 warnings |
| Format | `cargo fmt -- --check` | clean |
| Security | `cargo audit` | 0 vulnerabilities |
| Dependencies | `cargo outdated` | no critical updates |

### 3. Documentation

| Check | Command | Threshold |
|-------|---------|-----------|
| Docs build | `cargo doc --no-deps` | 0 warnings |
| README | exists + current | N/A |
| CHANGELOG | exists + updated | N/A |

### 4. Schema Drift

Detect changes between:
- REQUIREMENTS.md (what we planned)
- Implementation (what we built)

```bash
# Compare expected vs actual
diff <(grep "schema\|model\|struct" REQUIREMENTS.md) \
     <(grep "schema\|model\|struct" src/**/*.rs)
```

## Evidence Protocol

Every verification command must produce evidence:

```json
{
  "id": "uuid",
  "timestamp": "ISO8601",
  "command": "cargo test",
  "exit_code": 0,
  "duration_ms": 5000,
  "result_summary": "50 tests passed, 0 failed",
  "kind": "verification",
  "gsd_command": "gsd-verify-work"
}
```

## Stdio Operations

```bash
# Write evidence
printf '%s\n' '{"id":1,"op":"framework_hook_evidence_append","payload":{"repo_root":"<path>","command_preview":"cargo test --lib","result":"50 passed","kind":"gsd-verify-work"}}' | router-rs --stdio-json

# Checkpoint goal
printf '%s\n' '{"id":2,"op":"framework_autopilot_goal","payload":{"operation":"checkpoint","repo_root":"<path>","note":"verification checkpoint: tests passed"}}' | router-rs --stdio-json
```

## Verification Checklist

```
Verification Categories:
□ Test Coverage
  □ Unit tests pass (cargo test --lib)
  □ Integration tests pass (cargo test --test)
  □ Coverage ≥ 80% (cargo tarpaulin)
  □ E2E tests pass
  
□ Code Quality
  □ Clippy clean (cargo clippy)
  □ Format clean (cargo fmt)
  □ Audit clean (cargo audit)
  
□ Documentation
  □ Docs build clean (cargo doc)
  □ README exists and current
  □ CHANGELOG updated
  
□ Schema Consistency
  □ No drift between plan and implementation
  
Evidence:
□ All verification commands logged to EVIDENCE_INDEX.json
□ Checkpoints written to GOAL_STATE.json
□ Summary written to SESSION_SUMMARY.md
```

## Next Step

After verification for the current phase, proceed to `/gsd-ship` or the next phase's `/gsd-discuss-phase` (per-phase discuss → plan → execute → verify).

## Anti-Patterns

- Don't claim "tests passed" without evidence
- Don't skip coverage checks
- Don't ignore schema drift
- Don't proceed to ship without full verification
