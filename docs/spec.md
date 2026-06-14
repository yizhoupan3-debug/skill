---
last_verified: "2026-06-12"
version: "unified-v7"
---

# 框架统一规约 (Unified Framework Specification)

> 本文件是框架**总览规约**，覆盖架构总览、设计原则与五层模型。
> 各子系统详细规约见下方 `extends` 延伸文档（各自在其领域内为真源）。
> 实施路线图见 `artifacts/current/roadmap-v7.md`。

---

## 目录

1. [架构总览](#1-架构总览)
2. [五层模型](#2-五层模型)
3. [Core Crates](spec-core-crates.md)
4. [运行期沙箱契约](spec-sandbox-contract.md)
5. [多 Agent 编排契约](spec-multi-agent.md)
6. [跨宿主统一矩阵](spec-host-matrix.md) + [宿主接入契约](spec-host-matrix.md#7-宿主接入契约)
7. [路由与插件契约](spec-routing-plugin.md)
8. [运行时子系统](spec-runtime-subsystems.md) + [Hook 系统](spec-runtime-subsystems.md#10-hook-系统) + [传输与持久化](spec-runtime-subsystems.md#13-传输与持久化)
9. [安全守卫](spec-security-lifecycle.md) + [Closeout 与生命周期](spec-security-lifecycle.md#12-closeout-与生命周期)
10. [辅助模块](spec-auxiliary.md)
11. [可观测性](spec-observability-testing.md) + [存储压缩](spec-observability-testing.md#16-存储压缩) + [测试契约](spec-observability-testing.md#17-测试契约) + [Schema 索引](spec-observability-testing.md#18-schema-索引)

---

## 1. 架构总览

### 1.1 Crate 拓扑 (v7)

```
runtime-core (~38K LOC)       ← 核心生命周期/存储/trace/closeout
├── session_supervisor/       ← 工作进程管理（driver/worker/runtime）
├── framework_runtime/        ← MCP stdio harness + hooks 注册表
├── host_integration/         ← projection 安装/移除（re-export shim → host-projection）
├── closeout_enforcement.rs   ← hard/soft blocker 分级
├── 396 tests / 13 ignored
└── features: codegraph, host-{cursor,claude-code,codex,opencode}

host-projection (~34K LOC)     ← 宿主适配层（从 runtime-core 迁出）
├── hosts/                    ← 4 宿主 provider + hooks 实现
│   ├── claude_code_hooks.rs  ← PreToolUse/PostToolUse/Stop/SubagentStart-Stop
│   ├── codex_hooks/          ← Codex native hooks (5K LOC)
│   ├── cursor_hooks/         ← Cursor agent hooks
│   ├── opencode_agent.rs     ← OpenCode MCP stdio
├── host_integration/         ← projection 安装/移除逻辑
├── hooks.rs                  ← 函数指针注册表（82 个 OnceLock slots）
└── 433 tests

router-rs (~558 LOC src + 6K tests) ← CLI + 集成测试
├── CLI (clap): framework/host-integration/schema-drift
├── tests/ (275 passed)
└── features: codegraph, host-*

routing-engine (~8K LOC)       ← 路由评分/信号缓存
├── route/{eval,scoring,signal_cache,text}
└── 63 tests / 12 ignored

core-state (~7K LOC)           ← Goal/RFV/Evidence/TaskState
├── state_manager.rs, task_state.rs, step_ledger.rs
└── 82 tests

core-policy (~4K LOC)          ← Hook 策略/review gate/注册表
├── review_gate_engine.rs, hook_review_disk_state.rs
└── 含 186 条正则规则

codegraph-rs (~2.5K LOC)       ← 代码图谱（FTS5 + tree-sitter）
├── parser/{rust,typescript,python,go}
├── db/{schema,node_ops,edge_ops,fts_ops}
└── 64 tests (caller bug 已修复)

evolution-rs (~1.8K LOC)       ← 技能进化审计
├── 13 tests
└── ⚠️ 测试密度偏低

autoresearch-rs (~5.4K LOC)    ← 研究工作区控制平面
├── 单文件 main.rs（待拆分）
└── 🔴 仅 2 测试（严重不足）

browser-mcp (~4.8K LOC)        ← 浏览器 MCP + session supervisor
├── session_launch/list/inspect/terminate MCP tools
├── browser_* MCP tools
└── 8 tests（⚠️ 偏低）

framework-profile（已并入 framework-kernel）← 运行时配置 profile

rust_tools/ (6 活跃 MCP crates)
├── pdf_tool_rs (mcp-pdf)           ├── citation_tool_rs (mcp-citation)
├── financial_data_rs (mcp-financial) ├── gh_source_gate_rs (mcp-gh-source-gate)
├── ooxml_parser_rs (mcp-ooxml)     └── pptx_tool_rs (mcp-pptx)
└── 各自 lib.rs + mcp/mod.rs + mcp_main.rs binary
```

> 各 crate 的详细模块拆解、pub API 和技术债见 [`docs/modules/`](modules/) 下对应文档。

| 原则 | 含义 |
|------|------|
| **单一权威真源** | `RUNTIME_REGISTRY.json` 为宿主闭集唯一权威 |
| **L4/L5 解耦** | 宿主差异仅存于 L4 适配壳（host-projection） |
| **二元编排** | 仅 `subagent` + `workflow`；team 已废弃 |
| **纯 Rust 隔离** | PID + SQLite |
| **配置驱动接入** | 新宿主 ≤ 1 天（5 文件：provider + AGENTS + docs + feature + registry） |
| **Fail-closed** | 未知均默认拒绝 |
| **函数指针注册表** | hooks 通过 OnceLock 函数指针注册（非 trait），82 个 slots |
| **MCP 统一** | 4 个 MCP server 四宿主统一注册 |
| **用户级配置** | MCP/hooks/settings 配置只放用户级（~/.config/），不在项目目录 |

### 1.3 依赖关系约束 (v7 DAG)

```
router-rs → runtime-core → host-projection → core-state
                         → core-policy
                         → routing-engine
                         → framework-kernel (含 framework_profile)
                         → codegraph-rs (optional feature)
                         → browser-mcp (optional)
```

- host-projection 包含所有宿主 hooks 实现（从 runtime-core 迁出）
- runtime-core 通过 re-export shim 向后兼容 `framework_kernel::hosts::*`
- B0 core crates 不依赖 `router-rs`
- host 特有逻辑禁止出现在 B0 core crates 中

---

## 2. 五层模型

| 层 | 职责 | 允许 | 禁止 |
|----|------|------|------|
| **L0** | Skill 路由 | 路由信号、评分、准入 | 直接执行工具 |
| **L1** | Skill 契约 | verify_commands、拒因枚举 | 第二套连续性目录 |
| **L2** | 连续性工件 | artifacts/current/、EVIDENCE_INDEX schema | 与 L2 schema 冲突的并行真源 |
| **L3** | CLI 行为 | 门控、证据追加、closeout | 宿主 shell 复制 L3 决策 |
| **L4** | 宿主适配壳 | argv/stdin/超时/路径转发 | 长段策略 prose |
| **L5** | 宿主策略 | .mdc、AGENTS* 投影 | 与 L2 冲突的并行真源 |

---

## extends 延伸文档

| 文档 | 覆盖章节 |
|------|---------|
| [spec-core-crates.md](spec-core-crates.md) | §3 Core Crates |
| [spec-sandbox-contract.md](spec-sandbox-contract.md) | §4 运行期沙箱契约 |
| [spec-multi-agent.md](spec-multi-agent.md) | §5 多 Agent 编排契约 |
| [spec-host-matrix.md](spec-host-matrix.md) | §6 跨宿主统一矩阵 + §7 宿主接入契约 |
| [spec-routing-plugin.md](spec-routing-plugin.md) | §8 路由与插件契约 |
| [spec-runtime-subsystems.md](spec-runtime-subsystems.md) | §9 运行时子系统 + §10 Hook 系统 + §13 传输与持久化 |
| [spec-security-lifecycle.md](spec-security-lifecycle.md) | §11 安全守卫 + §12 Closeout 与生命周期 |
| [spec-auxiliary.md](spec-auxiliary.md) | §14 辅助模块 |
| [spec-observability-testing.md](spec-observability-testing.md) | §15 可观测性 + §16 存储压缩 + §17 测试契约 + §18 Schema 索引 |

---

## 契约漂移规则

本规约中的机器可读 Schema、状态流转图、指标定义是开发和测试的第一断言断点。

涉及上述配置规则的代码变更，**必须以"文档先行"形式首先修改本文件**，然后进行 Rust 实现与测试回归。

禁止：
- 从一个宿主复制 adapter 模板到另一个宿主而不改 key 名
- 测试夹具用"预期 bug 形态"反向锁死 bug
- 新增宿主/模块时不更新本文件
