---
last_verified: "2026-06-16"
depends_on:
  - ../spec.md
---

# Module Decoupling Architecture

## Layer Diagram

```
Layer 0 (leaf):  core-state  framework-kernel  routing-engine
                 tools/codegraph-rs  tools/autoresearch-rs  tools/evolution-rs
                 trace-runtime       runtime-storage
Layer 1:         core-policy
Layer 2:         host-projection
Layer 3:         runtime-core (facade) → framework-runtime  session-supervisor
                 runtime-core → browser-mcp
Layer 4:         router-rs
```

No circular dependencies. `runtime-core` is the largest coupling hotspot.

## Split Plan (v7) — Execution Status

| New crate | Location | Responsibility | Est. Lines | Extracted from runtime-core |
|-----------|----------|---------------|-----------|-----------------------------|
| `runtime-storage` | `core/runtime-storage/` | 状态持久化、文件锁、atomic write、后台任务状态 | ~8K | `runtime_storage/`, `background_state/` |
| `framework-runtime` | `core/framework-runtime/` | 框架运行时核心循环、execution contract、pre_tool_use_guard、closeout enforcement、trace I/O | ~5K | `closeout_enforcement.rs`, `execution_contract.rs`, `pre_tool_use_guard.rs`, `runtime_view.rs`, `trace_stream_io.rs`, `trace_attach.rs`, `trace_transport.rs`, `live_execute.rs`, `sandbox_control.rs`, `evolution_observer.rs` |
| `session-supervisor` | `core/session-supervisor/` | Worker 管理、session 生命周期、evolution_idle | ~5K | `session_supervisor/`, `harness_operator_nudges.rs` |
| `trace-runtime` | `core/trace-runtime/` | 事件追踪、observation、journal 聚合入口 | ~1K | `trace_runtime.rs` |

## Dependency Rules

1. Lower layers must not depend on higher layers.
2. `runtime-core` (facade) may re-export from extracted crates for backward compatibility.
3. `browser-mcp` depends on `runtime-core` only for shared state types.
4. No crate may depend on more than 3 path dependencies within the workspace.
