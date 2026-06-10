---
last_verified: "2026-06-09"
depends_on:
  - spec.md
  - framework_operator_primer.md
---

# 文档索引（控制面与契约）

**叙事分工**：仓库根 `AGENTS.md` = 跨宿主执行与语言策略；[`spec.md`](spec.md) = 统一控制面、沙箱与接入契约规范（总览 + 索引，延伸子文档见 spec.md `extends` 列表）；历史迁移叙述见 git 历史与 [`MIGRATION.md`](../MIGRATION.md)。

## 推荐阅读顺序

1. [仓库根 README.md](../README.md) — 项目简介、快速开始、宿主支持
2. [ONBOARDING.md](ONBOARDING.md) — 详细安装教程、宿主配置、日常更新、FAQ
3. [framework_operator_primer.md](framework_operator_primer.md) — 使用者一页纸：宿主差异、`REVIEW_GATE` 快查、真源阅读顺序、自检 `framework doctor`
4. [AGENTS.md](../AGENTS.md) — Skill 路由、Continuity、Closeout、Execution Ladder、MCP 工具安全拦截
5. [spec.md](spec.md) — 统一规约：架构、五层模型、运行期沙箱、编排、跨宿主矩阵、接入契约、路由、Closeout、测试契约等
6. [architecture/security.md](architecture/security.md) — MCP 工具安全拦截（§0 已实现于 `hook_policy/`）；§1+ 为计划中的完整安全体系（**status: aspirational**）

## 按主题

| 主题 | 文档 |
|------|------|
| 使用者视角：宿主差异、门控快查、阅读顺序 | [framework_operator_primer.md](framework_operator_primer.md) |
| 安装教程、日常更新、FAQ | [ONBOARDING.md](ONBOARDING.md) |
| MCP 工具安全拦截（mcp-tool-safety） | [architecture/security.md §0](architecture/security.md#0-mcp-工具安全拦截层-mcp-tool-safety)、`AGENTS.md` § MCP 工具安全拦截 |
| 可选 env / closeout 详表 | [references/AGENTS_OPERATOR_SURFACE.md](references/AGENTS_OPERATOR_SURFACE.md) |
| Cursor 子代理 hook 契约（fork_context、review-lite） | [references/cursor-subagent-hook-contract.md](references/cursor-subagent-hook-contract.md) · [`configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json`](../configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json) |
| REVIEW_GATE strict vs review-lite ADR | [adr/ADR-review-gate-lite.md](adr/ADR-review-gate-lite.md) |
| Env 命名模式（非第二份默认值表） | [framework_naming_conventions.md](framework_naming_conventions.md) |
| 政策分层地图（operator profiles 依赖） | [spec.md](spec.md) |
| RFV 多轮账本（`framework_rfv_loop`）契约与 lane 模板；数理推理强度 | [spec.md](spec.md)，[references/rfv-loop/](references/rfv-loop/)（含 [math-reasoning-harness.md](references/rfv-loop/math-reasoning-harness.md)） |
| 弱模型 / 上下文预算、Token 注入路径与 harness 合成交付 | 任务 ROADMAP：`artifacts/current/<task_id>/ROADMAP.md`；见 [plans/README.md](plans/README.md) |
| Closeout 程序化门禁与 schema | [spec.md](spec.md)，`configs/framework/CLOSEOUT_RECORD_SCHEMA.json` |
| `framework_profile` 与默认面 | [spec.md](spec.md) |
| 新宿主接入 / 多宿主适配 | [spec.md](spec.md) |
| 生成物 drift / doctor 快探针 | [spec.md](spec.md)；`framework host-integration generated-artifacts-status [--skip-generator-run]` |
| 任务级 schema drift（hooks 7 事件闭集、模板 parity、REQUIREMENTS↔ROADMAP 标题） | `router-rs schema-drift contract` / `baseline` / `check`（[`schema_drift.rs`](../core/runtime-core/src/schema_drift.rs)）；验收见 [`skills/verifyx/SKILL.md`](../skills/verifyx/SKILL.md)、[`configs/framework/SCHEMA_DRIFT_HEADINGS_CONTRACT.md`](../configs/framework/SCHEMA_DRIFT_HEADINGS_CONTRACT.md) |
| Cursor Plan / My 可验收 todo | [`skills/plan-mode/SKILL.md`](../skills/plan-mode/SKILL.md)、[`skills/planx/SKILL.md`](../skills/planx/SKILL.md)；[`.cursor/rules/cursor-plan-output.mdc`](../.cursor/rules/cursor-plan-output.mdc)；索引 [plans/README.md](plans/README.md) |
| Codex 宿主投影边界 | [spec.md](spec.md)，[.codex/README.md](../.codex/README.md) |
| 运行期核心行为与沙箱统一规约 | [spec.md](spec.md) |
| Python 环境治理（uv-only，热路由 `$python-env-management`） | [`skills/python-env-management/SKILL.md`](../skills/python-env-management/SKILL.md) |
| 历史迁移、减法记录 | [`MIGRATION.md`](../MIGRATION.md)、git 历史 |
| 统一运维手册（安装 / 同步 / 备份 / 故障排查） | [operations/index.md](operations/index.md)（唯一真源；旧 `maintenance/ops-runbook.md` 已重定向） |
| **Host projection schema 校验**（闭集 MCP Key 矩阵 + 写盘前/写盘后自检 + 已知 bug；2026-06-04 opencode 故障的根因档案） | [maintenance/host-projection-schema-validity.md](maintenance/host-projection-schema-validity.md) · 引用 [spec.md](spec.md) · [framework_naming_conventions.md §MCP Key Convention](framework_naming_conventions.md#mcp-key-convention闭集禁从一个-host-抄到另一个) |
| Plans 索引（ROADMAP 真源；已删 stub 不恢复） | [plans/README.md](plans/README.md) |
| Workflow supervisor phase 工件 | `configs/framework/WORKFLOW_LANE_NOTES_SCHEMA.json` (removed) · [`skills/agent-swarm-orchestration/references/workflow-supervisor-protocol.md`](../skills/agent-swarm-orchestration/references/workflow-supervisor-protocol.md) |

## 概念与源码映射

见 [spec.md §2](spec.md#2-五层模型)。

## 已淘汰叙述（清理边界）

- **勿假设** `router-rs` 只存在于 `core/router-rs/target/release/`。根目录 `.cargo/config.toml` 可将 `target-dir` 指到 workspace 统一目录；解析以 `cargo metadata` 的 `target_directory` 为准（或 `cargo build` / `cargo run` 的输出路径）。
- **勿依赖** 旧版 `.cursor/hooks/*.sh` 脚本链：steady-state 以 [`.cursor/hooks.json`](../.cursor/hooks.json) 为准（**默认 7 事件**；见 [`docs/hosts/cursor.md`](hosts/cursor.md)）。Claude Code 为 [`.claude/settings.json`](../.claude/settings.json) **4 事件**（见 [`docs/hosts/claude.md`](hosts/claude.md)）。校验：`framework maint verify-cursor-hooks`；构建 release 见两宿主手册「内存 / release」。
- **勿将** 已删除的 `docs/history/` 或过期 plan 路径当作当前契约；steady-state 仅认本索引列出的文档与 `configs/framework/*.json`。
