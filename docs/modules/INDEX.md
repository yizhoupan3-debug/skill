---
last_verified: "2026-06-19"
---

# 模块文档索引

按代码 crate 组织，每个模块文档记录：职责、核心功能、pub 接口、依赖关系、近期变更、已知技术债。

## Crate 分层

| 层级 | Crate | 行数 | 文档 |
|------|-------|------|------|
| B0 | `framework-kernel` | ~3K | [framework-kernel.md](framework-kernel.md) |
| B0 | `core-state` | ~7K | （薄代理，见 runtime-core） |
| B1 | `runtime-core` (facade) | ~20K | [runtime-core.md](runtime-core.md) |
| B1 | `runtime-core-contracts` | ~3K | 本页 §runtime-core-contracts |
| B1 | `core-policy` | ~4K | [core-policy.md](core-policy.md) |
| B1 | `framework-runtime` | ~9K | (extracted from runtime-core) |
| B1 | `session-supervisor` | ~3K | (extracted from runtime-core) |
| B1 | `runtime-storage` | ~5K | (extracted from runtime-core) |
| B1 | `trace-runtime` | ~1K | (extracted from runtime-core) |
| B1 | `loop-engine` | ~2K | (extracted from runtime-core) |
| B2 | `host-projection` | ~37K | [host-projection.md](host-projection.md) |
| B2 | `browser-mcp` | ~5K | [browser-mcp.md](browser-mcp.md) |

## 阅读路径

1. **快速了解架构**：[framework-kernel.md](framework-kernel.md) → [runtime-core.md](runtime-core.md) → [host-projection.md](host-projection.md)
2. **深入 hook 系统**：[host-projection.md#hosts-子模块详解](host-projection.md#hosts-子模块详解) → [core-policy.md](core-policy.md)
3. **框架运行时核心**：[runtime-core.md#framework_runtime-子模块详解](runtime-core.md#framework_runtime-子模块详解)
4. **浏览器自动化**：[browser-mcp.md](browser-mcp.md)

## runtime-core-contracts

v7.1 从 `runtime-core::contracts` 提取的独立 crate（lib 名 `rt_core_contracts`）。

**职责**：纯数据契约模块 —— 框架技能枚举、hook 观察规则、session 调用追踪、web fetch 守卫等。不依赖 runtime-core 内部实现。

**注意**：`hook_timing`、`review_gate`、`task_command` 因深层循环依赖留守 runtime-core 内。

**依赖**：core-state, core-policy, framework-kernel, framework-runtime, host-projection, routing-engine

**测试**：90 例，33 insta snapshot 中 9 例位于本 crate。

**近期变更**：v7.1 从 runtime-core 提取（2026-06-18）。

## loop-engine

v6.5 从 `runtime-core` 提取的独立 crate。

**职责**：loop 运行态 —— 阶段机、kill switch、safety level、closeout 聚合、run 生命周期管理。

**依赖**：core-state, framework-kernel

**测试**：44 例。

**近期变更**：v7.1 大函数拆分 + doc 补齐（2026-06-19）。
