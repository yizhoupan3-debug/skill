---
last_verified: "2026-06-14"
depends_on:
  - ../spec.md
  - ../architecture/module-decoupling.md
---

# ADR-003: runtime-core 拆分策略

## Status

Accepted (2026-06-14).

## Context

`runtime-core` crate 在 v6.5 达到 37,871 行 / 101 文件 / 573 pub API，是框架中最大的编译热点和耦合点。23 条跨 crate re-export 使其成为事实上的 God Module。任何修改都需要重新编译整个 crate，增量编译收益低。

## Decision

将 `runtime-core` 拆分为 4 个职责单一的 crate，保持 `runtime-core` 作为 facade crate：

| crate | 职责 | 从 runtime-core 提取的模块 |
|-------|------|--------------------------|
| `runtime-storage` | 状态持久化、文件锁、atomic write | `runtime_storage/`, `background_state/` |
| `framework-runtime` | 框架运行时核心循环、dispatch、lifecycle | `framework_runtime/`, `execution_contract.rs`, `closeout_enforcement.rs`, `rfv_loop.rs` |
| `session-supervisor` | worker 管理、session 生命周期 | `session_supervisor/`, `harness_operator_nudges.rs` |
| `trace-runtime` | 事件追踪、observation、journal | `trace_runtime.rs`, `trace_stream_io.rs`, `telemetry_journal.rs` |

### 拆分原则

1. **逐 crate 拆分，每步全量测试**：每次提取一个 crate 后运行全量 `cargo test --workspace`，确保无回归。
2. **保持 DAG 无环**：新 crate 只依赖底层 crate（`core-state`, `framework-kernel`, `core-policy`, `routing-engine`），不反向依赖 `runtime-core`。
3. **Facade re-export**：`runtime-core` 保持 `pub use rt_storage::*` 等 re-export 以兼容下游调用者，新代码直接依赖拆分后 crate。

## Consequences

- **编译增量改善**：改 `runtime-storage` 不重编译 `session-supervisor`。
- **最大单 crate 从 38K→15K 行**（facade crate 仅保留 ~13K 行集成代码）。
- **依赖关系更清晰**：`trace-runtime` 不需要依赖 `framework-runtime`。
- **短期代价**：拆分期间可能引入临时 re-export；需在拆分完成后清理。

## Related

- `core/runtime-storage/` — 已提取的 storage crate
- `docs/architecture/module-decoupling.md` — 模块解耦架构（待创建）
- `artifacts/current/roadmap-v7.md` §6 — 模块解耦 Wave
