---
last_verified: "2026-06-09"
depends_on:
  - ../host_adapter_contract.md
  - ../harness_architecture/index.md
---

# Claude Code 宿主操作手册

**闭集 id**：`claude-code` · **传输**：claude-hooks · **权威**：`RUNTIME_REGISTRY.json` → `host_projections.claude-code`

**策略注入（双文件）**：[`AGENTS.md`](../../AGENTS.md)（内核）+ [`AGENTS_CLAUDE.md`](../../AGENTS_CLAUDE.md)（Claude Code transport delta only）；**Review gate canonical** 清门语义以本宿主 hook 为参考实现（[`host_adapter_contract.md`](../host_adapter_contract.md) §0.1），Stop **advisory-only**。

## 代理身份与画风 (Agent Identity & Style)

- **核心身份**：主代理定位为 **MIT 博士级科研与顶级工程专家**，具备端到端、高难度的科研与复杂 system 工程执行能力。
- **回复画风**：严格保持 **专业、严谨、客观、谦逊** 的学术与工程专家风格，避免夸大、浮躁或过度礼貌。
- **回复语言**：默认面向用户的回复必须使用 **简体中文**（代码、路径、命令或第三方原文除外），且使用自然的学术中文表达，避免翻译腔。仅当用户在当轮中明确要求使用英文时，方可切换至英文。
- **回答准则**：回答避免空话，直接给出具体的、可执行的建议；对不确定的信息直接说明，严禁凭空编造。

## 能力边界与 Harness 入口 (Capabilities & Harness Entrypoints)

在 Claude Code 环境下，Harness 和任务管理的核心入口与工作区定义如下：

- **Harness 核心入口**：
  - **任务推进及推进控制**：利用 `/implementx` 和 `/verifyx` 指令，配合 `framework_goal_drive` stdio（CLI hook 传输）推进宏任务。
  - **任务状态治理**：`framework_goal_drive` stdio + `artifacts/current/<task_id>/GOAL_STATE.json`。Claude Desktop 场景下使用 MCP `goal_state_manage`。
- **工作区及状态产物**：
  - 核心状态与任务物化存放在 `artifacts/current/<task_id>/` 目录下。
  - 主要包含任务状态文件 `GOAL_STATE.json` 以及交互/审核状态文件 `RFV_LOOP_STATE.json`。
- **门控与审稿机制**：
  - 拥有 `PreToolUse`、`UserPromptSubmit`、`PostToolUse` 和 `Stop` 等 4 个核心集成钩子事件。
  - **Review gate canonical**：清门语义以本宿主为准（[`host_adapter_contract.md`](../host_adapter_contract.md) §0.1）；Stop **advisory-only** `CLAUDE_REVIEW_GATE` nudge。
  - 深度 Review：**默认 `lifecycle_profile: my-light` 不注入 spawn-first**；非 my-light 时 spawn-first 配对审稿，见 [`skills/code-review-deep/SKILL.md`](../../skills/code-review-deep/SKILL.md)。

## Hook 事件矩阵

**默认注册 4 事件**（减法闭集）：`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`。项目 env：[`.claude/router-rs-hook.env`](../../.claude/router-rs-hook.env)（模板 [`configs/framework/claude-router-rs-hook.env`](../../configs/framework/claude-router-rs-hook.env)）；launcher **release 优先** 同 Cursor（[`claude-router-rs-hook.sh`](../../configs/framework/claude-router-rs-hook.sh)）。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| PreTool / Stop 守卫、settings 变更提示 | 宿主 hooks 调用 `router-rs claude hook --event=PreToolUse\|Stop\|…` | [`claude_code_hooks.rs`](../../core/router-rs/src/hosts/claude_code_hooks.rs) | `.claude/hook-state/review-subagent-*.json`、`.claude/hook-state/hook_state_*.json`（Cursor 指纹 payload 静默忽略）；出站 Claude hook JSON |
| **Claude Stop × `.claude` 状态 JSON** | Stop | `claude_code_hooks::run_stop` | `hook-state/review-subagent-*.json` / `hook_state_*.json` 缺失不单独拦截；**已存在但不可读或损坏**：**fail-closed**，`stopReason` 含 `CLAUDE_HOOK_STATE_UNREADABLE` |
| 投影规则与 hook 绑定 | `router-rs framework host-integration install --to claude` | [`host_integration/mod.rs`](../../core/router-rs/src/host_integration/mod.rs) | `.claude/rules/framework.md`、`.claude/settings.json`（四事件 hook）、`.claude/.framework-projection.json`（project scope） |
| **Paper prose L4** | `UserPromptSubmit` 写作/润色语境 | `paper_prose_hook.rs` | `PAPER_PROSE_QUALITY_HOOK`（**默认开**：`ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK`）；`ROUTER_RS_CLAUDE_PAPER_ADVERSARIAL_HOOK=1` opt-in |

**统一原则**：宿主配置命令须 **短命 + 超时**；语义在 Rust，不在 shell 脚本分支。

**Review gate 磁盘（canonical）**：`.claude/hook-state/review-subagent-<session_key>.json`（basename 真源 [`hook_review_subagent_state_basename`](../../core/core-policy/src/hook_review_disk_state.rs)）。**Legacy 自动迁移**（读时升 canonical、写后删旧文件）：`.claude/hook-state/review_gate_<key>.json`（phase-3 前）、`.claude/review_gate_<key>.json`（扁平遗留）。PreToolUse **deny** 对上述路径的写操作。并行会话分流：`ROUTER_RS_CLAUDE_SESSION_NAMESPACE`（对齐 Cursor `SESSION_NAMESPACE`）。

## 安装与文件分布 (Installation & Scope)

- **文件 Scope 配置**：
  - **Hooks 行为配置文件**：路径为 `.claude/settings.json`，绑定脚本为 [`claude-router-rs-hook.sh`](../../configs/framework/claude-router-rs-hook.sh)。
  - **项目环境变量文件**：路径为 [`.claude/router-rs-hook.env`](../../.claude/router-rs-hook.env)。
  - **Framework 规则文件**：路径为 `.claude/rules/framework.md`。
  - **项目叙事文件**：路径为 `.claude/CLAUDE.md`（项目叙事；跨宿主 policy 仍以根目录双文件 `AGENTS.md` + `AGENTS_CLAUDE.md` 为准）。
- **环境安装命令**（与 Cursor 对齐 My 生命周期；**须含 user scope** 刷新 `~/.claude/rules/framework.md`）：
  ```bash
  ./scripts/install-claude.sh
  # 或仅全局：./scripts/install-claude.sh --scope user
  ```
  其它仓库：`./scripts/claude-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"` + `install-claude.sh --scope user`。

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
3. **`/implementx`**：执行阶段。进入执行区时，需配合 `goal_state_manage` MCP / `framework_goal_drive` stdio 以及物化的 `GOAL_STATE.json`。主线程主要负责调度，**一口气**跑完 `WAVE_STATE` 全部的执行 wave。
   - **执行 Profile 调优**：默认 `lifecycle_profile: my-light`（suppress Stop 上 review **advisory** nudge 与 spawn-first；findings-only 仍可用）。Review gate 清门语义为本宿主 **canonical**，见 [`host_adapter_contract.md`](../host_adapter_contract.md) §0.1。
4. **`/verifyx`**：验证与清理收尾阶段。验证完成后，执行 **Post-verify task-dir purge**，对 `artifacts/current/<task_id>/` 目录进行安全清理。

显式辅助命令（五宿主同路径）：`/deepinterview`、`/gitx`、`/update`。

## Python 环境治理 (Python Environment)

在 macOS 开发环境下，Python 的运行环境与依赖治理需严格遵循以下准则：

- **环境锁存**：使用专属的 **`$python-env-management`** 进行环境的长效治理，默认运行环境为 **Python 3.12**。
- **工具链选择**：推行 **uv-only** 机制。每个仓库的依赖及环境状态必须通过 `uv.lock` 进行绝对锁定，禁止使用传统的 `pip`。
- **Skill 支撑**：当环境异常或缺少 `uv` 时，调用 `skills/python-env-management/SKILL.md` 自动进行安装与 PATH 补全。

## 进程管理与性能调优 (Process & Memory)

1. **构建 Release 二进制**（优化体积与响应速度）：
   ```bash
   CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
     cargo build --release --manifest-path core/router-rs/Cargo.toml
   ```
2. **Launcher 探测顺序**：
   仓库 `core/router-rs/target/release` $\rightarrow$ `/tmp/skill-cargo-target/release` $\rightarrow$ debug $\rightarrow$ `PATH`。

## 自检诊断与验证 (Self-Test)

- **运行 Claude Hook 集成测试**：
  ```bash
  cargo test --manifest-path core/router-rs/Cargo.toml claude
  ```
- **自检主机投影状态**：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration status
  ```
