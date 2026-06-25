---
last_verified: "2026-06-26"
scope: documentation-map
---

# 文档体系

本框架是**四宿主共用 Skill 系统**（Claude / Cursor / Codex / OpenCode），8 层运行时模型（L0–L7），核心由 Rust 实现。

## 核心文档

| 文档 | 一句话说明 |
|------|----------|
| [architecture.md](architecture.md) | L0–L7 层模型、DAG 验证矩阵、宿主隔离契约、架构原则 P1–P10 |
| [operations/index.md](operations/index.md) | 运维中枢：安装/升级、模块操作、状态管理、排障 |
| [math-reasoning-harness.md](math-reasoning-harness.md) | 数学验证工具链：三层分离架构、SymPy/Z3/Lean 后端 |
| [../AGENTS.md](../AGENTS.md) | 跨宿主代理策略（生命周期、语言、CodeGraph、行为差异） |
| [../README.md](../README.md) | 仓库快速入门：能力概览 |
| [../skills/SKILL_FRAMEWORK_PROTOCOLS.md](../skills/SKILL_FRAMEWORK_PROTOCOLS.md) | 共享最小协议层（运行时、停止规则、自审计） |
| [../skills/SKILL_ROUTING_LAYERS.md](../skills/SKILL_ROUTING_LAYERS.md) | 分层路由详解（L0–L4 各层边界、Special Gates、重路由信号） |

## 已删除文档记录

以下文件在 2026-06 文档重构中删除：

| 删前路径 | 原因 |
|----------|------|
| `docs/contributing.md` | 私有框架，无外部贡献者 |
| `docs/design-decisions.md` | 决策已稳定，记录在 git 历史 |
| `docs/hosts/handbook.md` | 跨宿主一致性已由 `RUNTIME_REGISTRY.json` 驱动 |
| `docs/migration.md` | 迁移已完成 |
| `docs/runtime-status.md` | crate 列表与 architecture.md §2 重复 |
| `docs/hosts/README.md` | 5 行无实质内容 |
| `docs/plans/doc-restructure-2026-06-24.md` | 已完成计划 |
| `docs/reports/2026-06-23-runtime-audit.md` | 被 2026-06-24 报告取代 |
| `docs/reports/2026-06-24-runtime-audit.md` | 一次性审计快照，非参考文档 |
| `docs/research/harness.md` | 大篇幅过期规约，§19 引用与实际 13 节不匹配 |
