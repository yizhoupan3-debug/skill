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
| 宿主投影 lifecycle/review 文案 | `configs/framework/host_projection_narrative.json`（`host-integration install` 读取） |
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

- **Python 环境（macOS）**：长期治理须显式 **`$python-env-management`**（uv-only、默认 3.12、每仓库 `uv.lock`）；operator 禁止使用 `pip`。该 skill 在冷表 manifest，不在热 `SKILL_ROUTING_RUNTIME` 30 行——环境类请求勿只靠泛化路由。
- **路由**：热入口 `skills/SKILL_ROUTING_RUNTIME.json`；只读命中项 `skill_path`；冷表见 `skills/SKILL_MANIFEST.json`。
- **可选 env / 注入 / closeout**：`docs/references/AGENTS_OPERATOR_SURFACE.md`（勿在本文重复全表）。
- **连续性摘要**：`docs/harness_architecture.md` §2–§3。

## Skill Routing

- **默认生命周期**：`/discussx` → `/planx` → `/implementx` → `/verifyx`（`implementx` **一口气**跑完 `WAVE_STATE` 全部 wave；主线程只调度）。见 [`skills/implementx/SKILL.md`](skills/implementx/SKILL.md)、[`MIGRATION.md`](MIGRATION.md)。**`/gsd-*` 与 legacy-gsd 已于 2026-05 彻底移除**；勿再使用，见 MIGRATION 退役对照表。
- **执行区**：`/implementx`、`/verifyx` + `GOAL_STATE.json`（`lifecycle_profile: my-light`；`framework_goal_drive` stdio）。`my-light` 关闭 `REVIEW_GATE` 硬拦与 spawn-first nudge（亦含 pre-execution `/discussx|planx` 与磁盘 `GOAL_STATE.lifecycle_profile: my-light`；hook 层全 suppress，skill 层 findings-only 仍适用）。
- 勿用 slug 猜路径；勿预读整个 `skills/`。

## Continuity artifacts（手动画板 only）

- 真源：`artifacts/current/<task_id>/`（见 `docs/harness_architecture.md`）；**无** hook 自动 digest / `GOAL_CONTINUE` / Stop checkpoint 默认路径。
- Goal/RFV 磁盘：`GOAL_STATE.json` / `RFV_LOOP_STATE.json`；显式 stdio：`framework_goal_drive` / `framework_rfv_loop`。
- 历史 env 名见 `docs/references/AGENTS_OPERATOR_SURFACE.md`（多数 hook 续跑 env **已无操作**）。

## Host Boundaries

| 宿主 id | 手册 | Hook 硬门控 |
|---------|------|-------------|
| `cursor` | `docs/hosts/cursor.md` | ✓ Agent 面 |
| `codex-cli` | `docs/hosts/codex-cli.md` | ✓ |
| `claude-code` | `docs/hosts/claude.md` | ✓（含 PreToolUse） |
| `claude-desktop` | `docs/hosts/claude-desktop.md` | △ MCP advisory only（无 PreToolUse） |

- **Cursor 机读短码**（宿主注入 `router-rs …` 单行）：`AG_FOLLOWUP`、`REVIEW_GATE` 等；**`GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` hook 注入已拔除**（2026-05），续跑用 `/implementx` + `framework_goal_drive` stdio 与 `artifacts/current/<task_id>/` 手动画板；**禁止**自拟仿 hook 长文。
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

- **Cursor 宿主差异**：`.cursor/rules/execution-subagent-gate.mdc`、`review-subagent-gate.mdc`（仅 lane / hook / `updateCurrentStep` 等 Cursor 硬约束）。
- **Review findings-only**（全宿主）：[`skills/code-review-deep/SKILL.md`](skills/code-review-deep/SKILL.md)（compact 信封、透镜、默认只读 findings）；深度可数 lane 见 `host_adapter_contract.md` §0.1。
- **并行 subagent / Codex 侧车 / 完整梯子**：[`docs/references/EXECUTION_LADDER.md`](docs/references/EXECUTION_LADDER.md)。

## Closeout

- 必须有验证证据或明确 blocker；聊天回复不必长篇贴 diff。
- `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 分层见 `docs/references/AGENTS_OPERATOR_SURFACE.md` 与 `docs/harness_architecture.md`。

## Goal drive

- `/implementx`、`/verifyx` + `framework_goal_drive` stdio → `artifacts/current/<task_id>/GOAL_STATE.json`；**无** hook 续跑注入；env 与手动画板见 `docs/references/AGENTS_OPERATOR_SURFACE.md`、`docs/harness_architecture.md`。
- 执行 wave / 验证：`skills/implementx/SKILL.md`、`skills/verifyx/SKILL.md`（verify 后 purge `artifacts/current/<task_id>/`，见 verifyx § Post-verify task-dir purge）。

## Manuscript / LaTeX file writes

- **Default: overwrite in place** on `.tex`, `.Rmd`, and manuscript `.md` — use StrReplace or write the same path; do **not** create `*.bak_*`, `*.bak`, or macOS-style numbered duplicates (`file 2.tex`) unless the user explicitly asks for a backup in that turn.
- **R Markdown projects**: edit **`.Rmd` only** (plus project scripts); regenerate `.tex`/`.pdf` via the repo’s `render_*.R`. Do not treat pandoc-generated `.tex` as the source of truth or leave numbered build artifacts in the report directory.
- Paper-workbench detail: [`skills/paper-workbench/references/edit-scope-gate.md`](skills/paper-workbench/references/edit-scope-gate.md) §文件写入默认.

## Git

- 未经用户明确要求不得创建分支/worktree；只读检查现有状态。
