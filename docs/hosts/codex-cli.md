# Codex CLI 宿主操作手册

**闭集 id**：`codex-cli` · **传输**：codex-hooks · **权威**：`RUNTIME_REGISTRY.json` → `host_projections.codex-cli`

## 代理身份与画风 (Agent Identity & Style)

- **核心身份**：主代理定位为 **MIT 博士级科研与顶级工程专家**，具备端到端、高难度的科研与复杂系统工程执行能力。
- **回复画风**：严格保持 **专业、严谨、客观、谦逊** 的学术与工程专家风格，避免夸大、浮躁或过度礼貌。
- **回复语言**：默认面向用户的回复必须使用 **简体中文**（代码、路径、命令或第三方原文除外），且使用自然的学术中文表达，避免翻译腔。仅当用户在当轮中明确要求使用英文时，方可切换至英文。
- **回答准则**：回答避免空话，直接给出具体的、可执行的建议；对不确定的信息直接说明，严禁凭空编造。

## 能力边界与 Harness 入口 (Capabilities & Harness Entrypoints)

在 Codex CLI 环境下，Harness 和任务管理的核心入口与工作区定义如下：

- **Harness 核心入口**：
  - **任务推进及推进控制**：利用 `/implementx` 和 `/verifyx` 指令，配合 `framework_goal_drive` stdio 推进宏任务。
  - **任务状态治理**：使用 `goal_state_manage` 进行状态的启动与收尾管理。
- **工作区及状态产物**：
  - 核心状态与任务物化存放在 `artifacts/current/<task_id>/` 目录下。
  - 主要包含任务状态文件 `GOAL_STATE.json` 以及交互/审核状态文件 `RFV_LOOP_STATE.json`。
- **门控与审稿机制**：
  - **非 my-light** 时，`UserPromptSubmit` 可注入 `spawn_first_nudge` 触发审稿引导；**`lifecycle_profile: my-light`**（默认 My 链）下 **不**注入 spawn-first，Stop 上 **`REVIEW_GATE` 硬拦关闭**，仍可用 findings-only review。深度 Review 规范见 [`skills/code-review-deep/SKILL.md`](../../skills/code-review-deep/SKILL.md)。
  - 通过 `Stop` 钩子处理 `REVIEW_GATE` 阶段判断与收尾验证。

## Hook 事件矩阵

细则见 [`harness_architecture.md`](../harness_architecture.md) §3、「主数据流」与 `.codex/hooks.json`。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| PostTool 证据、`CODEX_REVIEW_GATE` | 配置项指向 `router-rs codex hook …` | `codex hook`（[`codex_hooks/mod.rs`](../../core/router-rs/src/hosts/codex_hooks/mod.rs)） | **opt-in** `EVIDENCE_INDEX` 追加；SessionStart **不**注入 continuity digest / `GOAL_CONTINUE`；wave-2：PostTool 深度 lane → `phase≥2`，Stop compact/rg_clear 清门；`ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` 关闭硬拦 |
| **Paper prose L4** | `UserPromptSubmit` 写作/润色语境 | `paper_prose_hook.rs` | `PAPER_PROSE_QUALITY_HOOK`（**默认开**：`ROUTER_RS_CODEX_PAPER_PROSE_HOOK`）；`ROUTER_RS_CODEX_PAPER_ADVERSARIAL_HOOK=1` opt-in |
| **Codex hook stdout** | 任一 hook 进程退出 0 | `dispatch_codex_command` → `codex_hook_stdout_payload` | **始终**打印单行紧凑 JSON；无附带输出时为 **`{}`** |
| **Codex Stop × `.codex/hook-state`** | Stop 事件 | `handle_codex_stop` | 状态文件缺失：不据此拦截；状态不可读（损坏 JSON / IO）：**fail-closed**，`followup_message` 含 `CODEX_HOOK_STATE_UNREADABLE` |
| 宿主入口对齐 | `router-rs codex sync` | shared `host_entrypoint_sync` + Codex provider | 生成 `.codex/hooks.json`、**`AGENTS_CODEX.md`**、`.codex/README.md` 及 **`host_entrypoints_sync_manifest`**；跨宿主内核 **[`AGENTS.md`](../../AGENTS.md)** 人工维护、不由 sync 覆盖 |

**统一原则**：宿主配置命令须 **短命 + 超时**；语义在 Rust，不在 shell 脚本分支。

## 安装与文件分布 (Installation & Scope)

- **文件 Scope 配置**：
  - **[`AGENTS.md`](../../AGENTS.md)**（跨宿主内核，人工维护）、**[`AGENTS_CODEX.md`](../../AGENTS_CODEX.md)**（Codex delta，sync 材料化）、**`.codex/hooks.json`**：**Project**，位于仓库根或 `.codex/`。
  - **Framework prompt 快照**：**Project**，路径为 `.codex/prompts/framework.md`。
  - **全局 skill surface**：**User**，路径为 `$CODEX_HOME/skills`，指向仓库中 `artifacts/codex-skill-surface/skills`。
- **同步与安装命令**：
  当修改了 `router-rs` 嵌入的 AGENTS 文本、Codex hook 模板或需重新材料化时，运行以下同步命令：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"
  ```

## Skill 存放与路由 (Skills & Routing)

- **Skill 存放位置**：所有自定义及框架内置 Skill 均统一放置在项目根目录的 `skills/` 文件夹中。
- **Skill 路由机制**：
  - **热路由入口**：`skills/SKILL_ROUTING_RUNTIME.json` 为当前的运行期热入口，只读命中项的 `skill_path`。
  - **冷表清单**：`skills/SKILL_MANIFEST.json` 作为全部已注册 Skill 的完整冷表清单。
  - **查找原则**：严禁通过模糊匹配或文件名盲目猜测路径，也不得无故预读整个 `skills/` 目录，必须通过路由表进行精确匹配。

## 默认生命周期 (Lifecycle)

任务流严格遵循以下标准的渐进生命周期：

$$\text{Discuss} \longrightarrow \text{Plan} \longrightarrow \text{Implement} \longrightarrow \text{Verify}$$

1. **`/discussx`**：初始需求对齐与技术预研阶段。
2. **`/planx`**：规划阶段，生成或更新 `artifacts/current/<task_id>/ROADMAP.md` 与 `WAVE_STATE.json`，明确 minimal delta 与 verification plan，并报用户审批。
3. **`/implementx`**：执行阶段。进入执行区时，需配合 `framework_goal_drive` stdio 以及物化的 `GOAL_STATE.json`。主线程主要负责调度，**一口气**跑完 `WAVE_STATE` 全部的执行 wave。
   - **执行 Profile 调优**：默认使用 `lifecycle_profile: my-light`（关闭 Stop 上 `REVIEW_GATE` 硬拦与 UPS spawn-first nudge；findings-only review 仍可用）。
4. **`/verifyx`**：验证与清理收尾阶段。验证完成后，执行 **Post-verify task-dir purge**，对 `artifacts/current/<task_id>/` 目录进行安全清理。

## Python 环境治理 (Python Environment)

在 macOS 开发环境下，Python 的运行环境与依赖治理需严格遵循以下准则：

- **环境锁存**：使用专属的 **`$python-env-management`** 进行环境的长效治理，默认运行环境为 **Python 3.12**。
- **工具链选择**：推行 **uv-only** 机制。每个仓库的依赖及环境状态必须通过 `uv.lock` 进行绝对锁定，禁止使用传统的 `pip`。
- **Skill 支撑**：当环境异常或缺少 `uv` 时，调用 `skills/uv/SKILL.md` 自动进行安装与 PATH 补全。

## 独有长效会话机制 (Session Supervisor)

- **`session_supervisor`**：集成 tmux 长会话管理，具体详见 `rust-session-supervisor` 专项 skill。

## 自检诊断与验证 (Self-Test)

- **自检 Codex Hooks**：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint verify-codex-hooks
  ```
- **验证 Skill 路由及状态**：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework skills validate
  ```

## 独有环境变量与参数 (Environment Variables & Parameters)

为确保 Codex CLI 与审稿机制的稳定执行，支持以下独有环境变量：

- **`ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`**：**默认开启**（`unset` = 开）。PostTool 深度 lane 且省略 `fork_context` 时可计为独立 reviewer 证据。设 `0`/`false`/`off`/`no` 则要求 JSON 显式 `fork_context: false`。
- **`ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY`**：规定是否在会话交互过程中严格要求稳定的 Session Key，防止非幂等会话串线。
- **`ROUTER_RS_CODEX_HOOK_STATE_SALT`**：用于设定 Codex hook 的状态盐（salt），用以保障钩子状态存取的安全性。

## 会话周期与重新武装 (Session Lifecycle & Re-arm)

- **UserPromptSubmit 重新武装 (re-arm) 机制**：在每次用户提问提交（UserPromptSubmit / UPS）后，系统会执行重新武装（re-arm），重置特定拦截门控，允许新一轮的动态指令判断。


