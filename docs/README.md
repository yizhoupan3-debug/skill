---
last_verified: "2026-06-22"
depends_on:
  - spec.md
  - ../AGENTS.md
---

# 文档索引

**叙事分工**：仓库根 `AGENTS.md` = 跨宿主执行与语言策略；[`spec.md`](spec.md) = 统一控制面与接入契约规范。宿主差异见 [`hosts/`](hosts/)（[共通](hosts/_common.md) + [hook 宿主](hosts/hook-hosts.md) + [OpenCode](hosts/opencode.md)）。运维见 [`operations/index.md`](operations/index.md)。

## 推荐阅读顺序

1. [仓库根 README.md](../README.md) — 项目简介、快速开始
2. [AGENTS.md](../AGENTS.md) — 跨宿主策略、Lifecycle、Closeout
3. [spec.md](spec.md) — 统一规约：架构、五层模型、沙箱、路由、Closeout
4. [operations/getting-started.md](operations/getting-started.md) — 安装教程
5. 宿主手册：共通 [`hosts/_common.md`](hosts/_common.md)、差异 [`hosts/hook-hosts.md`](hosts/hook-hosts.md) / [`hosts/opencode.md`](hosts/opencode.md)

## 按主题

| 主题 | 文档 |
|------|------|
| **按代码模块** | [modules/INDEX.md](modules/INDEX.md)（各 crate 模块详解） |
| 宿主差异、hook 事件、Stop 行为 | [hosts/](hosts/)：共通 [`_common.md`](hosts/_common.md)，hook 宿主 [`hook-hosts.md`](hosts/hook-hosts.md)（Claude / Cursor / Codex），OpenCode [`opencode.md`](hosts/opencode.md) |
| 安装教程 / 升级 / 多机同步 | [operations/getting-started.md](operations/getting-started.md) |
| 运维主手册（配置、排障、路径速查） | [operations/index.md](operations/index.md) |
| 统一规约（架构、契约、closeout） | [spec.md](spec.md) + [spec/](spec/) 子规约 |
| 子系统详细规约 | [spec/](spec/)（core-crates / multi-agent / host-matrix 等） |
| Env 命名模式 | [framework_naming_conventions.md](framework_naming_conventions.md) |
| Profile 契约 | [framework_profile_contract.md](framework_profile_contract.md) |
| Git 规范 | [git_hygiene.md](git_hygiene.md) |
| Cursor 子代理 hook 契约 | [references/cursor-subagent-hook-contract.md](references/cursor-subagent-hook-contract.md) |
| REVIEW_GATE ADR | [adr/ADR-review-gate-lite.md](adr/ADR-review-gate-lite.md) |
| Review 流程 | [references/review-protocol.md](references/review-protocol.md) |
| Office CLI 工具 | [references/office-document-clis.md](references/office-document-clis.md) |
| Worktree 指南 | [`git_hygiene.md`](git_hygiene.md) §主分支切片 |
| 安全策略（SSRF、MCP） | [operations/security.md](operations/security.md) |
| 备份 / 恢复 | [operations/backup-restore.md](operations/backup-restore.md) |
| 历史迁移 | [`MIGRATION.md`](../MIGRATION.md)、git 历史 |
| Quality Gate / 数理推理 | `spec.md` + 代码（`core/runtime-core/src/quality_gate.rs`） |
| 跨宿主架构 | [cross-host-architecture.md](cross-host-architecture.md) |
| Hook 锁层级 | [references/hook_lock_order.md](references/hook_lock_order.md) |
| ADR 索引 (002-009) | [`002`](adr/002-mcp-native-opencode.md) MCP 原生接入 / [`003`](adr/003-runtime-core-split.md) Runtime Core 拆分 / [`004`](adr/004-error-handling-strategy.md) 错误处理策略 / [`005`](adr/005-observability-tracing.md) 可观测性 Tracing / [`006`](adr/006-six-layer-architecture.md) 六层架构 / [`007`](adr/007-dual-exit-gates.md) 双 Exit Gate / [`008`](adr/008-cross-host-consistency.md) 跨宿主一致性 / [`009`](adr/009-doc-versioning-strategy.md) 文档版本策略 |
| Python 环境治理 | [`skills/python-env-management/SKILL.md`](../skills/python-env-management/SKILL.md) |

## 已淘汰叙述

- **勿假设** `router-rs` 只存在于 `core/router-rs/target/release/`。解析以 `cargo metadata` 的 `target_directory` 为准。
- **勿依赖** 旧版 `.cursor/hooks/*.sh` 脚本链：steady-state 以 [`.cursor/hooks.json`](../.cursor/hooks.json) 为准。
