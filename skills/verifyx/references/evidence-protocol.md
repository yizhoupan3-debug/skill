# Evidence Protocol

Evidence is the foundation of verifyx / framework verification.

## Evidence Definition

Evidence = verification command + result + timestamp + provenance

## Evidence Index Schema (`evidence-index-v2`)

`EVIDENCE_INDEX.json` uses `schema_version: evidence-index-v2` with a top-level **`artifacts[]`** array. Each artifact row:

```json
{
  "schema_version": "evidence-index-v2",
  "artifacts": [
    {
      "recorded_at": "2026-05-19T10:30:00Z",
      "command_preview": "cargo test --lib",
      "exit_code": 0,
      "success": true,
      "kind": "verifyx",
      "lifecycle_command": "/verifyx",
      "tags": ["test", "unit", "core"]
    }
  ]
}
```

Common fields on each `artifacts[]` row: `command_preview`, `recorded_at`, `exit_code`, `success`, `kind` (`cursor_post_tool_verification` | `codex_post_tool_verification` | `manual_verification` | `hook_evidence` | `verifyx` | `missing_evidence`), optional `lifecycle_command` (`/verifyx` | `/implementx`), optional `tags`, optional `stdout_snippet`.

## Evidence Collection Rules

1. **Every command**: Log to `EVIDENCE_INDEX.json` under `artifacts/current/<task_id>/`
2. **Pass or fail**: Log both (failure evidence is valuable)
3. **Include context**: stdout snippet, duration, tags
4. **Tag by type**: test, lint, security, docs, etc.

## Evidence Types

### Cross-host PostTool (Auto, opt-in)

When `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1` and router-rs hooks detect verification-like commands:

```bash
cargo test
pytest
npm test
cargo clippy
```

Auto-logged with `kind: "cursor_post_tool_verification"` or `codex_post_tool_verification`.

**双轨说明**：
- **Cursor / Codex**（有 shell hook）：环境变量 `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1` 启用后，PostToolUse hook 自动捕获验证命令并写入 `EVIDENCE_INDEX.json`。

### Manual Verification

User or agent runs verification:

```bash
cargo audit
cargo tarpaulin
```

Logged with `kind: "manual_verification"`

### Hook Evidence

Explicit evidence append via stdio (Cursor / Codex 环境):

```bash
printf '{"id":1,"op":"framework_hook_evidence_append",...}' | router-rs --stdio-json
```

Logged with `kind: "hook_evidence"`

### Verifyx / lifecycle

Ship-phase verification under my-* lifecycle:

```bash
schema-drift check
router-rs closeout evaluate
```

Logged with `kind: "verifyx"` and optional `lifecycle_command: "/verifyx"`.

## Evidence Aggregation

Replace `<task_id>` with the active task directory under `artifacts/current/<task_id>/`.

### By Category

```bash
# Get all test evidence
grep -A5 '"kind": ".*test.*"' artifacts/current/<task_id>/EVIDENCE_INDEX.json

# Get verifyx evidence
grep -A5 '"kind": "verifyx"' artifacts/current/<task_id>/EVIDENCE_INDEX.json
```

### By Tag or lifecycle

```bash
# Optional tags on artifact rows (when present)
grep -B2 -A3 '"tags".*"wave"' artifacts/current/<task_id>/EVIDENCE_INDEX.json

# Filter by lifecycle command
jq '.artifacts[] | select(.lifecycle_command == "/implementx")' artifacts/current/<task_id>/EVIDENCE_INDEX.json
```

### Summary Statistics

```bash
# Count evidence by type
jq '[.artifacts[].kind] | group_by(.) | map({kind: .[0], count: length})' artifacts/current/<task_id>/EVIDENCE_INDEX.json
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

- `artifacts/current/<task_id>/EVIDENCE_INDEX.json` (summary, ~1KB per entry)
- `artifacts/current/<task_id>/evidence/` (detailed logs, as needed)

Evidence is removed when verifyx purges the task dir after ship (see `skills/verifyx/SKILL.md`).

## Post-verify purge and closeout notes

After `router-rs closeout evaluate` writes `artifacts/closeout/<task_id>.json`:

1. Delete `artifacts/current/<task_id>/` (mandatory purge).
2. Record purge intent in the closeout record **`notes`** field if useful (e.g. `task_artifacts_purged; task_dir_removed`).

These purge markers are **operator checklist text only**. They are **not** fields on `CLOSEOUT_RECORD_SCHEMA.json` / `CloseoutRecord` (`deny_unknown_fields` rejects unknown keys).

## Evidence Audit

Before ship, audit evidence:

1. Every planned verification has evidence
2. All evidence has valid `recorded_at` timestamps
3. No conflicting evidence (same command, different results)
4. Evidence chain is complete (start → end)
