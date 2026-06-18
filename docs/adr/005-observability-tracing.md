---
last_verified: "2026-06-19"
depends_on:
  - ../spec.md
  - ../spec-observability-testing.md
---

# ADR-005: 可观测性 — tracing 选型及埋点策略

## Status

Accepted (2026-06-14).

## Context

框架当前零结构化日志。调试依赖 `eprintln!` / `dbg!` 和手动添加的 print 语句。随着 crate 拆分和模块边界增多，需要标准化的可观测性基础设施。

## Decision

1. **选用 `tracing` crate**：workspace 依赖 `tracing` + `tracing-subscriber`（env-filter + JSON 格式）。tracing 是 Rust 生态最成熟的可观测性框架，支持结构化 span/event，与 tokio 异步运行时可互操作。
2. **关键路径埋点**：
   - dispatch 入口/出口：`host_id`, `command`, 耗时（info）
   - hook 处理：`event_type`, `policy_decision`, 耗时（info）
   - MCP tool 调用：`tool_name`, 结果状态, 耗时（info）
   - session 生命周期：创建/销毁/worker 状态变迁（info）
   - 错误路径：Error 类型 + 上下文（error/warn）
3. **CLI 集成**：`--log-level` 参数 / `RUST_LOG` 环境变量。性能敏感路径加 `#[instrument(skip_all)]`。
4. **时机**：与 §6 模块解耦同步进行——拆分新 crate 时即设计 tracing span 边界。
5. **对现有测试的影响**：tracing 通过 `tracing-subscriber::fmt::TestWriter` 在测试中静默输出；不影响现有断言。

## Consequences

- **优势**：`RUST_LOG=debug router-rs claude hook ...` 有结构化输出；panic 时可从 span 上下文中提取诊断信息。
- **代价**：引入 tracing 依赖；热路径 span 可能引入微小开销（可通过 `#[instrument(skip_all)]` 和 level 控制）。
- **迁移**：现有 `eprintln!` 逐步迁移到 `tracing::info!` / `tracing::warn!`，非一次性完成。

## Related

- `artifacts/current/roadmap-v7.md` §14 — 可观测性 Wave
- `docs/spec/observability-testing.md` — 可观测性规约
