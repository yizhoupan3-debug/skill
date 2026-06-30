# Chain Engine — DAG Task Chain Orchestration

`chain-engine` crate provides the DAG scheduling engine for the Task Chain
system. It extends the original linear `TASK_CHAIN.json` format with
conditional branching, parallel groups, retry policies, timeout groups,
and failure strategies.

## Architecture

```
chain_dag_init → TASK_CHAIN.json (chain-dag-v1)
       │
       ▼
  ┌──────────┐   ┌───────────┐   ┌──────────────┐
  │ scheduler │──▶│  tracker  │──▶│   engine     │
  │ advance   │   │ timeouts  │   │ background   │
  │ dag()     │   │ retries   │   │ poller       │
  │           │   │ failures  │   │ (spawn/stop) │
  └─────┬─────┘   └─────┬─────┘   └──────┬───────┘
        │               │                │
        ▼               ▼                ▼
  TASK_CHAIN.json ←→ task_output files → CHAIN_OUTPUT.json
```

## Modules

| Module | File | Purpose |
|--------|------|---------|
| `types` | `src/types.rs` | `ChainDagRoot`, `DagTaskEntry`, `DagCondition`, `RetryPolicy`, `TimeoutGroupSpec`, `GlobalDagConfig` |
| `scheduler` | `src/scheduler.rs` | `advance_dag()` — idempotent DAG scheduler; `validate_dag()` — cycle/resolve checks; `evaluate_condition()` — condition gate evaluation |
| `tracker` | `src/tracker.rs` | `process_timeouts()` — group-level timeout; `process_failures()` — retry scheduling + failure strategies |
| `engine` | `src/engine.rs` | `spawn_dag_poller()` — background polling thread with backoff; `PollerHandle` — stop/signal/is_running |
| `compat` | `src/compat.rs` | Auto-detect and convert old linear `TASK_CHAIN.json` format |

## Key Design Decisions

### Idempotent Scheduler

`advance_dag()` fully recomputes the DAG state on every call. No persistent
scheduler state means crash-safe restart.

### Fair Round-Robin Scheduling

The scheduler collects all eligible tasks first, then selects from parallel
groups in round-robin fashion (CE-17). Tasks without a `parallel_group` are
not group-limited.

### Group-Level Timeout Clock

The timeout clock starts from the earliest `started_at` across ALL tasks in
the group (including completed ones). This prevents sequential execution
within a group from incorrectly resetting the clock (CE-03 fix).

## Schema: TASK_CHAIN.json (chain-dag-v1)

```json
{
  "schema_version": "chain-dag-v1",
  "chain_id": "fix-bugs",
  "mode": "dag",
  "tasks": [{
    "task_id": "scan",
    "title": "Scan codebase",
    "depends_on": [],
    "condition": null,
    "parallel_group": null,
    "timeout_group": null,
    "retry": null,
    "status": "completed"
  }, {
    "task_id": "fix",
    "title": "Fix issues",
    "depends_on": ["scan"],
    "condition": {
      "source": "scan",
      "type": "output_field",
      "field": "outputs.verification_status",
      "operator": "eq",
      "value": "passed"
    },
    "parallel_group": "fixers",
    "retry": { "max_attempts": 3, "backoff_base_ms": 1000 }
  }],
  "global_config": {
    "max_concurrent_tasks": 4,
    "on_any_failure": "pause_dag"
  }
}
```

### Task Status Lifecycle

```
       ┌───────────────────────────────────────┐
       │              Pending ◄─────────────────│──── retry_scheduled (backoff)
       │                  │                    │
       ▼                  │                    │
     Running ◄────────────┘                    │
       │                                       │
       ├──→ Completed                          │
       ├──→ Failed ──→ retry_scheduled ────────┘
       │          └──→ (exhausted) stays Failed
       └──→ Skipped (condition false)
       └──→ Blocked (failure strategy paused)
```

### Global Config

| Field | Default | Description |
|-------|---------|-------------|
| `max_concurrent_tasks` | 4 | Max tasks concurrently in Running state |
| `default_retry` | None | Default retry policy for tasks without explicit retry |
| `on_any_failure` | `pause_dag` | Strategy: `pause_dag`, `abort_dag`, `continue` |

## Backward Compatibility

Old linear `TASK_CHAIN.json` (with `current_index`) is auto-detected and
upgraded to `ChainDagRoot` with `mode: "linear"`. The original
`tool_task_chain_advance` continues to work for linear chains.
