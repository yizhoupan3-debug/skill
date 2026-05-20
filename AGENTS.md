# Codex Agent Policy

## 权威分层（改哪里才生效）

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议（语言、路由、Continuity、Execution Ladder、Closeout） | 仓库根 `AGENTS.md` |
| Cursor 执行面默认值 | `AGENTS.md` + `.cursor/rules/*-gate.mdc`（仅宿主差异） |
| Codex 策略快照 | 磁盘 `AGENTS.md`；`codex sync` + **编译嵌入**（见下） |
| Cursor framework 叙事 | `router-rs framework host-integration install --to cursor --scope user` → `~/.cursor/rules/framework.mdc` |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架命令 / CLI | `configs/framework/RUNTIME_REGISTRY.json` |
| hook 行为 | 各宿主 `hooks.json` + `router-rs` |

**文档地图**：`docs/harness_architecture.md` · `docs/host_adapter_contract.md` · `docs/rust_contracts.md` · `docs/README.md`

### Codex：`AGENTS.md` 构建快照（策略 A）

修改本文件后同步 Codex：

```bash
cargo build --manifest-path scripts/router-rs/Cargo.toml
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
cp AGENTS.md ~/.codex/AGENTS.md   # 用户级策略与仓库对齐
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint install-codex-user-hooks --framework-root "$PWD"
```

`~/.codex/AGENTS.md` 与项目 `.codex/*` 由 sync **以仓库为准** 材料化（执行前备份）。

## Language

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外）。
- 仅当用户当轮明确要求英文回复时才可切换。

## Agent Identity

- 主代理按 MIT 博士级科研与工程专家标准约束判断与端到端执行；非履历声明。
- 各宿主（Codex / Cursor / Claude Code / Claude Desktop）适用同一质量标准。

## Root

- Codex：`CODEX_HOME`（默认 `~/.codex`）；仓库内优先 `skills/` 与 `skills/SKILL_ROUTING_RUNTIME.json`。
- **禁止**把本机绝对路径写入策略真源；用仓库根、`CODEX_HOME`、`CURSOR_HOME` 或 `$HOME` 解析。

## 个人使用（最小操作面）

- **路由**：热入口 `skills/SKILL_ROUTING_RUNTIME.json`；只读命中项 `skill_path`。
- **可选 env / 注入 / closeout**：`docs/references/AGENTS_OPERATOR_SURFACE.md`（勿在本文重复全表）。
- **连续性摘要**：`docs/harness_architecture.md` §2–§3。

## Skill Routing

- **默认生命周期（全闭集宿主）**：GSD（`/gsd-new-project` → `/gsd-discuss-phase` → `/gsd-plan-phase` → `/gsd-execute-phase` → `/gsd-verify-work` → `/gsd-ship`）。Pre-execution **禁止改产品代码**（`skills/gsd/shared/phase-boundaries.md`）。
- **执行区**（可改产品代码）：`/gsd-execute-phase`、`/gsd-verify-work`、`/gsd-ship` + `GOAL_STATE.json`（`framework_autopilot_goal` stdio）。**`/autopilot` 已退役** → 用 `/gsd-execute-phase`（见 `MIGRATION.md`）。
- 勿用 slug 猜路径；勿预读整个 `skills/`。

## Continuity artifacts

- 真源：`artifacts/current/`（见 `docs/harness_architecture.md`）。
- Goal/RFV：`GOAL_STATE.json` / `RFV_LOOP_STATE.json`；视图 `router-rs framework snapshot`。
- 续跑注入开关：见 `docs/references/AGENTS_OPERATOR_SURFACE.md`。

## Host Boundaries

| 宿主 id | 手册 | Hook 硬门控 |
|---------|------|-------------|
| `cursor` | `docs/hosts/cursor.md` | ✓ Agent 面 |
| `codex-cli` | `docs/hosts/codex-cli.md` | ✓ |
| `claude-code` | `docs/hosts/claude.md` | ✓（含 PreToolUse） |
| `claude-desktop` | `docs/hosts/claude-desktop.md` | △ MCP advisory only（无 PreToolUse） |

- **Cursor 机读短码**（宿主注入 `router-rs …` 单行）：`AG_FOLLOWUP`、`REVIEW_GATE`、`GSD_GOAL_CONTINUE`、`RFV_LOOP_CONTINUE` 等；**禁止**自拟仿 hook 长文。
- **Cursor `updateCurrentStep`**：禁止空载荷；见 `execution-subagent-gate.mdc`。
- 路由问题 → runtime；hook 问题 → 对应 `hooks.json`。

## Task Intake

- 抽取目标、约束、交付与成功标准；选最窄 owner；最小可验证 delta。
- 关键不可逆选择才问用户。

## Coding First Principles

- 五门槛：Goal / Non-goals / Existing owner / Minimal delta / Validation。
- 减法优先；禁止为不确定未来加抽象；证据收口（测试/diff/blocker）。

## Knowledge Hygiene

- `AGENTS.md` 是地图，不是百科；真源在 runtime、skill、`docs/`、artifacts。
- 改 policy 前查路径是否仍由 runtime 决定。

## Execution Ladder

- **Cursor**：`execution-subagent-gate.mdc` / `review-subagent-gate.mdc` 为执行面真源。
- **Review 与执行解耦**：纯 review 回合**只找 findings**，禁止默认改代码/提交/推进 execute；**纯 review 回合除外**于 GSD 默认链。深度 lane 见 `host_adapter_contract.md` §0.1。
- **并行 subagent**：可拆独立子问题时默认 3–5 个 `fork_context=false` lane；**主线程始终负责上下文判断**、集成与最终验证。
- **Codex / 无 Cursor gate**：仅显式 subagent 或 GSD 执行区命令（`/gsd-execute-phase` 等）时 **bounded sidecar admission**。
- **完整规则**：`docs/references/EXECUTION_LADDER.md`。

## Closeout

- 必须有验证证据或明确 blocker；聊天回复不必长篇贴 diff。
- `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 分层见 `docs/references/AGENTS_OPERATOR_SURFACE.md` 与 `docs/harness_architecture.md`。

## GSD goal drive

- `/gsd-execute-phase`（及 verify/ship）→ `framework_autopilot_goal` → `artifacts/current/<task_id>/GOAL_STATE.json`；`drive_until_done` 时 Cursor Stop 注入 **`GSD_GOAL_CONTINUE`**（关闭：`ROUTER_RS_GSD_GOAL_CONTINUE_HOOK=0`，兼容 `ROUTER_RS_AUTOPILOT_DRIVE_HOOK`）。
- 宏任务用地平线切片 + `artifacts/current` 冷启动摘要。
- 细节：`skills/gsd/execute-phase/SKILL.md`、`docs/harness_architecture.md`。

## Git

- 未经用户明确要求不得创建分支/worktree；只读检查现有状态。
