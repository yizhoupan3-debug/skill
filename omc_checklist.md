# OMC 替代与 Rust 深接入执行清单

> 目标：在**不保留 OMC 作为运行时依赖**、也**不把宿主私有语义反写进 framework truth** 的前提下，把你真正需要的两项能力落成你体系里的新可插拔能力层：
>
> 1. **tmux 外部会话管理**
> 2. **限流后自动续跑**
>
> 同时必须完成：
>
> - **彻底删除 OMC 残留**
> - **保留 `autopilot` 与 `deepinterview` 的用户心智**
>   - Claude: `/autopilot` `/deepinterview`
>   - Codex: `$autopilot` `$deepinterview`
> - **并明确写死：它们承接原版 OMC 的核心能力，但本仓版本必须做得更强**
>   - `autopilot`：更强的根因定位、验证证据、恢复续跑、收敛闭环
>   - `deepinterview`：更强的根因定位、分级 findings、证据化核查、fix -> verify 收敛
> - **Rust-first**
> - **与当前 framework / runtime / host adapter 深度接入**

## 当前状态快照

- **项目内 shared truth 基本已经 OMC-free**
  - 当前仓库扫描未命中 project-scoped `omc` / `oh-my-claude*` / `autopilot` / `deepinterview` 文本面。
  - 项目 `CLAUDE.md` 已是 shared framework proxy，不再以 OMC 为真源。
  - `.claude/settings.json` / `.claude/hooks/*` / `.claude/commands/*` 当前已由 `scripts/materialize_cli_host_entrypoints.py` 管理。

- **宿主级 OMC 残留仍然存在，且仍在生效**
  - `/Users/joe/.claude/CLAUDE.md` 顶部仍有 `<!-- OMC:START --> ... <!-- OMC:END -->` 注入块，版本 `4.13.0`。
  - `/Users/joe/.claude/settings.json`
    - `enabledPlugins.oh-my-claudecode@omc = true`
    - `extraKnownMarketplaces.omc` 仍指向 `Yeachan-Heo/oh-my-claudecode`
    - `statusLine.command` 仍指向 `/Users/joe/.claude/hud/claude-hud.sh`
  - `/Users/joe/.claude.json` 仍记录 `oh-my-claudecode:*` 命令历史：
    - `oh-my-claudecode:autopilot`
    - `oh-my-claudecode:team`
    - `oh-my-claudecode:deep-interview`
    - `oh-my-claudecode:autoresearch`
    - `oh-my-claudecode:verify`
  - Claude 插件/缓存残留仍在：
    - `/Users/joe/.claude/plugins/cache/omc`
    - `/Users/joe/.claude/plugins/oh-my-claudecode`
    - `/Users/joe/.claude/plugins/data/oh-my-claudecode-omc`
    - `/Users/joe/.claude/plugins/marketplaces/omc`
    - `/Users/joe/.claude/.omc`
    - `/Users/joe/.claude/hud/omc-hud.mjs`
  - npm / PATH 残留仍在：
    - `/Users/joe/.npm-global/bin/omc`
    - `/Users/joe/.npm-global/bin/omc-cli`
    - `/Users/joe/.npm-global/bin/oh-my-claudecode`
    - `/Users/joe/.npm-global/lib/node_modules/oh-my-claude-sisyphus`

- **工作区级 OMC 噪音也仍在**
  - 当前 repo 根目录存在 `.omc/`
  - `.gitignore` 仍忽略 `.omc/`
  - 这说明项目层虽然已不再把 OMC 当真源，但旧运行态仍未完成退场

- **现有 framework 已经具备可复用的底座**
  - 已有共享 CLI-family host adapter：
    - `codex_cli_adapter`
    - `claude_code_adapter`
  - 已有 Rust-first background/control-plane 底座：
    - `scripts/router-rs/src/background_state.rs`
    - `scripts/runtime_background_cli.py`
    - `codex_agno_runtime/src/codex_agno_runtime/control_plane_contracts.py`
  - 已有 continuity / resume / attach / replay seams：
    - `event_transport`
    - `checkpoint_resume_manifest`
    - `describe_runtime_event_handoff`
  - 已有 Claude host materialization 面：
    - `scripts/materialize_cli_host_entrypoints.py`
    - `.claude/commands/*.md`
    - `.claude/hooks/*.sh`
  - 已有 Codex skill routing 面：
    - `skills/*/SKILL.md`
    - `skills/SKILL_MANIFEST.json`

- **当前缺口不是底层 runtime，而是缺一个 shared supervisor capability**
  - 还没有 framework-native 的 **tmux 外部会话管理**
  - 还没有 framework-native 的 **限流后自动续跑**
  - 还没有 project-native 的 `autopilot` / `deepinterview` 命名能力面

---

## 目标架构

把 OMC 里你真正要的东西，拆成你体系自己的 **Rust-first shared supervisor lane**：

### 1. 新的共享能力层

建议新增一个 framework-native 能力面，暂定名：

- `session-supervisor`

它不是 OMC 兼容壳，也不是新的 host truth，而是：

- **Rust-owned**
- **host-neutral contract**
- **通过 thin host drivers 接 Codex / Claude**
- **状态进入现有 control-plane / background-state / continuity artifacts**

### 2. 职责拆分

- **Rust core**
  - worker session registry
  - tmux pane/session lifecycle
  - blocked/running/resuming/terminal 状态机
  - rate-limit wait / backoff / resume scheduler
  - transport / handoff / recovery anchors

- **host drivers**
  - `claude_driver`
  - `codex_driver`
  - 只负责：
    - spawn
    - resume
    - detect rate-limit
    - map host-specific flags / CLI behavior

- **user-facing alias surfaces**
  - Claude:
    - `/autopilot`
    - `/deepinterview`
  - Codex:
    - `$autopilot`
    - `$deepinterview`
  - 这些 alias 只是 shared capability 的薄入口，不允许各宿主分叉语义

### 3. 数据落点

- **严禁继续使用 `.omc/**` 作为 steady-state state root**
- 新能力应接入现有路径族：
  - runtime durable state: `codex_agno_runtime/data/*` 或现有 background-state store
  - continuity / evidence: `SESSION_SUMMARY.md` `NEXT_ACTIONS.json` `EVIDENCE_INDEX.json` `TRACE_METADATA.json`
  - ops / debug evidence: `artifacts/ops/*`

### 4. 设计原则

- 不 vendoring OMC
- 不复制 OMC agent catalog / prompt 语义
- 不把 OMC 的命令名当作框架真源，只保留用户心智别名
- host adapter 仍然只是 thin projection
- runtime/controller authority 继续收口到 Rust contract lane

---

## 本轮并行任务总表

- 本轮可并行执行任务总数：**6 项**
- 并行原则：**每项只改自己负责的文件族；共享 owner 文件只由指定 lane 负责**
- 完成定义：**6 项都满足验收标准，且 OMC 不再作为 live runtime / host prompt / plugin dependency 存在**

| ID | 任务 | 主要目标 | 独占写入范围 |
|---|---|---|---|
| 1 | OMC 退场 contract 与 capability discovery 收口 | 把 “删 OMC、保能力、Rust-first” 冻结成 shared contract | `docs/host_adapter_contracts.md`, `docs/rust_contracts.md`, `configs/framework/RUNTIME_REGISTRY.json`, `codex_agno_runtime/src/codex_agno_runtime/runtime_registry.py`, `codex_agno_runtime/src/codex_agno_runtime/framework_profile.py`, 相关 contract tests |
| 2 | Rust session-supervisor control plane | 新增共享 session supervisor 状态机与 durable store | `scripts/router-rs/src/background_state.rs`, `scripts/router-rs/src/framework_runtime.rs`, `scripts/router-rs/src/main.rs`, 必要时新增 `scripts/router-rs/src/session_supervisor.rs`, 对应 Rust tests |
| 3 | tmux 外部会话管理与 host drivers | 落地 Codex/Claude 外部 worker session 生命周期 | `scripts/router-rs/src/main.rs`, 新增或更新 host-driver Rust 模块，必要时新增 `scripts/runtime_supervisor_cli.py` 作为 thin wrapper，`tests/test_codex_agno_runtime_runtime.py`，background/session tests |
| 4 | 限流后自动续跑 | 落地 host-neutral wait/resume daemon 与 rate-limit classifier | `scripts/router-rs/src/main.rs`, 必要时新增 `scripts/router-rs/src/rate_limit_wait.rs`, `codex_agno_runtime/src/codex_agno_runtime/event_transport.py` 仅限薄投影适配，相关 runtime/control-plane tests |
| 5 | `autopilot` / `deepinterview` 保留但去 OMC 依赖 | 以 shared framework truth 重建两个能力面，并分别投影到 Claude `/` 和 Codex `$` | `skills/autopilot/SKILL.md`, `skills/deepinterview/SKILL.md`, `skills/SKILL_MANIFEST.json`, `scripts/materialize_cli_host_entrypoints.py`, `.claude/commands/autopilot.md`, `.claude/commands/deepinterview.md`, skill / host-entrypoint tests |
| 6 | 宿主 cutover 与 OMC 残留清除 | 完成 Claude / Codex host 切换与 OMC 全面卸载 | `/Users/joe/.claude/CLAUDE.md`, `/Users/joe/.claude/settings.json`, `/Users/joe/.claude.json`, `/Users/joe/.claude/plugins/**`, `/Users/joe/.claude/.omc/**`, `/Users/joe/.claude/hud/omc-hud.mjs`, `/Users/joe/.npm-global/bin/{omc,omc-cli,oh-my-claudecode}`, `/Users/joe/.npm-global/lib/node_modules/oh-my-claude-sisyphus`, repo `.omc/`, `.gitignore`（仅当最终确认不再需要忽略 `.omc/`） |

---

## 1. OMC 退场 contract 与 capability discovery 收口

### 当前状态

- framework 已有 `codex_cli_adapter` / `claude_code_adapter` / `cli_family_capability_discovery`
- 但还没有把 “shared supervisor capability” 作为一等 contract 外显
- OMC 退场边界也还没冻结成 contract

### 目标

把以下事实冻结到 shared contract：

- OMC 是**被替代对象**，不是兼容内核
- 新能力面是 framework-native：
  - `external_session_supervisor`
  - `rate_limit_auto_resume`
  - `host_resume_entrypoint`
  - `host_tmux_worker_management`
- `autopilot` / `deepinterview` 是 shared capability alias，不是 OMC 兼容层

### 独占写入范围

- `docs/host_adapter_contracts.md`
- `docs/rust_contracts.md`
- `configs/framework/RUNTIME_REGISTRY.json`
- `codex_agno_runtime/src/codex_agno_runtime/runtime_registry.py`
- `codex_agno_runtime/src/codex_agno_runtime/framework_profile.py`
- capability / contract 相关测试

### 禁止越界

- 不实现具体 tmux / wait daemon
- 不改 host 私有设置文件
- 不把 OMC 命令名写成 canonical contract id

### 交付物

- `session-supervisor` 的 shared contract
- Codex / Claude capability discovery 中新增 supervisor / auto-resume 能力位
- OMC 退场 contract 和残留清理清单

### 验收标准

- capability discovery 能明确看见：
  - 哪些 host 支持外部会话管理
  - 哪些 host 支持 resume
  - 哪些 host 支持 rate-limit auto-resume
- contract 中明确声明 `.omc/**` 不是新的 runtime truth
- `autopilot` / `deepinterview` 的 canonical owner 是 framework，而不是宿主插件

---

## 2. Rust session-supervisor control plane

### 当前状态

- 已有 durable background state store
- 已有 queued/running/completed 等背景任务语义
- 但还没有 worker session / tmux pane / host session id 的统一建模

### 目标

在 Rust control plane 中新增 supervisor 状态机，至少覆盖：

- worker identity
- host kind
- tmux session / pane identity
- workspace / worktree path
- attached session id / resume token
- blocked reason
- retry policy
- terminal status

### 独占写入范围

- `scripts/router-rs/src/background_state.rs`
- `scripts/router-rs/src/framework_runtime.rs`
- `scripts/router-rs/src/main.rs`
- 如确有必要，新增 `scripts/router-rs/src/session_supervisor.rs`
- 对应 Rust tests

### 禁止越界

- 不先做 host-specific launch shell 脚本泥球
- 不在 Python 侧重写 supervisor authority
- 不重新发明第二套 continuity artifacts

### 交付物

- Rust-owned `session-supervisor` state store
- session lifecycle schema
- session summary / group summary / recovery summary payload

### 验收标准

- session supervisor state 可 durable 存储并恢复
- session 状态机可表达：
  - `queued`
  - `launching`
  - `running`
  - `blocked_rate_limit`
  - `resume_scheduled`
  - `resuming`
  - `completed`
  - `failed`
  - `interrupted`
- 不需要 `.omc/` 即可恢复 supervisor 状态

---

## 3. tmux 外部会话管理与 host drivers

### 当前状态

- Claude CLI 已有原生 `--tmux` / `--worktree` / `--continue` / `--resume`
- Codex CLI 没有原生 `--tmux`，但有稳定的 `resume`
- 当前 framework 还没有统一的 host driver 层

### 目标

把“拉起真实外部 CLI 工人并托管生命周期”做成 shared capability：

- Claude: 可用其原生 tmux/worktree/resume 能力
- Codex: 由外层 supervisor 拉起 tmux pane 并托管 `codex` CLI

### 独占写入范围

- `scripts/router-rs/src/main.rs`
- 新增或更新 host-driver Rust 模块
- 如确有必要，新增 `scripts/runtime_supervisor_cli.py` 作为 thin wrapper
- `tests/test_codex_agno_runtime_runtime.py`
- background/session 定向测试

### 禁止越界

- 不把 host driver 逻辑回写进 `host_adapter_contracts.md` 之外的 framework truth
- 不把 tmux pane 输出当作唯一恢复源
- 不复用 OMC 的 `team` / `omc team` 命名和内部状态目录

### 交付物

- `claude_driver`
- `codex_driver`
- shared launch / stop / inspect / resume API

### 验收标准

- supervisor 能以统一入口管理 Claude / Codex 外部 worker
- 每个 worker 都能记录：
  - host
  - cwd/worktree
  - tmux session/pane
  - session id / resume target
  - current status
- 可以做长活并行、跨模型外包、后台挂起
- 全链路不依赖 OMC 插件目录或 `.omc` 状态文件

---

## 4. 限流后自动续跑

### 当前状态

- 现有 runtime 已有 replay / resume / handoff / heartbeat seams
- 但还没有“宿主级限流等待器 + 自动恢复调度器”

### 目标

做一个 Rust-owned 的 wait/resume daemon：

- 识别 host-specific rate-limit/overload block
- 计算 `next_resume_at`
- 按策略自动继续
- 将结果写回 control-plane / evidence

### 独占写入范围

- `scripts/router-rs/src/main.rs`
- 如确有必要，新增 `scripts/router-rs/src/rate_limit_wait.rs`
- 仅在必要时更新 `codex_agno_runtime/src/codex_agno_runtime/event_transport.py` 的薄投影
- runtime / control-plane tests

### 禁止越界

- 不把“继续”逻辑写成宿主脚本黑盒
- 不依赖轮询 `.omc` 或 OMC HUD state
- 不把限流检测写成字符串散落在多个脚本里

### 交付物

- host-neutral wait/resume state machine
- Claude / Codex rate-limit classifier
- auto-resume daemon CLI / control-plane entrypoint

### 验收标准

- Claude / Codex 都能被归一化成统一 blocked reason
- 系统能为 blocked worker 生成 `next_resume_at`
- 到点后能调用：
  - Claude: `claude --continue` 或 `claude --resume ...`
  - Codex: `codex resume ...` 或 `codex resume --last`
- 恢复尝试与失败原因会进入 durable state 与 evidence
- daemon authority 在 Rust，不以 Python 作为 steady-state owner

---

## 5. `autopilot` / `deepinterview` 保留但去 OMC 依赖

### 当前状态

- 当前 repo 还没有 project-native 的 `autopilot` / `deepinterview`
- 但已有可组合的 skill 底座：
  - `execution-controller-coding`
  - `idea-to-plan`
  - `systematic-debugging`
  - `plan-to-code`
  - `subagent-delegation`
  - `code-review`
  - `architect-review`
  - `security-audit`
  - `test-engineering`

### 目标

保留用户熟悉的两个名字，但让它们变成你自己的 shared capability alias：

- `autopilot`
  - 不是 OMC 的 prompt 复制品
  - 是对你现有 execution-controller / planning / debugging / verification 体系的统一入口
- `deepinterview`
  - 不是 OMC reviewer prompt 复制品
  - 是对你现有 review / architecture / security / test / convergence 流程的统一入口

### 独占写入范围

- `skills/autopilot/SKILL.md`
- `skills/deepinterview/SKILL.md`
- `skills/SKILL_MANIFEST.json`
- `scripts/materialize_cli_host_entrypoints.py`
- `.claude/commands/autopilot.md`
- `.claude/commands/deepinterview.md`
- skill / host-entrypoint tests

### 禁止越界

- 不拷贝 OMC 的 agent catalog / magic keywords / tier-0 workflow 叙事
- 不在 Claude 和 Codex 上分叉 `autopilot` / `deepinterview` 语义
- 不把这两个 alias 做成宿主插件专属行为

### 交付物

- Codex:
  - `$autopilot`
  - `$deepinterview`
- Claude:
  - `/autopilot`
  - `/deepinterview`
- 统一的 shared capability definition

### 推荐语义

- `autopilot`
  - 主 owner：`execution-controller-coding`
  - 前置 reroute：
    - 任务模糊 → `idea-to-plan`
    - 根因未知 → `systematic-debugging`
  - 执行 owner：
    - `plan-to-code`
    - `subagent-delegation`
    - `execution-audit-codex`

- `deepinterview`
  - 主 owner：`code-review`
  - 叠加 review lanes：
    - `architect-review`
    - `security-audit`
    - `test-engineering`
    - 必要时 `execution-audit-codex`

### 验收标准

- Claude `/autopilot` `/deepinterview` 可用
- Codex `$autopilot` `$deepinterview` 可用
- 两端只是不同入口，不是不同逻辑
- 用户断开 OMC 后仍保留原先对这两个名字的使用习惯

---

## 6. 宿主 cutover 与 OMC 残留清除

### 当前状态

- 当前 OMC 仍然从全局 Claude prompt、插件、marketplace、npm、`.omc` 路径、命令历史多个层级残留
- 这会继续污染 Claude host shell，并且模糊新 framework-native supervisor 的权威边界

### 目标

把 OMC 从“还在影响 live host 行为”降到“完全退场，不再被加载、不再被依赖、不再再生残留”。

### 独占写入范围

- `/Users/joe/.claude/CLAUDE.md`
- `/Users/joe/.claude/settings.json`
- `/Users/joe/.claude.json`
- `/Users/joe/.claude/plugins/**`
- `/Users/joe/.claude/.omc/**`
- `/Users/joe/.claude/hud/omc-hud.mjs`
- `/Users/joe/.npm-global/bin/{omc,omc-cli,oh-my-claudecode}`
- `/Users/joe/.npm-global/lib/node_modules/oh-my-claude-sisyphus`
- repo `.omc/`
- `.gitignore`（只在最终确认不再需要 `.omc/` 忽略后处理）

### 禁止越界

- 不误删你当前 framework 自己的 `.claude/hooks/*` / `.claude/commands/*`
- 不把 Claude global proxy 一起删空，应该替换为 framework-native 全局入口
- 不保留任何仍会被 host 继续自动加载的 OMC 注入块

### 交付物

- host cleanup inventory
- host cutover script / runbook
- OMC uninstall and purge steps

### 建议切换顺序

1. 先完成新 capability 与别名入口
2. 再切全局 Claude prompt/config
3. 再删 plugin / marketplace / npm 安装
4. 最后清 repo/workspace `.omc/` 与历史残留

### 验收标准

- `~/.claude/CLAUDE.md` 不再含 OMC 注入块
- `~/.claude/settings.json` 不再启用 `oh-my-claudecode@omc`
- `~/.claude/settings.json` 不再含 `extraKnownMarketplaces.omc`
- `~/.claude/plugins/**` 不再保留 OMC 目录
- `~/.claude/.omc/**` 被清空
- `~/.npm-global/bin/omc*` 与 `oh-my-claudecode` 消失
- `~/.npm-global/lib/node_modules/oh-my-claude-sisyphus` 消失
- repo 根目录 `.omc/` 被清理，且新体系运行不会再生成 `.omc/`
- 如 `~/.claude.json` 中 `oh-my-claudecode:*` 只剩历史 usage 条目，也应做一次备份后清理；若该文件结构不适合精确删改，则至少确保它不再驱动任何 live behavior

---

## 收口顺序

本轮建议按以下 gate 顺序收口，而不是一次性乱改：

1. **Task 1**
   - 先冻结 contract / capability discovery / retirement boundary
2. **Task 2 + Task 3**
   - 落 Rust supervisor core 与 tmux/session drivers
3. **Task 4**
   - 加 rate-limit auto-resume
4. **Task 5**
   - 把 `autopilot` / `deepinterview` 做成 shared alias
5. **Task 6**
   - 彻底切换宿主并删除 OMC 残留

只有在 **Task 5 已经可用** 后，才允许执行 **Task 6** 的最终删除动作。

---

## 本轮完成定义

以下条件必须同时成立，才算这轮真正完成：

1. Claude 与 Codex 都能使用新的 shared supervisor capability
2. tmux 外部 worker 管理可用
3. 限流后自动续跑可用
4. `/autopilot` `/deepinterview` 与 `$autopilot` `$deepinterview` 可用
5. OMC 不再作为：
   - 全局 prompt 注入源
   - 插件依赖
   - marketplace 源
   - npm/CLI 依赖
   - 状态目录
6. steady-state authority 明确在 Rust/control-plane，而不是 Python 脚本拼接层

---

## 不纳入本轮的内容

以下内容**不纳入本轮**，避免把 OMC 替代任务做歪：

1. **复制 OMC 的完整 agent/command 生态**
   - 本轮只保留你真正要的两项能力与两个别名心智。

2. **把 OMC 当 compatibility runtime 长期共存**
   - 本轮目标是退场，不是双栈长期并存。

3. **重新发明第二套 continuity / state / artifact 系统**
   - 必须接入现有 background state、continuity artifacts、resume/handoff seams。

4. **把 host adapter 做成 controller**
   - host adapter 仍是 thin projection，不能抢 framework core 主导权。

5. **用 Python 长期持有 wait/resume authority**
   - 可以短期做 thin wrapper，但 steady-state authority 必须在 Rust。
