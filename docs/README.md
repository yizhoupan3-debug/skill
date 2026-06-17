---
last_verified: "2026-06-16"
depends_on:
  - spec.md
  - ../AGENTS.md
---

# 文档索引

**叙事分工**：仓库根 `AGENTS.md` = 跨宿主执行与语言策略；[`spec.md`](spec.md) = 统一控制面与接入契约规范。宿主差异见 [`hosts/`](hosts/)。运维见 [`operations/index.md`](operations/index.md)。

## 推荐阅读顺序

1. [仓库根 README.md](../README.md) — 项目简介、快速开始
2. [AGENTS.md](../AGENTS.md) — 跨宿主策略、Lifecycle、Closeout
3. [spec.md](spec.md) — 统一规约：架构、五层模型、沙箱、路由、Closeout
4. [operations/getting-started.md](operations/getting-started.md) — 安装教程
5. 各宿主手册 `hosts/<host>.md`

## 按主题

| 主题 | 文档 |
|------|------|
| **按代码模块** | [modules/INDEX.md](modules/INDEX.md)（各 crate 模块详解） |
| 宿主差异、hook 事件、Stop 行为 | [hosts/](hosts/)（claude / cursor / codex / opencode） |
| 安装教程 / 升级 / 多机同步 | [operations/getting-started.md](operations/getting-started.md) |
| 运维主手册（配置、排障、路径速查） | [operations/index.md](operations/index.md) |
| 统一规约（架构、契约、closeout） | [spec.md](spec.md) |
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
| RFV / 数理推理 | `spec.md` + 代码（`core/core-state/src/rfv_loop.rs`） |
| Python 环境治理 | [`skills/python-env-management/SKILL.md`](../skills/python-env-management/SKILL.md) |

## 已淘汰叙述

- **勿假设** `router-rs` 只存在于 `core/router-rs/target/release/`。解析以 `cargo metadata` 的 `target_directory` 为准。
- **勿依赖** 旧版 `.cursor/hooks/*.sh` 脚本链：steady-state 以 [`.cursor/hooks.json`](../.cursor/hooks.json) 为准。
