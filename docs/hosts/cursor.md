---
last_verified: "2026-06-02"
depends_on:
  - ../spec.md
  - ../spec.md
---

# Cursor 宿主操作手册

**闭集 id**：`cursor` · **传输**：cursor-hooks · **权威**：`RUNTIME_REGISTRY.json` → `host_projections.cursor`

## 代理身份与画风 (Agent Identity & Style)

- **核心身份**：主代理定位为 **MIT 博士级科研与顶级工程专家**，具备端到端、高难度的科研与复杂系统工程执行能力。
- **回复画风**：严格保持 **专业、严谨、客观、谦逊** 的学术与工程专家风格，避免夸大、浮躁或过度礼貌。
- **回复语言**：默认面向用户的回复必须使用 **简体中文**（代码、路径、命令或第三方原文除外），且使用自然的学术中文表达，避免翻译腔。仅当用户在当轮中明确要求使用英文时，方可切换至英文。
- **回答准则**：回答避免空话，直接给出具体的、可执行的建议；对不确定的信息直接说明，严禁凭空编造。

## 能力边界与 Harness 入口 (Capabilities & Harness Entrypoints)

在 Cursor 环境下，Harness 和任务管理的核心入口与工作区定义如下：

- **Harness 核心入口**：
  - **任务推进及推进控制**：利用 `/implementx` 和 `/verifyx` 指令，配合 `framework_goal_drive` stdio 推进宏任务。
  - **任务状态治理**：`framework_goal_drive` stdio（及 MCP `goal_state_manage`）写 `GOAL_STATE.json`；Cursor **Stop** hook 用同一 `resolve_cursor_continuity_frame` / hydration 指针选 task，**不**单独扫 orphan。
- **工作区及状态产物**：
  - 核心状态与任务物化存放在 `artifacts/current/<task_id>/` 目录下。
  - 主要包含任务状态文件 `GOAL_STATE.json` 以及交互/审核状态文件 `RFV_LOOP_STATE.json`。
- **门控与审稿机制**：
  - 结合 `beforeSubmitPrompt`、`stop`、`subagentStart`/`subagentStop`、`postToolUse`、`sessionStart`/`sessionEnd` 等 7 个核心事件进行行为守卫。
  - **Stop `REVIEW_GATE` 全局 advisory-only**（对齐 [`AGENTS_CURSOR.md`](../../AGENTS_CURSOR.md)）：仅 `followup_message` nudge，**不** `permission: deny` / 硬拦 Stop。**`lifecycle_profile: my-light`** 在 My 入口或磁盘 `GOAL_STATE` 时另 **suppress** `REVIEW_GATE` / spawn-first nudge；findings-only review 仍可用（见 [`skills/code-review-deep/SKILL.md`](../../skills/code-review-deep/SKILL.md)）。

## Fail-open / Fail-closed 设计意图

Cursor 采用 **fail-closed** 策略：hook 二进制缺失时，关键门控事件返回 `decision:block`。这与 Claude Code / Codex CLI 一致。

**设计理由**：Cursor 的 7 事件 hook 紧密嵌入会话生命周期（`stop` 可阻断提交、`beforeSubmitPrompt` 可注入 spawn-first nudge）。二进制损坏意味着安全关键路径断裂，fail-closed 避免 agent 在无审查状态下执行不可逆操作。


## Hook 事件矩阵

**默认注册 7 事件**（2026-05-20 减法闭集）：`beforeSubmitPrompt`、`stop`、`sessionStart`、`sessionEnd`、`postToolUse`、`subagentStart`、`subagentStop`。已移除：`afterAgentResponse`、`beforeShellExecution`/`afterShellExecution`、`afterFileEdit`、`preCompact`（恢复见 [`MIGRATION.md`](../../MIGRATION.md)）。项目 env：[`.cursor/router-rs-hook.env`](../../.cursor/router-rs-hook.env)；`postToolUse` 对非门控工具走 **fast-path**（[`post_tool_use_needs_work`](../../core/host-projection/src/hosts/cursor_hooks/handlers.rs)）。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| Review / subagent 门控、beforeSubmit/Stop | `router-rs cursor hook <event>` | `cursor_hooks::execute_cursor_hook` → `CursorHookHost::dispatch` → `dispatch_cursor_hook_event` | `.cursor/hook-state/review-subagent-*.json`；**`ROUTER_RS_CURSOR_REVIEW_GATE_MODE`**=`strict`（默认 multiset）或 `lite`（仅 `id:` pending）；`framework doctor` 打印 mode；Stop advisory 提示上限 **`ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES`** |
| Stop / beforeSubmit 出站 | Same | [`cursor_hooks/`](../../core/host-projection/src/hosts/cursor_hooks/mod.rs) | **my-light Stop 早退**：仅 `CLOSEOUT_FOLLOWUP` + `SESSION_CLOSE_STYLE`（无 `REVIEW_GATE` / `AG_FOLLOWUP`）；非 my-light 保留完整 Stop 链；**不**合并 `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` |
| **Paper prose L4** | beforeSubmit 命中 `has_paper_prose_edit_context` | `paper_prose_hook.rs` | 合并 `PAPER_PROSE_QUALITY_HOOK`（**默认开**：`ROUTER_RS_CURSOR_PAPER_PROSE_HOOK`，`0` 关）；对抗审稿 opt-in：`ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK=1` |
| **SessionStart** | SessionStart | `cursor_hooks`（`handle_session_start`） | **仅** `Repo:` 单行（`ROUTER_RS_OPERATOR_INJECT=0` 时为空）；**无** digest / 无 pointer hint |
| **运维自检** | 手工排障 | `router-rs framework doctor --repo-root <repo>` | **metadata-only** `generated-artifacts-status`；`ROUTER_RS_TASK_LEDGER_FLOCK` 关闭时打印 WARN |

**排障（短）**：

- **`fork_context` 缺省**：默认 **`ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` 开启**时可推断 `false`；关闭后仅布尔 `false` 计独立证据。显式 `fork_context: true` 永不算。
- **磁盘 `GOAL_STATE` 与 pre-goal**：默认 strict；legacy 宽松设 `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK=0|false|off|no`。
- **`cursor-router-rs-hook.sh` exit code**：critical 事件（beforeSubmit/Stop/postToolUse/subagentStart/subagentStop）在 `router-rs` 缺失时 **fail-closed**；其余 **fail-open**。
- **fail-closed 出站字段（按事件）**：beforeSubmit / PostTool（review-armed 锁失败）/ Stop（部分路径）→ `"continue": false`；subagentStart（限额/锁失败）→ `"permission": "deny"`。launcher 缺 binary 时 PostTool 亦 `continue:false`。
- **仿宿主续跑行**：聊天区无 `router-rs ` 前缀的仿机读行勿当 hook 真源；以 hook stdout JSON 为准。
- **清门粘贴**：勿把 **`RG_FOLLOWUP`…** 当清门令牌；请用 **`rg_clear`**、拒因 token，或自然语言 override。

## 对话中断排障

症状：对话像被掐断、无法提交、子代理 `permission: deny`、Stop 后需手动 `/implementx` 续跑。

| 现象 | 常见根因 | 处理 |
|------|----------|------|
| Stop 后任务未完成 | **无** hook `GOAL_CONTINUE`（2026-05 已删） | `/implementx` + `framework_goal_drive` stdio + `artifacts/current/<task_id>/` |
| Stop 后出现 `router-rs REVIEW_GATE` / `AG_FOLLOWUP` | 非 **my-light** 且 review 未清门（advisory nudge，非硬拦） | 先 spawn `fork_context=false` 深度 lane；或 `rg_clear` / 拆开 review 与 `/implementx` |
| `beforeSubmit` 无法继续（`continue:false`） | hook-state 锁/持久化失败 | 查 `.cursor/hook-state` 权限；应急 `ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN=1` |
| 子代理 `permission: deny`（open count） | 重复 `subagentStart` 或 session 分片 | 看 `review-subagent-*.json` 的 `active_subagent_count` vs pending；升级后旧 state 可删或等新会话 |
| PostTool 卡 ~20s | L1/L3 争用或 armed 全路径 L3 | 默认已修 L3→L1 逆序；仍慢则 w2 压测后可将 gate timeout 提到 25（见 `.cursor/hooks.json`） |
| 双聊天互相影响 | 同 `cwd` 共桶 | 各聊天设 **`ROUTER_RS_CURSOR_SESSION_NAMESPACE`**（见 `.cursor/router-rs-hook.env` 注释） |
| `CLOSEOUT_FOLLOWUP`（my-light） | 无磁盘 goal 仍声称完成 | 仅 hydration 有 `GOAL_STATE` 时触发；口语「完成了」不应再拦 |

**PostToolUse timeout**：门控事件默认 **20s**（`hooks.json`）；`postToolUse` 超时会导致 review multiset 不完整 → Stop 循环。慢盘先查 hook-state 体积与锁 stderr（`hook-state lock held`）。

**router-rs 缺失**：critical 事件 **fail-closed**（`continue:false` / 工具拒绝）；确保 `core/router-rs/target/release/router-rs` 存在或 `ROUTER_RS_BIN` 指向二进制。

**SESSION_CLOSE_STYLE**：每轮 Stop 可能注入软提示；不需要时设 `ROUTER_RS_OPERATOR_INJECT=0`。

**session_key 升级**：修复后 hook-state 文件名 hash 可能变化；首会话门控状态重置，可用 `rg_clear` 或删 `.cursor/hook-state/review-subagent-*.json`（仅本机调试）。

**统一原则**：宿主配置命令须 **短命 + 超时**；语义在 Rust，不在 shell 脚本分支。

## 安装与文件分布 (Installation & Scope)

- **文件 Scope 配置**：
  - **Framework 叙事**：**User only**，路径为 `~/.cursor/rules/framework.mdc`。
  - **Harness hooks**：**Project**，路径为 `<repo>/.cursor/hooks.json`、`.cursor/router-rs-hook.env`。
  - **Review / execution gate 规则**：**Project**，路径为 `<repo>/.cursor/rules/*.mdc`。
  - **My lifecycle commands / agents**：**Project**，路径为 `<repo>/.cursor/commands/*.md`（含 `/discussx`、`/planx`、`/implementx`、`/verifyx`、**`/workflow`**）、`.cursor/agents/deep-reviewer.md`。
  - **Hook state**：**Project**，路径为 `<repo>/.cursor/hook-state/`。
  - 说明：上述 project L4 面 不在 GENERATED_ARTIFACTS drift-gate 内（手维护）；framework host-integration install --to cursor 不托管 hooks。完整清单见 [`spec.md`](../spec.md) §13。
- **环境安装命令**：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
    framework host-integration install --to cursor --scope user
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
2. **`/planx`**：规划阶段，生成或更新 `artifacts/current/<task_id>/ROADMAP.md` 与 `WAVE_STATE.json`（见 [`skills/planx/SKILL.md`](../../skills/planx/SKILL.md)），明确 minimal delta 与 verification plan，并报用户审批。
3. **`/implementx`**：执行阶段。进入执行区时，需配合 `framework_goal_drive` stdio 以及物化的 `GOAL_STATE.json`。主线程主要负责调度，**一口气**跑完 `WAVE_STATE` 全部的执行 wave。
   - **执行 Profile 调优**：`lifecycle_profile: my-light` 在 My 入口斜杠或磁盘 `GOAL_STATE` 时生效（suppress Stop 上 `REVIEW_GATE` advisory nudge 与 `beforeSubmitPrompt` spawn-first nudge；findings-only review 仍可用）。
4. **`/verifyx`**：验证与清理收尾阶段。验证完成后，执行 **Post-verify task-dir purge**，对 `artifacts/current/<task_id>/` 目录进行安全清理。

## Python 环境治理 (Python Environment)

在 macOS 开发环境下，Python 的运行环境与依赖治理需严格遵循以下准则：

- **环境锁存**：使用专属的 **`$python-env-management`** 进行环境的长效治理，默认运行环境为 **Python 3.12**。
- **工具链选择**：推行 **uv-only** 机制。每个仓库的依赖及环境状态必须通过 `uv.lock` 进行绝对锁定，禁止使用传统的 `pip`。
- **Skill 支撑**：当环境异常或缺少 `uv` 时，调用 `skills/python-env-management/SKILL.md` 自动进行安装与 PATH 补全。

## 进程管理与性能调优 (Process & Memory)

1. **构建 Release 二进制**（优化文件体积与加载速度）：
   ```bash
   CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
     cargo build --release --manifest-path core/router-rs/Cargo.toml
   ```
2. **Launcher 探测顺序**：
   仓库 `core/router-rs/target/release` $\rightarrow$ `/tmp/skill-cargo-target/release` $\rightarrow$ debug $\rightarrow$ `PATH`。
3. **项目环境变量**：
   `beforeSubmitPrompt` 支持通过 `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE` 自动注入子代理继承主会话模型的单行 nudge 机制。

## 自检诊断与验证 (Self-Test)

- **校验 Cursor Hooks**：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint verify-cursor-hooks
  ```
- **校验集成状态**：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration status
  ```
- **运行集成测试**：
  ```bash
  cargo test --manifest-path core/router-rs/Cargo.toml host_integration
  ```
