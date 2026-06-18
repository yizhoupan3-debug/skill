---
last_verified: "2026-06-16"
depends_on:
  - ../spec.md
  - ../spec/core-crates.md
---

# ADR-003: runtime-core 拆分策略

## Status

Accepted (2026-06-14). **Execution**: v7 已提取 4 个子 crate，runtime-core 从 ~38K 降至 ~14K 行。  

## Context

`runtime-core` crate 在 v6.5 达到 37,871 行 / 101 文件 / 573 pub API，是框架中最大的编译热点和耦合点。23 条跨 crate re-export 使其成为事实上的 God Module。任何修改都需要重新编译整个 crate，增量编译收益低。

## Decision

将 `runtime-core` 拆分为 4 个职责单一的 crate，保持 `runtime-core` 作为 facade crate：

| crate | 路径 | 职责 | 从 runtime-core 提取的模块 |
|-------|------|------|--------------------------|
| `runtime-storage` | `core/runtime-storage/` | 状态持久化、文件锁、atomic write、后台任务状态 | `runtime_storage/`, `background_state/` |
| `framework-runtime` | `core/framework-runtime/` | 框架运行时核心循环、execution contract、closeout enforcement、trace I/O、pre_tool_use_guard | `closeout_enforcement.rs`, `execution_contract.rs`, `pre_tool_use_guard.rs`, `runtime_view.rs`, `trace_stream_io.rs`, `trace_attach.rs`, `trace_transport.rs`, `live_execute.rs`, `sandbox_control.rs`, `evolution_observer.rs` |
| `session-supervisor` | `core/session-supervisor/` | Worker 管理、session 生命周期、evolution_idle | `session_supervisor/`（driver/worker/runtime/process/evolution_idle） |
| `trace-runtime` | `core/trace-runtime/` | 事件追踪、trace 管道聚合入口 | `trace_runtime.rs` |

### 拆分原则

1. **逐 crate 拆分，每步全量测试**：每次提取一个 crate 后运行全量 `cargo test --workspace`，确保无回归。
2. **保持 DAG 无环**：新 crate 只依赖底层 crate（`core-state`, `framework-kernel`, `core-policy`, `routing-engine`），不反向依赖 `runtime-core`。
3. **Facade re-export**：`runtime-core` 保持 `pub use rt_storage::*` 等 re-export 以兼容下游调用者，新代码直接依赖拆分后 crate。

## Consequences

- **编译增量改善**：改 `runtime-storage` 不重编译 `session-supervisor`。
- **最大单 crate 从 38K→14K 行**（facade crate 保留 ~14K 行集成代码，提取 ~19K 行到子 crate）。
- **依赖关系更清晰**：`trace-runtime` 不需要依赖 `framework-runtime`。
- **短期代价**：拆分期间可能引入临时 re-export；需在拆分完成后清理。
- **v7 执行状态**：4 个子 crate 已全部创建并通过编译，`background_state`、`session_supervisor`、`trace_runtime` 完全提取，`framework-runtime/closeout_enforcement/execution_contract/pre_tool_use_guard` 等完全提取。`rfv_loop.rs` 和 `harness_operator_nudges.rs` 暂留在 runtime-core。

## Related

- `core/runtime-storage/` — 已提取的 storage crate
- `core/framework-runtime/` — 已提取的 framework-runtime crate
- `core/session-supervisor/` — 已提取的 session-supervisor crate
- `core/trace-runtime/` — 已提取的 trace-runtime crate
- `docs/spec/core-crates.md` §3.6 — 模块解耦架构
- `artifacts/current/roadmap-v7.md` §6 — 模块解耦 Wave
