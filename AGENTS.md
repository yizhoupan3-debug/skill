# Agent Policy (Cross-Host)

跨宿主叙述性协议真源。宿主差异见 `AGENTS_CURSOR.md`、`AGENTS_CODEX.md`、`AGENTS_CLAUDE.md`、`AGENTS_ANTIGRAVITY.md`、`AGENTS_OPENCODE.md`。

**双文件注入（硬约束）**：各闭集宿主须**同时**注入仓库根 **`AGENTS.md`**（跨宿主内核）与 **`AGENTS_<HOST>.md`**（transport delta）；**禁止**合并为单文件。Codex 为编译期 concat embed；其余宿主为仓库根双文件 + 各宿主投影（framework rule / MCP / hook）。路径见各 delta 与 [`docs/hosts/`](docs/hosts/)。

**闭集宿主（2026-06）**：`codex`、`claude-code`、`antigravity`、`cursor`、`opencode` — 真源 `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`。已退役 id（勿再 `install --to` 或写入 registry）：`claude-desktop`、`codex-app`、`codex-cli`、`antigravity-app`、`antigravity-cli`。迁移见 [`MIGRATION.md`](MIGRATION.md) §闭集宿主收敛（2026-06）。

## 权威分层（改哪里才生效）

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议（语言、路由、Continuity、Execution Ladder、Closeout） | 仓库根 **`AGENTS.md`**（本文件） |
| 宿主执行面差异 | `AGENTS_<HOST>.md` + 各宿主 hook/rules |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架命令 / CLI | `configs/framework/RUNTIME_REGISTRY.json` |
| 宿主投影 lifecycle/review 文案 | `configs/framework/host_projection_narrative.json` |
| hook 行为 | 各宿主 `hooks.json` + `router-rs`；review gate **Claude Code canonical**（[`host_adapter_contract.md`](docs/host_adapter_contract.md) §0.1） |

**文档地图**：[`docs/harness_architecture/index.md`](docs/harness_architecture/index.md) · [`docs/host_adapter_contract.md`](docs/host_adapter_contract.md) · [`docs/rust_contracts.md`](docs/rust_contracts.md) · [`docs/README.md`](docs/README.md)

## Language

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外），且使用自然的学术中文表达，避免翻译腔。
- 仅当用户当轮明确要求英文回复时才可切换。
- **回答避免空话**，直接给出具体的、可执行的建议；**对不确定的信息直接说明**，严禁凭空编造。

## Agent Identity

- 主代理按**严谨的科研学者与系统工程专家**标准约束判断与端到端执行；非履历声明。
- 各宿主适用同一高质量执行与质量标准。

## Root

- 仓库内优先 `skills/` 与 `skills/SKILL_ROUTING_RUNTIME.json`。
- **禁止**把本机绝对路径写入策略真源；用仓库根、宿主 home 环境变量或 `$HOME` 解析。

## 个人使用（最小操作面）

- **Python 环境（macOS）**：长期治理须显式使用项目统一的 Python 环境（uv-only、默认 3.12、每仓库 `uv.lock`）；禁止使用全局 `pip`。环境类请求勿只靠泛化路由。由于 macOS 采用统一内存设计，在运行中长周期或重度 Python/ML 任务时，必须以更高频度进行主动的内存回收（如循环内调用 `gc.collect()` 或释放 `torch.mps.empty_cache()`），从源头上规避 swap 膨胀并降低运行期与静态的内存开销。
- **路由**：热入口 `skills/SKILL_ROUTING_RUNTIME.json`；只读命中项 `skill_path`；冷表见 `skills/SKILL_MANIFEST.json`。
- **可选 env / 注入 / closeout**：[`docs/references/AGENTS_OPERATOR_SURFACE.md`](docs/references/AGENTS_OPERATOR_SURFACE.md)（勿在本文重复全表）。
- **连续性摘要**：[`docs/harness_architecture/02-data-flows.md`](docs/harness_architecture/02-data-flows.md) §2–§3。

## Skill Routing

- **默认生命周期**：`/discussx` → `/planx` → `/implementx` → `/verifyx`（`implementx` 一口气跑完 `WAVE_STATE` 全部 wave；主线程只调度）。见 [`skills/implementx/SKILL.md`](skills/implementx/SKILL.md)、[`MIGRATION.md`](MIGRATION.md)。
- **执行区**：`/implementx`、`/verifyx` + `GOAL_STATE.json`（`lifecycle_profile: my-light` 由 **My 入口斜杠**或磁盘 `GOAL_STATE.lifecycle_profile` 触发；`framework_goal_drive` stdio / `goal_state_manage` MCP）。Hook review：**Claude Code canonical** 清门（`independent_reviewer_seen` 或 override）；**全宿主** Stop 上 `REVIEW_GATE` **advisory-only**（不硬拦 Stop；见 [`docs/host_adapter_contract.md`](docs/host_adapter_contract.md) §0.1）。`my-light` 另 **suppress** review nudge 与 spawn-first（亦含 pre-execution `/discussx|/planx` 与磁盘 profile；hook 层全 suppress，skill 层 findings-only 仍适用）。
- 勿用 slug 猜路径；勿预读整个 `skills/`。

## Continuity artifacts（手动画板 only）

- 真源：`artifacts/current/<task_id>/`（见 [`docs/harness_architecture/02-data-flows.md`](docs/harness_architecture/02-data-flows.md)）；**无** hook 自动 digest / `GOAL_CONTINUE` / Stop checkpoint 默认路径。
- Goal/RFV 磁盘：`GOAL_STATE.json` / `RFV_LOOP_STATE.json`；显式 stdio：`framework_goal_drive` / `framework_rfv_loop`。
- **会话级作用域**：Goal state 仅作用于当前对话 session，不做跨对话持久化。新 session 首次 `goal_state_manage operation=start` 创建新 state，不读取旧 session 残留。跨 session 延续需用户显式 `resume`。详见 roadmap v5 §4.9。
- 历史 env 名见 [`docs/references/AGENTS_OPERATOR_SURFACE.md`](docs/references/AGENTS_OPERATOR_SURFACE.md)。

## Task Intake

- 抽取目标、约束、交付与成功标准；选最窄 owner；最小可验证 delta。
- 关键不可逆选择才问用户。

## Coding First Principles

- 五门槛：Goal / Non-goals / Existing owner / Minimal delta / Validation。
- 减法优先；禁止为不确定未来加抽象；证据收口（测试/diff/blocker）。

## Knowledge Hygiene

- `AGENTS.md` 与各 `AGENTS_*.md` 是地图，不是百科；真源在 runtime、skill、`docs/`、artifacts。
- 改 policy 前查路径是否仍由 runtime 决定。

## Execution Ladder

- 完整规则：[`docs/references/EXECUTION_LADDER.md`](docs/references/EXECUTION_LADDER.md)。
- **Review findings-only**：[`skills/code-review-deep/SKILL.md`](skills/code-review-deep/SKILL.md)（compact 信封、透镜、默认只读 findings）；深度可数 lane 与 Stop **advisory-only** `REVIEW_GATE`（Claude canonical 清门）见 [`docs/host_adapter_contract.md`](docs/host_adapter_contract.md) §0.1（closeout 硬门禁分层见 Closeout 节）。
- 宿主 hook **transport** 差异见各 `AGENTS_<HOST>.md` 与 `.cursor/rules/*-gate.mdc`（仅 Cursor 有 gate 规则文件）；清门语义不分宿主。

## Closeout

- 必须有验证证据或明确 blocker；聊天回复不必长篇贴 diff。
- `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 分层见 [`docs/references/AGENTS_OPERATOR_SURFACE.md`](docs/references/AGENTS_OPERATOR_SURFACE.md) 与 [`docs/harness_architecture/04-closeout-and-depth.md`](docs/harness_architecture/04-closeout-and-depth.md)。

## Goal drive

- `/implementx`、`/verifyx` + MCP `goal_state_manage`（Claude Desktop / Antigravity / OpenCode）或 `framework_goal_drive` stdio（CLI / Cursor / Codex）→ `artifacts/current/<task_id>/GOAL_STATE.json`；**无** hook 续跑注入；env 与手动画板见 [`docs/references/AGENTS_OPERATOR_SURFACE.md`](docs/references/AGENTS_OPERATOR_SURFACE.md)、[`docs/harness_architecture/index.md`](docs/harness_architecture/index.md)。
- 执行 wave / 验证：[`skills/implementx/SKILL.md`](skills/implementx/SKILL.md)、[`skills/verifyx/SKILL.md`](skills/verifyx/SKILL.md)（verify 后 purge `artifacts/current/<task_id>/`，见 verifyx § Post-verify task-dir purge）。

## Manuscript / LaTeX file writes

- **Default: overwrite in place** on `.tex`, `.Rmd`, and manuscript `.md` — use StrReplace or write the same path; do **not** create `*.bak_*`, `*.bak`, or macOS-style numbered duplicates (`file 2.tex`) unless the user explicitly asks for a backup in that turn.
- **R Markdown projects**: edit **`.Rmd` only** (plus project scripts); regenerate `.tex`/`.pdf` via the repo’s `render_*.R`. Do not treat pandoc-generated `.tex` as the source of truth or leave numbered build artifacts in the report directory.
- **Manuscript prose quality chain** (润色/写作/中英文分场景): front door [`skills/paper-workbench/references/prose-chain-contract.md`](skills/paper-workbench/references/prose-chain-contract.md); gate [`skills/paper-workbench/references/prose-quality-gate.md`](skills/paper-workbench/references/prose-quality-gate.md). **`language_register` and `writing_mode: ladder-full` are inferred automatically** — do not wait for user tokens.
- Paper-workbench detail: [`skills/paper-workbench/references/edit-scope-gate.md`](skills/paper-workbench/references/edit-scope-gate.md) §文件写入默认.

## Git

- 未经用户明确要求不得创建分支/worktree；只读检查现有状态。
- **Worktree 隔离（硬约束）**：未经用户当轮显式批准，**禁止**在 git worktree 隔离环境中运行或修改任何文件。包括但不限于：`Agent(isolation="worktree")`、`EnterWorktree`、`git worktree add` 以及任何等效隔离机制。涉及文件写入或执行的操作必须在主工作目录中进行，除非用户在当前会话中明确指定了 worktree。

## Scientific Coding Standards (科研代码与实验规范)

- **统一的随机种子接口 (`x seed`)**：所有科研脚本、数值仿真及机器学习逻辑须暴露一致的随机种子设置接口（`--seed` 或配置 `seed`），确保可重复性。
- **指定的产出归档目录 (`output-x-seed`)**：实验产出须收拢在 `output-x-seed` 目录下（或含 seed 的子目录），严禁散落仓库根或其它临时路径。
- **全面启用 Checkpoint 机制**：长流程任务须周期性存盘并支持从 checkpoint 无损恢复（Resume/State Load）。
