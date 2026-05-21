# Evidence Protocol

Evidence is the foundation of GSD verification.

## Evidence Definition

Evidence = verification command + result + timestamp + provenance

## Evidence Entry Schema

```json
{
  "id": "uuid-v4",
  "timestamp": "2026-05-19T10:30:00Z",
  "command": "cargo test --lib",
  "exit_code": 0,
  "duration_ms": 5234,
  "result_summary": "50 tests passed, 0 failed, 0 ignored",
  "stdout_snippet": "test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out",
  "kind": "cursor_post_tool_verification | manual_verification | hook_evidence | gsd-verification",
  "gsd_command": "gsd-verify-work | gsd-execute-phase | gsd-ship | null",
  "tags": ["test", "unit", "core"],
  "artifact_path": "path/to/test/results/log"
}
```

## Evidence Collection Rules

1. **Every command**: Log to EVIDENCE_INDEX.json
2. **Pass or fail**: Log both (failure evidence is valuable)
3. **Include context**: stdout snippet, duration, tags
4. **Tag by type**: test, lint, security, docs, etc.

## Evidence Types

### Cursor/Codex PostTool (Auto)

When router-rs hooks detect verification-like commands:
```bash
cargo test
pytest
npm test
cargo clippy
```

Auto-logged with `kind: "cursor_post_tool_verification"`

### Manual Verification

User or agent runs verification:
```bash
cargo audit
cargo tarpaulin
```

Logged with `kind: "manual_verification"`

### Hook Evidence

Explicit evidence append:
```bash
printf '{"id":1,"op":"framework_hook_evidence_append",...}' | router-rs --stdio-json
```

Logged with `kind: "hook_evidence"`

### GSD Commands

GSD-specific verification:
```bash
schema drift detection
coverage analysis
documentation check
```

Logged with `kind: "gsd-verification"`

## Evidence Aggregation

### By Category

```bash
# Get all test evidence
grep -A5 '"kind": ".*test.*"' EVIDENCE_INDEX.json

# Get all verification evidence
grep -A5 '"kind": "gsd-verification"' EVIDENCE_INDEX.json
```

### By Wave

```bash
# Get evidence for wave 1
grep -B2 -A3 '"wave": 1' EVIDENCE_INDEX.json
```

### Summary Statistics

```bash
# Count evidence by type
jq '[.entries[].kind] | group_by(.) | map({kind: .[0], count: length})' EVIDENCE_INDEX.json
```

## Evidence-Based Decisions

### Verification Pass

If all expected evidence exists AND all exit codes = 0:
→ Verification passed

### Verification Fail

If any evidence shows exit code ≠ 0:
→ Verification failed
→ Log to EVIDENCE_INDEX with failure details
→ Create fix task

### Missing Evidence

If expected evidence is missing:
→ Verification incomplete
→ Log as `kind: "missing_evidence"`
→ Block further progress

## Evidence Retention

Evidence is kept in:
- `EVIDENCE_INDEX.json` (summary, ~1KB per entry)
- `artifacts/current/<task_id>/evidence/` (detailed logs, as needed)

Evidence is not deleted until task completion and closeout.

## Evidence Audit

Before ship, audit evidence:

1. Every planned verification has evidence
2. All evidence has valid timestamps
3. No conflicting evidence (same command, different results)
4. Evidence chain is complete (start → end)
