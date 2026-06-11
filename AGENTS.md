# Agent Policy (Cross-Host)

跨宿主叙述性协议真源。宿主差异见 `AGENTS_<HOST>.md`。

**双文件注入（硬约束）**：各闭集宿主须**同时**注入仓库根 **`AGENTS.md`**（跨宿主内核）与 **`AGENTS_<HOST>.md`**（transport delta）；**禁止**合并为单文件。

**闭集宿主（2026-06）**：`codex`、`claude-code`、`antigravity`、`cursor`、`opencode` — 真源 `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`。已退役 id：`claude-desktop`、`codex-app`、`codex-cli`、`antigravity-app`、`antigravity-cli`。

## 权威分层

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议（语言、路由、Lifecycle、Closeout） | 仓库根 **`AGENTS.md`** |
| 宿主执行面差异 | `AGENTS_<HOST>.md` + 各宿主 hook/rules |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架命令 / CLI | `configs/framework/RUNTIME_REGISTRY.json` |
| hook 行为 | 各宿主 `hooks.json` + `router-rs` |

## Language

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外），自然学术中文。
- 仅当用户当轮明确要求英文时才可切换。
- **回答避免空话**；**对不确定的信息直接说明**，严禁凭空编造。

## 个人使用（最小操作面）

- **Python 环境（macOS）**：uv-only、默认 3.12、每仓库 `uv.lock`；禁止 `pip`。重度 Python/ML 任务须高频 `gc.collect()` / `torch.mps.empty_cache()`。
- **Skill Routing**：热入口 `skills/SKILL_ROUTING_RUNTIME.json`；只读命中项 `skill_path`。

## Lifecycle

- **Default lifecycle**：`/discussx` → `/planx` → `/implementx` → `/verifyx`。详见 `docs/references/EXECUTION_LADDER.md`。
- **Review**：Review findings-only。显式 `$code-review-deep` 或 review 请求仍适用。详见 `skills/code-review-deep/SKILL.md`。
- **Closeout**：`closeout_gate` / `complete` 为 advisory（`my-light`）。

## Continuity artifacts（手动画板 only）

- 真源：`artifacts/current/<task_id>/`；**无** hook 自动 digest / `GOAL_CONTINUE` / Stop checkpoint 默认路径。
- Goal/RFV 磁盘：`GOAL_STATE.json` / `RFV_LOOP_STATE.json`；显式 stdio：`framework_goal_drive` / `framework_rfv_loop`。
- **会话级作用域**：Goal state 仅作用于当前对话 session，不做跨对话持久化。

## Task Intake

- 抽取目标、约束、交付与成功标准；选最窄 owner；最小可验证 delta。
- 关键不可逆选择才问用户。

## Coding First Principles

- 五门槛：Goal / Non-goals / Existing owner / Minimal delta / Validation。
- 减法优先；禁止为不确定未来加抽象；证据收口（测试/diff/blocker）。

## Manuscript / LaTeX

- **Default: overwrite in place** — 不创建 `*.bak_*` / `*.bak` / `file 2.tex` 除非用户明确要求。
- **R Markdown**: 编辑 `.Rmd` only；不以 pandoc `.tex` 为真源。

## Git

- 未经用户明确要求不得创建分支/worktree；只读检查现有状态。
- **Worktree 隔离（硬约束）**：未经用户当轮显式批准，禁止在 git worktree 中运行或修改任何文件。

## Scientific Coding Standards

- **统一随机种子接口**：所有科研脚本暴露 `--seed` 或 `seed` 配置。
- **产出归档目录**：`output-x-seed` 下，严禁散落仓库根。
- **Checkpoint 机制**：长流程须周期性存盘并支持无损恢复。
