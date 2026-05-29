# Claude Desktop 宿主操作手册

**闭集 id**：`claude-desktop` · **传输**：MCP stdio · **权威**：`RUNTIME_REGISTRY.json` → `host_projections.claude-desktop`

## 代理身份与画风 (Agent Identity & Style)

- **核心身份**：主代理定位为 **MIT 博士级科研与顶级工程专家**，具备端到端、高难度的科研与复杂系统工程执行能力。
- **回复画风**：严格保持 **专业、严谨、客观、谦逊** 的学术与工程专家风格，避免夸大、浮躁或过度礼貌。
- **回复语言**：默认面向用户的回复必须使用 **简体中文**（代码、路径、命令或第三方原文除外），且使用自然的学术中文表达，避免翻译腔。仅当用户在当轮中明确要求使用英文时，方可切换至英文。**语言硬约束已内联于 project/user `.claude/CLAUDE.md` §语言**；跨宿主真源仍为 [`AGENTS.md`](../../AGENTS.md)。
- **回答准则**：回答避免空话，直接给出具体的、可执行的建议；对不确定的信息直接说明，严禁凭空编造。

## 能力边界与 Harness 入口 (Capabilities & Harness Entrypoints)

在 Claude Desktop 环境下，Harness 和任务管理的核心入口与工作区定义如下：

- **Harness 核心入口**：
  - **任务推进**：`/implementx`、`/verifyx`；Desktop 侧经 MCP `goal_state_manage`（内部同源 `framework_goal_drive` stdio）。
  - **任务状态治理**：MCP `goal_state_manage` / `goal_state_read`。
- **工作区及状态产物**：
  - 核心状态与任务物化存放在 `artifacts/current/<task_id>/` 目录下。
  - 主要包含任务状态文件 `GOAL_STATE.json` 以及交互/审核状态文件 `RFV_LOOP_STATE.json`。
- **门控与审稿机制**：
  - **无** CLI PreToolUse/Stop；依赖 MCP 工作流 + 短投影 `.claude/CLAUDE.md`。
  - **非 `my-light`**：`closeout_gate` / `goal_state_manage complete` 在 MCP 层硬拦；`my-light` 为 advisory。
  - 深度 review：**无 hook 注入**；须主动 spawn 只读 reviewer 并写 `review-lanes/*.md`，见 [`skills/code-review-deep/SKILL.md`](../../skills/code-review-deep/SKILL.md)（findings-only）。

## Hook 事件矩阵

**能力边界**：无 CLI 级 PreToolUse / Stop 硬拦截（registry `harness_capability_exceptions`）；门控靠 **MCP 工具工作流** + 短投影文案。勿与 Claude Code 的四事件 hook 表混读。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| MCP 工具工作流 | Desktop MCP stdio | `router-rs` MCP server（见项目 `.claude/CLAUDE.md`） | `artifacts/current/` 与 Code 共用；`goal_state_manage` / `closeout_gate` / `framework_snapshot` |
| 投影安装 | 一次性接入 | `router-rs framework host-integration install --to claude-desktop` | project：`.claude/mcp.json`、`.claude/settings.json`（调研网络）、`.claude/CLAUDE.md`；user：`Claude/claude_desktop_config.json` **+ macOS 3P 时 `Claude-3p/claude_desktop_config.json`** + `~/.claude/settings.json` + 稳定二进制 `~/.local/share/skill-framework/bin/router-rs` |

**Desktop 模式**：**Chat** 用本地 MCP（`web_fetch` + `browser-mcp`）；**Cowork** 经 Connectors 注入 **`browser-mcp`** 为主路径（不用 `router-rs-framework`）。运维见 **[联网操作手册](claude-desktop-networking.md)**。

**统一原则**：宿主配置命令须 **短命 + 超时**；语义在 Rust，不在 shell 脚本分支。

## 安装与文件分布 (Installation & Scope)

**一键重装（推荐，框架更新后重复执行）**：

```bash
# 在 skill 仓库根目录（推荐：Code + Desktop + user 全局规则，与 Cursor 对齐）
./scripts/install-claude.sh
# 仅 Desktop MCP：
./scripts/install-claude-desktop.sh
```

脚本会：编译 release `router-rs`（若需要）→ 安装 **project**（`.claude/*`）+ **user**（`claude_desktop_config.json`）→ 打印 `status`。

**何时重跑**：`git pull` 后、`core/router-rs` 重新编译后、Desktop MCP 断连或路由异常时。

- **文件 Scope 配置**：
  - **MCP 联动配置与工作流描述**：主要指引保存在项目文件 `.claude/CLAUDE.md` 中。
- **环境安装与注册命令**：
  ```bash
  # project scope（仓库内 .claude/*）
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
    framework host-integration install --to claude-desktop --scope project --repo-root "$PWD"

  # user scope（macOS：Claude + Claude-3p 双写 mcpServers；稳定 MCP 二进制 adhoc codesign）
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
    framework host-integration install --to claude-desktop --scope user --repo-root "$PWD"
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
   - **执行 Profile 调优**：默认使用 `lifecycle_profile: my-light`。在此配置下将关闭 `REVIEW_GATE` 硬拦截和 spawn-first nudge，采用 findings-only 机制，保持极佳的轻量化流畅体验。
4. **`/verifyx`**：验证与清理收尾阶段。验证完成后，执行 **Post-verify task-dir purge**，对 `artifacts/current/<task_id>/` 目录进行安全清理。

## Python 环境治理 (Python Environment)

在 macOS 开发环境下，Python 的运行环境与依赖治理需严格遵循以下准则：

- **环境锁存**：使用专属的 **`$python-env-management`** 进行环境的长效治理，默认运行环境为 **Python 3.12**。
- **工具链选择**：推行 **uv-only** 机制。每个仓库 of 依赖及环境状态必须通过 `uv.lock` 进行绝对锁定，禁止使用传统的 `pip`。
- **Skill 支撑**：当环境异常或缺少 `uv` 时，调用 `skills/uv/SKILL.md` 自动进行安装与 PATH 补全。

## 自检诊断与验证 (Self-Test)

- **校验宿主投影状态**：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
    framework host-integration status
  ```

## 网络与内外部调研 (Network & Research)

**运维速查**：[`claude-desktop-networking.md`](claude-desktop-networking.md)（**维护清单**：install + egress + 权限补丁；联网；Allow/Deny 持久化；自检与验证）。代理侧声明见 **`.claude/CLAUDE.md` → 联网（按标签页）**。

Claude Desktop 有两层网络限制，install 脚本会一并处理可配置部分：

| 层级 | 现象 | 框架修复 |
|------|------|----------|
| **Bash 沙箱** | `curl`/`wget` 被 Seatbelt 拦截 | project + user `settings.json`：`permissions.allow` + **`sandbox.enabled: false`**（调研面关闭 Bash 沙箱；勿误开域名墙） |
| **MCP / 无 Bash** | Desktop Chat 无 WebFetch 工具 | MCP **`web_fetch`**（router-rs 内 HTTP GET）+ **`browser-mcp`**（`browser_open` / `browser_get_text`） |
| **Operon / LAN** | RFC1918（`192.168.x.x`）`EHOSTUNREACH`；stdio MCP 经 `disclaimer` .spawn | **宿主已知回归**（[#37994](https://github.com/anthropics/claude-code/issues/37994)）；外网 HTTPS 通常可用。LAN 变通：Tailscale 非 RFC1918 地址、Claude Code CLI、或 Remote HTTP MCP |

**重装后必做**：完全退出 Claude Desktop（Cmd+Q）→ 重开 → Settings → Connectors 确认 `router-rs-framework` 与 `browser-mcp` 已连接。

**macOS 3P / gateway 模式**：Desktop 实际 user-data 在 `~/Library/Application Support/Claude-3p/`；框架 install 会将 `mcpServers` **合并**进 `Claude-3p/claude_desktop_config.json`（保留 `deploymentMode` / `preferences`）。CCD 会话仍可能受 Desktop **managed-settings** 强制沙箱约束（仅允许 `127.0.0.1` 等）；外网调研须走 MCP `web_fetch` / `browser-mcp`，勿依赖 Bash `curl`。

### Chat vs Cowork（联网能力）

| 模式 | 外网路径 | 框架 install 是否覆盖 |
|------|----------|----------------------|
| **Chat / CCD** | 本地 MCP `web_fetch`、`browser-mcp`（`claude_desktop_config.json`） | 是（双写 `Claude-3p/` + 稳定 `router-rs` 二进制） |
| **Cowork** | VM 内 `mcp__workspace__web_fetch` / `WebSearch`；**不用** `router-rs-framework` | **否** — 受 3P **`coworkEgressAllowedHosts`** 沙箱白名单约束 |

Cowork 报 `cowork-egress-blocked`、仅允许 `127.0.0.1` 时，说明 3P 配置未放行 egress（默认只允许 inference gateway）。一键补丁：

```bash
./scripts/patch-claude-desktop-3p-cowork-egress.sh
```

或手动编辑 `~/Library/Application Support/Claude-3p/configLibrary/<appliedId>.json`（`<appliedId>` 见 `_meta.json`）。详见 [Cowork 3P Configuration](https://claude.com/docs/cowork/3p/configuration) 与 [`claude-desktop-networking.md`](claude-desktop-networking.md)。

**WebSearch** 走 inference provider 服务端，与 `coworkEgressAllowedHosts` 无关；gateway 上游 429/限流需在代理侧排查。

**调研优先级**（按标签，详见 `.claude/CLAUDE.md`）：

| 标签 | 顺序 |
|------|------|
| **Chat** | `web_fetch` → `browser-mcp` → 宿主 WebFetch |
| **Cowork** | **`browser-mcp` only**（勿依赖 `web_fetch` / `WebSearch` / workspace web_fetch） |
