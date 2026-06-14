---
last_verified: "2026-06-14"
depends_on:
  - ../spec.md
---

# Module Decoupling Architecture

## Layer Diagram

```
Layer 0 (leaf):  core-state  framework-kernel  routing-engine  codegraph-rs
                 autoresearch-rs  evolution-rs
Layer 1:         core-policy
Layer 2:         host-projection
Layer 3:         runtime-storage  runtime-core (facade) → browser-mcp
Layer 4:         router-rs
```

No circular dependencies. `runtime-core` is the largest coupling hotspot.

## Split Plan (v7)

| New crate | Responsibility | Est. Lines | Extracted from runtime-core |
|-----------|---------------|-----------|-----------------------------|
| `runtime-storage` | State persistence, file locks, atomic write | ~5K | `runtime_storage/`, `background_state/` |
| `framework-runtime` | Framework runtime core loop, dispatch, lifecycle | ~12K | `framework_runtime/`, `execution_contract.rs`, `closeout_enforcement.rs`, `rfv_loop.rs` |
| `session-supervisor` | Worker management, session lifecycle | ~4K | `session_supervisor/`, `harness_operator_nudges.rs` |
| `trace-runtime` | Event tracing, observation, journal | ~4K | `trace_runtime.rs`, `trace_stream_io.rs`, `telemetry_journal.rs` |

## Dependency Rules

1. Lower layers must not depend on higher layers.
2. `runtime-core` (facade) may re-export from extracted crates for backward compatibility.
3. `browser-mcp` depends on `runtime-core` only for shared state types.
4. No crate may depend on more than 3 path dependencies within the workspace.
