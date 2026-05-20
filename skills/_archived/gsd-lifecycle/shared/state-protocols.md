# State Files Protocol

GSD uses 5 state files for persistence across commands and hosts. Harness continuity also reads/writes shared L2 artifacts: `artifacts/current/<task_id>/GOAL_STATE.json` (goal contract; schema may use `router-rs-autopilot-goal-v1` or GSD-enriched fields), `RFV_LOOP_STATE.json`, and `EVIDENCE_INDEX.json` — treat GSD validators as extensions of those files, not parallel truth.

## Schema Validation

JSON Schema files for validation are located in `shared/validators/`:

| State File | Schema File |
|------------|-------------|
| GOAL_STATE.json | `validators/goal-state.schema.json` |
| RFV_LOOP_STATE.json | (reused from router-rs) |
| EVIDENCE_INDEX.json | `validators/evidence-index.schema.json` |
| WAVE_STATE.json | `validators/wave-state.schema.json` |
| SHIPPING_STATE.json | `validators/shipping-state.schema.json` |
| METRICS.json | `validators/metrics.schema.json` |

See `validators/README.md` for validation levels and usage.

## GOAL_STATE.json

**Location**: `artifacts/current/<task_id>/GOAL_STATE.json`
**Schema**: `router-rs-autopilot-goal-v1`
**Purpose**: Macro goal contract

```json
{
  "schema_version": "router-rs-autopilot-goal-v1",
  "task_id": "string",
  "goal": "string",
  "non_goals": ["string"],
  "done_when": ["string"],
  "validation_commands": ["string"],
  "drive_until_done": true,
  "status": "running|paused|completed|blocked",
  "started_at": "ISO8601",
  "checkpoints": []
}
```

## RFV_LOOP_STATE.json

**Location**: `artifacts/current/<task_id>/RFV_LOOP_STATE.json`
**Schema**: `router-rs-rfv-loop-v1`
**Purpose**: Multi-round adversarial loop ledger

```json
{
  "schema_version": "router-rs-rfv-loop-v1",
  "task_id": "string",
  "goal": "string",
  "max_rounds": 3,
  "current_round": 1,
  "loop_status": "active|paused|closed|blocked",
  "rounds": [
    {
      "round": 1,
      "review_summary": "string",
      "external_research_summary": "string|null",
      "fix_summary": "string",
      "verify_result": "PASS|FAIL|SKIPPED",
      "supervisor_decision": "continue|close|block",
      "findings": [
        {"severity": "P0|P1|P2", "description": "string", "status": "open|fixed|accepted"}
      ]
    }
  ],
  "stop_when": ["string"],
  "started_at": "ISO8601"
}
```

## EVIDENCE_INDEX.json

**Location**: `artifacts/current/<task_id>/EVIDENCE_INDEX.json`
**Schema**: `router-rs-evidence-index-v1`
**Purpose**: Verification command execution records

```json
{
  "schema_version": "router-rs-evidence-index-v1",
  "task_id": "string",
  "entries": [
    {
      "id": "uuid",
      "timestamp": "ISO8601",
      "command": "string",
      "exit_code": 0,
      "duration_ms": 1000,
      "result_summary": "string",
      "kind": "cursor_post_tool_verification|manual_verification|hook_evidence",
      "gsd_command": "gsd-new-project|gsd-execute-phase|gsd-ship|null"
    }
  ]
}
```

## WAVE_STATE.json

**Location**: `artifacts/current/<task_id>/WAVE_STATE.json`
**Schema**: `gsd-wave-state-v1`
**Purpose**: Wave execution state

```json
{
  "schema_version": "gsd-wave-state-v1",
  "task_id": "string",
  "current_wave": 1,
  "waves": [
    {
      "wave_id": 1,
      "phases": ["phase-1", "phase-2"],
      "status": "running|completed|blocked",
      "started_at": "ISO8601",
      "completed_at": "ISO8601|null",
      "agents": [
        {
          "agent_id": "string",
          "assigned_phase": "string",
          "status": "running|completed|failed",
          "context_usage": 20
        }
      ],
      "checkpoint": {
        "last_checkpoint": "string",
        "evidence_files": ["path"]
      }
    }
  ],
  "global_status": "running|completed|blocked"
}
```

### Topology extensions (`my-light` / personal lifecycle)

When using `/my-plan` or `lifecycle_profile: my-light`, each wave may also include:

| Field | Type | Meaning |
|-------|------|---------|
| `wave_key` | string | Stable id for `depends_on` edges (e.g. `w2-routing`) |
| `parallel_group` | string | Lanes sharing a group may run in parallel |
| `depends_on` | string[] | Prior `wave_key` values (serial prerequisite) |
| `execution_mode` | `parallel` \| `serial` | Scheduler hint for `/my-implement` |
| `lanes[]` | array | Per-lane `lane_id`, `scope_paths`, `done_when`, `lane_note_path` |

`/my-implement` runs **all waves in one breath** without pausing at wave boundaries for user confirmation (see `skills/my-implement/SKILL.md`).

## SHIPPING_STATE.json

**Location**: `artifacts/current/<task_id>/SHIPPING_STATE.json`
**Schema**: `gsd-shipping-state-v1`
**Purpose**: Delivery gate state

```json
{
  "schema_version": "gsd-shipping-state-v1",
  "task_id": "string",
  "gates": {
    "test_coverage": {"status": "pass|fail|pending", "coverage_percent": 0},
    "code_quality": {"status": "pass|fail|pending", "lint_results": "path", "security_scan": "path"},
    "documentation": {"status": "pass|fail|pending", "checked_files": []},
    "git_clean": {"status": "pass|fail|pending", "uncommitted_files": []},
    "worktree_review": {"status": "pass|fail|pending", "merge_commit": "string|null"}
  },
  "overall_status": "pass|fail|blocked",
  "started_at": "ISO8601",
  "completed_at": "ISO8601|null"
}
```

## Task ID Resolution

1. Check `artifacts/current/active_task.json` for `task_id`
2. Fallback to `artifacts/current/focus_task.json`
3. If neither exists, prompt user for task_id

## File Creation

State files are created in:
```
artifacts/current/<task_id>/
├── GOAL_STATE.json
├── RFV_LOOP_STATE.json
├── EVIDENCE_INDEX.json
├── WAVE_STATE.json (GSD specific)
└── SHIPPING_STATE.json (GSD specific)
```
