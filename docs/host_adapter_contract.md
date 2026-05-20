# 宿主适配契约（Host adapter contract）

本文件描述 **新宿主如何接入** 本仓库的连续性 harness 与 `router-rs` 控制面：哪些能力可移植复用、宿主侧事件如何映射到 CLI、以及 L4/L5 边界。实现细节仍以代码与 [`harness_architecture.md`](harness_architecture.md) 为准。

**权威列表**：`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported` 为 **当前闭集宿主 id**；安装/状态/卸载与 Codex **`host_entrypoints_sync_manifest`** 的 `supported_hosts` / `host_entrypoints` 由同一注册表推导（实现见 `router-rs` 中 `framework_host_targets`）。

**历史字段**：`host_targets.entrypoint_files` 已从注册表移除，请勿再添加；宿主策略入口与同步 manifest 的入口集合以 `host_targets.metadata.<host>.host_entrypoints` 为唯一权威（消费路径：`framework_host_targets::host_entrypoints_value_for_id`）。

**相关契约**（英文实现侧叙事）：[`rust_contracts.md`](rust_contracts.md)。**Codex 投影专用英文契约**：[`host_adapter_contracts.md`](host_adapter_contracts.md)（勿与本文混读）。

**手稿技能（paper-workbench 栈）**：论文前门与专科 lane 的可读契约以仓库 `skills/` 下对应 `SKILL.md` 与 reference 为内容真源；**安装与宿主投影**不以某一 IDE 为专属——闭集宿主列表与安装工具名以 **`configs/framework/RUNTIME_REGISTRY.json`** 为准，落地到 Cursor/Codex 等工作区时使用 **`router-rs framework host-integration install --to <host>`**（实现见 `host_integration.rs`）。技能栈索引见 [`../skills/paper-workbench/references/RESEARCH_PAPER_STACK.md`](../skills/paper-workbench/references/RESEARCH_PAPER_STACK.md)。

**解绑与共同沉降验收**：历史清单已归档为 stub（见 [`docs/plans/README.md`](plans/README.md)）；运行时以本文 §3.1 工程清单、[harness_architecture.md](harness_architecture.md) 与 `router-rs framework maint update-audit` 为准。

## 快速路径（我要接新宿主）

- **先读**：[`harness_architecture.md`](harness_architecture.md) **§5**（扩展规则）与 **§6**（文件映射），再读本文件 **[§3.1](#31-可复制执行清单工程顺序)** 工程清单与下文 **§0**（维护地图）。
- **再改**：按 §3.1 勾选推进；合并顺序见该节 **「PR 顺序」** 一行（`RUNTIME_REGISTRY` → hooks 模块 → `dispatch` → `host_integration` → L4 → 测试）。
- **跑测**：`cargo test --manifest-path scripts/router-rs/Cargo.toml`；若改动仓库根 [`tests/`](../tests/) 下用例，再于仓库根执行 `cargo test`。
- **`codex sync` / 入口同步（材料化）**：**首选**在重建 `router-rs` 后执行 **`router-rs framework sync-entrypoints --repo-root "$PWD"`** 以重材料化宿主入口与 manifest；若团队心智仍偏 Codex 子命令，可使用 **`router-rs codex sync --repo-root "$PWD"`**（与上者同一 `sync_host_entrypoints` 实现，兼容别名）。变更落在 **Codex 投影链**或需重材料化 `AGENTS.md` / `.codex/*` 时按此路径操作。

---

## 0. North-star contract 与维护地图

**解耦**：宿主差异只允许停留在 L4 适配壳与 `RUNTIME_REGISTRY.host_targets` 元数据；L2 工件 schema、L3 CLI 行为、门控/证据追加逻辑必须复用 Rust owner。**无感**：用户在 **任一直持闭集宿主**上得到同一套 **harness 连续性、证据、closeout 与热路由闭集语义**；安装工具名、入口文件形状与 hook 事件名仍可按宿主变化。

**非目标 / 边界**：

- 不把每个宿主做成 **独立** runtime fork；不在 hook shell、`.mdc` 或 skill prose 中复制 L3 决策。

## 0.1 Countable REVIEW_GATE deep review lanes

**权威真源**（`configs/framework/RUNTIME_REGISTRY.json`，`registry_review_gate.rs` 编译期嵌入）：

| 字段 | 消费宿主 | 含义 |
|------|----------|------|
| `review_gate.deep_gate_lanes` | **Cursor**、**Codex CLI**（`hook_common::is_deep_review_gate_lane_normalized`） | 可数深度审稿 lane：**仅** `general-purpose`、`best-of-n-runner` 及归一化等价名 `generalpurpose`/`bestofnrunner` |
| `review_gate.claude_reviewer_lanes` | **Claude Code**（`claude_hooks::reviewer_lane` → `is_claude_reviewer_lane_from_registry`） | 上表四拼写 **加上** `review`、`reviewer`、`critic`、`code-review` |

**勿混读**：`deep_gate_lanes` **不是** Claude 超集；在 Cursor/Codex 上使用 `subagent_type: "review"` 且 `fork_context=false` **不会**计入独立审稿证据。

本文件不维护第二份 lane 枚举列表。增删 lane 时改 `RUNTIME_REGISTRY.json` 对应字段并重建 `router-rs`。

### 跨宿主 REVIEW_GATE 差异（operator）

| 能力 | Cursor | Codex CLI | Claude Code | Claude Desktop / codex-app |
|------|--------|-----------|-------------|----------------------------|
| 可数深度 lane 真源 | `deep_gate_lanes` | `deep_gate_lanes` | `claude_reviewer_lanes` | 无 hook 面 |
| subagentStart/Stop multiset | ✓ | ✗（单次 PostToolUse 证据） | ✗ | ✗ |
| `reject_reason` / `rg_clear` Stop 清门 | ✓ | ✗ | ✗ | ✗ |
| Stop 硬短码 | `router-rs REVIEW_GATE incomplete` | `CODEX_REVIEW_GATE` | `CLAUDE_REVIEW_GATE` | — |
| GSD `GSD_GOAL_CONTINUE` on Stop | ✓ | ✗（continuity checkpoint） | ✗（UserPromptSubmit nudge） | ✗ |
| Stop soft-nag / spoof scrub | ✓ | 部分 | 部分 | — |

细节见 [`harness_architecture.md`](harness_architecture.md) §5.0 与各 [`docs/hosts/*.md`](hosts/cursor.md)。

- **产品级外部能力**（tmux `session_supervisor`、原生 MCP 配置面、桌面线程宿主等）可以按宿主不同；这些以 `RUNTIME_REGISTRY.host_projections.*.capabilities` 与相关 status 字段为显式真源，**不得**从 skill `platforms` 或热路由里“假看齐”。
- **Harness hook 面严格矩阵**：`host_projections.*.harness_capabilities` 默认必须满足 [`configs/framework/RUNTIME_REGISTRY_SCHEMA.json`](configs/framework/RUNTIME_REGISTRY_SCHEMA.json) 中的 `harness_capability_policy`（`core_always` + `cli_agent_hook_baseline`）。**若某宿主无法提供** `cli_agent_hook_baseline` 中的某项，必须在同一投影对象上声明 `harness_capability_exceptions`（`cap` + `status: unsupported` + 非空 `rationale`）；根目录 `tests/policy_contracts.rs` 的 `runtime_registry_host_projections_split_harness_capabilities` 从 schema + registry **机读**校验，禁止再用宿主名字硬编码例外。
- **热路由宿主展开**：`SKILL.md` 未声明 `platforms` 或声明 `supported` / `all-hosts` 时，`router-rs framework skills refresh` 将 `host_support.platforms` 展开为 **`host_targets.supported` 全量**；热路由技能（`SKILL_ROUTING_RUNTIME.json`）在策略上应对每一闭集 id 可路由，**豁免**仅保留给确属 Codex 安装面 skills（见根 `tests/policy_contracts.rs` 常量）。**与上条分工**：NL 热路由默认闭集全覆盖；**hook 级 harness 能力**以 registry 矩阵与例外为准，不与 skill `platforms` 混读。
- **NL 热路由 suppress/boost 数据面**：按记录（slug / gate 等）与 `signals.rs` 谓词组合的 suppress、boost 规则真源在 [`configs/framework/NL_ROUTE_ADJUSTMENTS.json`](../configs/framework/NL_ROUTE_ADJUSTMENTS.json)（`router-rs` 嵌入，`route/nl_route_adjustments.rs`）。跨查询的短语 marker（如 meta-routing 子串表）在 [`configs/framework/ROUTING_SIGNAL_MARKERS.json`](../configs/framework/ROUTING_SIGNAL_MARKERS.json)；**不要**把两类规则塞进同一文件以免双真源争用。
- **Review gate lane 闭集（Cursor/Codex）**：`review_gate.deep_gate_lanes`；Claude Code 用 `review_gate.claude_reviewer_lanes`（见 §0.1 表）。

**宿主集合术语（避免混读）**：
`host_targets.supported` 是全局闭集宿主 id；
`host_targets.metadata.<host>.projection_status` / `installable` 决定该宿主是否进入 install/status/remove 投影路径；
`host_projections` 是当前可生成 profile payload 的宿主集合；
`framework_commands.*.host_entrypoints` 是显式命令入口覆盖集合；
`skills/SKILL_PLUGIN_CATALOG.json` 中 `skills.<slug>.host_support.platforms` 是 skill body 可用宿主集合。`codex-app` 可以出现在 skill host support 中，即使 framework command 显式入口只列 `codex-cli`；这表示同一 Codex 投影族的消费面不同，不是 drift。

| 维护面 | 不变量（共享） | 变量（宿主元数据 / 薄适配） | grep anchor |
|--------|----------------|-----------------------------|-------------|
| L2 连续性 | `artifacts/current`、`EVIDENCE_INDEX`、`GOAL_STATE`、`SESSION_SUMMARY` schema | 当前任务指针路径由 repo root 解析 | `CURRENT_ARTIFACT_DIR` / `EVIDENCE_INDEX_FILENAME` |
| 事件 × CLI × L2 | 宿主事件最终调用 `router-rs`，验证类命令追加同一 evidence row 形状 | Codex `PostToolUse` / Cursor `postToolUse` 的事件字段归一化 | `try_append_post_tool_shell_evidence` |
| 宿主安装/入口 | 闭集宿主 id 来自 `host_targets.supported`，缺元数据 fail-closed；install 遍历只使用 `projection_status=implemented && installable=true` | `host_targets.metadata.<host>.install_tool`、`projection_status`、`installable` 与 `host_entrypoints` | `installable_host_id_and_skills_install_tool_pairs` |
| L4 / L5 边界 | L4 只做 argv/stdin/超时/路径转发；L5 只承载 skill 契约与可读叙事 | `.cursor/rules/*.mdc`、Codex `AGENTS.md` 投影形状不同 | `host_entrypoints_sync_manifest` |
| `${CODEX_HOME}/skills` | 表示 Codex 用户级 skill 投影根；仓库开发态优先 `skills/` | 仅 Codex install/sync 使用该 HOME 语义，Cursor 不复用 | `workspace_bootstrap_defaults.skills.user_dir` |
| **Skill 宿主元数据（`host_support.platforms`）** | 编辑 **`skills/<slug>/SKILL.md`** 顶层或 `metadata.platforms`（YAML）；**缺省或未写**时按 `RUNTIME_REGISTRY.host_targets.supported` **全集**展开；可用 **`supported` / `all-hosts`** 令牌显式表达同一语义；维护后运行 **`router-rs framework skills validate`**（可选 **`refresh --write`** 写 `SKILL_TIERS.json`） | 闭集 id 为 `codex-cli` / `codex-app` / `cursor` / `claude-code` / `claude-desktop`；历史别名 `codex`→双 Codex id、`claude`→`claude-code` 由路由层归一 | `tests/policy_contracts.rs` **`runtime_host_support_platforms_are_registry_closed_and_match_skill_md`**、**`hot_runtime_skill_records_cover_all_supported_hosts`** |

单行指针：五层模型见 [`harness_architecture.md`](harness_architecture.md)；Rust API / CLI 契约见 [`rust_contracts.md`](rust_contracts.md)；跨宿主语言、路由与执行协议见仓库根 [`../AGENTS.md`](../AGENTS.md)。

**闭集宿主扩展**：除 Codex / Cursor 外，Claude Code 闭集 id 为 **`claude-code`**（注册表 `host_targets.supported`）；hooks 通过 **`router-rs claude hook`**，投影安装 **`router-rs framework host-integration install --to claude`**（`install_tool` 名 **`claude`** 见 `RUNTIME_REGISTRY.json`）。**Claude Desktop** 闭集 id 为 **`claude-desktop`**；hooks 通过 **`router-rs claude-desktop agent`**，投影安装 **`router-rs framework host-integration install --to claude-desktop`**（`install_tool` 名 **`claude-desktop`**）。两者在 Rust 里共享 host-neutral stdio-agent hook 协议实现，但 host id、投影路径（`.claude/*` / `.claude-desktop/*`）、环境变量、gate token 与 `router_rs_observation.host` 必须保持独立。

**单行指针**：跨 Cursor 工作区接入与操作步骤见 [`docs/hosts/cursor.md`](hosts/cursor.md)；仓库级一键接入见根 [`README.md`](../README.md) →「其它仓库一键接入」「建议自检命令序列」（约 L147–L192）。**其它宿主操作手册（≤1 页）**：[`docs/hosts/codex-cli.md`](hosts/codex-cli.md)、[`docs/hosts/claude.md`](hosts/claude.md)。

**Cursor 投影 scope**：`framework.mdc` 与 browser MCP 规则仅安装到 **user** scope（`~/.cursor/rules/`）；仓库内保留 `.cursor/hooks.json` 与项目级 harness 配置。`framework maint refresh-host-projections` 对 `cursor` 使用 `--scope user`，其它 installable 宿主仍为 `project`。

---

## 1. Portable core（宿主无关可复用面）

以下条件满足时，新宿主只需提供 **薄适配层**（转发 stdin/JSON、超时、路径解析），无需复制门控算法或业务 prose：

| 区域 | 内容 | 真源 / 约定 |
|------|------|----------------|
| L2 | 连续性工件、`EVIDENCE_INDEX`、`GOAL_STATE`、`SESSION_SUMMARY` 等 | `artifacts/current/`、`docs/harness_architecture.md` §1–§3 |
| L3 CLI | `router-rs framework snapshot|contract-summary|hook-evidence-append|…`、`closeout`、`task-state-*` | `docs/rust_contracts.md`、`RUNTIME_REGISTRY.json` → `framework_commands` |
| 共用门控启发式 | review / delegation / reject_reason / normalize_tool 等纯函数 | `scripts/router-rs/src/hook_common.rs` |
| Cursor PostTool → shell evidence 形状 | [`hook_posttool_normalize.rs`](../scripts/router-rs/src/hook_posttool_normalize.rs) → `framework_runtime::try_append_post_tool_shell_evidence` | 字段抽取 helper 仍在 [`cursor_hooks/`](../scripts/router-rs/src/cursor_hooks/mod.rs)（含 `tool_name_of` 等） |
| Cursor review/subagent stdin 流水线 | stdin JSON → `dispatch_cursor_hook_event` → stdout JSON | `scripts/router-rs/src/review_gate.rs` + [`cursor_hooks/`](../scripts/router-rs/src/cursor_hooks/mod.rs) |

**反模式**：在 L4 shell/bash 里复制 L3 正则门控、`EVIDENCE_INDEX` 拼写规则或 RFV/G goal 拼接逻辑——应调用已有子命令或由 hook 二进制统一处理。

---

## 2. 事件 → `router-rs` CLI 对照（摘要）

以下内容只列 **入口与磁盘副作用字段名级别** 指针；细则见 [`harness_architecture.md`](harness_architecture.md) §3、「主数据流」与各宿主 `hooks.json`。

### Codex（`hooks.json`）

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| 会话连续性 / digest / PostTool、`CODEX_REVIEW_GATE` | 配置项指向 `router-rs codex hook …` | `codex hook`（`codex_hooks.rs`） | `EVIDENCE_INDEX`、`FRAMEWORK_DIGEST` / session 工件等（以 hook 分支为准）；默认可清点深度审稿 lane 见 [`harness_architecture.md`](harness_architecture.md) **§5.0** |
| **Codex hook stdout** | 任一 hook 进程退出 0 | `dispatch_codex_command` → `codex_hook_stdout_payload` | **始终**打印单行紧凑 JSON；无附带输出时为 **`{}`** |
| **Codex Stop × `.codex/hook-state`** | Stop 事件 | `handle_codex_stop` | 状态文件缺失：不据此拦截；状态不可读（损坏 JSON / IO）：**fail-closed**，`followup_message` 含 `CODEX_HOOK_STATE_UNREADABLE` |
| 宿主入口对齐 | `router-rs codex sync` | 经 shared `host_entrypoint_sync` engine + Codex provider 生成 `.codex/hooks.json`、`AGENTS.md` 等及 **`host_entrypoints_sync_manifest`** | 受 **`RUNTIME_REGISTRY.host_targets.supported`** 约束 |

### Cursor（`.cursor/hooks.json`）

**默认注册 7 事件**（2026-05-20 减法闭集）：`beforeSubmitPrompt`、`stop`、`sessionStart`、`sessionEnd`、`postToolUse`、`subagentStart`、`subagentStop`。已移除：`afterAgentResponse`、`beforeShellExecution`/`afterShellExecution`、`afterFileEdit`、`preCompact`（恢复见 [`MIGRATION.md`](../MIGRATION.md)）。项目 env：[`.cursor/router-rs-hook.env`](../.cursor/router-rs-hook.env)；`postToolUse` 对非门控工具走 **fast-path**（[`post_tool_use_needs_work`](../scripts/router-rs/src/cursor_hooks/handlers.rs)）。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| Review / subagent 门控、beforeSubmit/Stop | `router-rs cursor hook <event>` | `review_gate::run_review_gate` → `dispatch_cursor_hook_event` | `.cursor/hook-state/review-subagent-*.json`（及策略合并字段，见运行时）；Stop 上 `REVIEW_GATE` 重复硬提示上限见 **`ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NAGGES`**（[`harness_architecture.md`](harness_architecture.md) §5 表） |
| 续跑类合并 | Same | [`cursor_hooks/`](../scripts/router-rs/src/cursor_hooks/mod.rs) + `autopilot_goal` / `rfv_loop` | `additional_context` / `followup_message`（宿主 JSON 出站字段） |
| **`ROUTER_RS_OPERATOR_INJECT` × SessionStart** | SessionStart | `cursor_hooks`（`handle_session_start`） | 与 Codex 对称：闸关则不写入连续性 `additional_context`；闸开则复用 `framework_runtime::continuity_digest` 与 Codex 同源段落；出站超长时与 review gate 等路径一致追加 `...[~trunc]`（细则见 [`harness_architecture.md`](harness_architecture.md) §2.1） |
| **运维自检** | 手工排障 | `router-rs framework doctor --repo-root <repo>` | 生成物为 **metadata-only** `generated-artifacts-status`（不跑慢 generator）；`ROUTER_RS_TASK_LEDGER_FLOCK` 关闭时打印醒目 WARN（见 harness §2.3 / §3.1） |

**Cursor 排障（短）**：

- **`fork_context` 缺省（Cursor）**：默认 **`ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` 开启**时，可数深度 lane 且字段缺失可推断为 `false`；关闭后与 Codex 一致——**仅**可解析为布尔 `false` 时计独立证据。显式 `fork_context: true` 永不算。见 [`cursor_review_independent_fork`](../scripts/router-rs/src/review_gate_engine.rs) 与 harness **§5.0**。
- **磁盘 `GOAL_STATE` 与 pre-goal**：若需收紧「仅凭盘上 GOAL 即在 beforeSubmit 视同 pre-goal 已满足」的信任边界，见 [`harness_architecture.md`](harness_architecture.md) §5 中 `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK`。
- **`cursor-router-rs-hook.sh` 与 exit code**：对 **critical** 事件（如 beforeSubmit/Stop/postToolUse/subagentStart/subagentStop）在 `router-rs` 缺失时 **fail-closed**；其余事件 **fail-open**（stderr 提示 + exit 0）。仅看 exit code 时可能漏判 SessionStart 等是否实际注入了上下文。
- **仿宿主续跑行**：若在聊天区看到 `RG_FOLLOWUP` 等**无 `router-rs ` 前缀**的仿机读行，勿当 hook 真源；以 hook stdout JSON 为准，说明见 [`harness_architecture.md`](harness_architecture.md) **§4.3**。
- **清门粘贴**：**不要**把 **`RG_FOLLOWUP`…** 当清门令牌粘贴；[`saw_reject_reason`](../scripts/router-rs/src/hook_common.rs) 不因该前缀清门。请用 **`rg_clear`**、拒因 token，或自然语言 override；与 [framework_operator_primer.md](framework_operator_primer.md)「粘贴清门」一致。

### Claude Code（`router-rs claude hook`）

**默认注册 4 事件**（减法闭集，与 Cursor 7 事件不同宿主能力）：`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`。项目 env：[`.claude/router-rs-hook.env`](../.claude/router-rs-hook.env)（模板 [`configs/framework/claude-router-rs-hook.env`](../configs/framework/claude-router-rs-hook.env)）；launcher **release 优先** 同 Cursor（[`claude-router-rs-hook.sh`](../configs/framework/claude-router-rs-hook.sh)）。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| PreTool / Stop 守卫、settings 变更提示 | 宿主 hooks 调用 `router-rs claude hook --event=PreToolUse|Stop|…` | `claude_hooks.rs` | `.claude/review_gate_*.json`、`hook_state_*.json`（Cursor 指纹 payload 静默忽略）；出站 Claude hook JSON |
| **Claude Stop × `.claude` 状态 JSON** | Stop | `claude_hooks::run_stop` | `review_gate_*.json` / `hook_state_*.json` 缺失不单独拦截；**已存在但不可读或损坏**：**fail-closed**，`stopReason` 含 `CLAUDE_HOOK_STATE_UNREADABLE`（与 Codex `CODEX_HOOK_STATE_UNREADABLE` 同形排障） |
| 投影规则与 hook 绑定 | `router-rs framework host-integration install --to claude` | `host_integration.rs` | `.claude/rules/framework.md`、`.claude/settings.json`（`PreToolUse` / `UserPromptSubmit` / `PostToolUse` / `Stop`）、`.claude/.framework-projection.json`（project scope） |

### Claude Desktop（`router-rs claude-desktop agent`）

**能力边界**：无 CLI 级 PreToolUse / Stop 硬拦截（registry `harness_capability_exceptions`）；门控靠 **MCP 工具工作流** + 短投影文案。勿与 Claude Code 的四事件 hook 表混读。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| MCP 工具工作流 | Desktop MCP stdio | `router-rs` MCP server（见项目 `.claude/CLAUDE.md`） | `artifacts/current/` 与 Code 共用；`goal_state_manage` / `closeout_gate` / `framework_digest` |
| 投影安装 | 一次性接入 | `router-rs framework host-integration install --to claude-desktop` | 项目 `.claude/CLAUDE.md`（短指针）、`.mcp.json`；**不**写入 `.claude/settings.json` hook 四事件 |
| 操作手册 | — | — | [`docs/hosts/claude-desktop.md`](hosts/claude-desktop.md) |

**统一原则**：宿主配置中的命令应保持 **短命 + 超时**；语义在 Rust，不在宿主脚本里分支业务规则。

---

## 3. 新宿主接入 Checklist

**路径表**（按职责；新宿主通常需各改一处或并列扩展）：

| 职责 | 仓库路径 |
|------|----------|
| Cursor hook 语义与出站 JSON | [`cursor_hooks/mod.rs`](../scripts/router-rs/src/cursor_hooks/mod.rs) |
| Codex hook 语义 | `scripts/router-rs/src/codex_hooks.rs` |
| shared host entrypoint sync engine | `scripts/router-rs/src/host_entrypoint_sync.rs` |
| Codex host entrypoint provider | `scripts/router-rs/src/codex_hooks.rs` |
| Claude Code / Claude Desktop stdio-agent hook 语义（stdin JSON） | `scripts/router-rs/src/claude_hooks.rs`（共享实现；公开入口为 `router-rs claude hook` / `router-rs claude-desktop agent`） |
| `framework host-integration install`、投影 manifest、入口模板 | `scripts/router-rs/src/host_integration.rs` |
| CLI 子命令注册与 `framework`/`cursor`/`codex`/`claude`/`claude-desktop` 分发 | `scripts/router-rs/src/cli/dispatch.rs`（及 `cli/dispatch_body.txt`） |
| 宿主侧事件绑定 | 仓库根 `.cursor/hooks.json`；Codex 侧 `.codex/hooks.json`（由 sync/install 写入）；Claude 侧 `.claude/settings.json`、Claude Desktop 侧 `.claude/settings.json`（由 host integration 写入） |
| 闭集宿主 id 与 `install_tool` / `host_entrypoints` | `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported` 与 `host_targets.metadata` |
| `review_gate` 磁盘 loader | `scripts/router-rs/src/registry_loader.rs` |
| GSD/review 安装文案 | `configs/framework/host_projection_narrative.json` |
| 生成物 drift manifest | `configs/framework/GENERATED_ARTIFACTS.json` |

1. **在 `RUNTIME_REGISTRY.json`** 扩展 `host_targets.supported`、`host_targets.metadata.<host>.install_tool` 与 `host_targets.metadata.<host>.host_entrypoints`（及若需要，`host_projections.*`）；`framework_host_targets.rs` 必须只从注册表读取这些值，并补齐 fail-closed 单测。
2. **薄 hook**：仅解析 workspace root / repo root → 组装 `router-rs …` argv；stdin 透传钩子 JSON。
3. **验证 L2**：在真实任务指针下跑一次验证类命令，确认 `EVIDENCE_INDEX` / `NEXT_ACTIONS` 等可按现有 schema 写入。
4. **再接入安装/投影**：`framework host-integration install --to …` 路径应注册到宿主专用安装函数（与其它宿主并列，`match`/`factory` 收敛在宿主集成模块内）。
5. **文档**：先更新本节或 `rust_contracts.md` Host 小节，再合入大范围行为改动（与 [`harness_architecture.md`](harness_architecture.md) **§7** 文末维护说明一致）。

**当前边界**：`host_targets.supported` 已包含 Codex / Cursor / Claude Code / Claude Desktop 闭集宿主；后续新增宿主仍必须先改 [`RUNTIME_REGISTRY.json`](../configs/framework/RUNTIME_REGISTRY.json)，并按本清单补齐 adapter、hook、投影与测试。不要添加无 adapter / 无验证的占位宿主 id。

### 3.1 可复制执行清单（工程顺序）

下列路径均为 **仓库根相对**；按顺序勾选可减少漏改。新宿主 CLI 约定仍为：**stdin JSON → Rust 结构化处理 → stdout JSON**（与现有 `cursor_hook` / `codex hook` 入口一致）。

**PR 顺序（建议单行记忆）**：`RUNTIME_REGISTRY`（及测试夹具中的最小 registry）→ `framework_host_targets` → `<host>_hooks` + `main` mod → `dispatch`（含 `dispatch_body.txt`）→ `host_integration` → L4 `hooks.json` → `tests/host_integration` / `tests/policy_contracts`。

**本仓库日常「接满」技能与路由（开发者在改 skill / registry 后）**：`router-rs framework skills validate --repo-root "$PWD"`（可选 `framework skills refresh --write`）→ `router-rs framework sync-entrypoints --repo-root "$PWD"` → 按需 `router-rs framework host-integration install --to <codex|cursor|claude>`（`installable` 宿主）。

| 阶段 | 主要落点 |
|------|----------|
| 注册表 / 契约 | `RUNTIME_REGISTRY.json`，[`tests/common/mod.rs`](../tests/common/mod.rs)，[`framework_host_targets.rs`](../scripts/router-rs/src/framework_host_targets.rs) |
| L3 入口 | 新增 `scripts/router-rs/src/<host>_hooks.rs`（命名对齐 [`codex_hooks.rs`](../scripts/router-rs/src/codex_hooks.rs) / [`cursor_hooks.rs`](../scripts/router-rs/src/cursor_hooks.rs)），[`main.rs`](../scripts/router-rs/src/main.rs) |
| CLI 分发 | [`dispatch_body.txt`](../scripts/router-rs/src/cli/dispatch_body.txt)，[`dispatch.rs`](../scripts/router-rs/src/cli/dispatch.rs) |
| 安装 / 投影 | [`host_integration.rs`](../scripts/router-rs/src/host_integration.rs)，[`GENERATED_ARTIFACTS.json`](../configs/framework/GENERATED_ARTIFACTS.json)（若新增生成物） |
| L4 + 验证 | 宿主 `hooks.json`，[`tests/host_integration.rs`](../tests/host_integration.rs)，[`tests/policy_contracts.rs`](../tests/policy_contracts.rs) |

- [ ] **[`configs/framework/RUNTIME_REGISTRY.json`](../configs/framework/RUNTIME_REGISTRY.json)**：在 `host_targets.supported` 追加宿主 id；补齐 `host_targets.metadata.<id>.install_tool` 与 `host_entrypoints`（字符串或 JSON 数组形状需与现网 `codex-cli` / `cursor` **对称**，避免半套映射）；若改动 `framework_commands` 等按宿主分列的表，逐键对照现有列。
- [ ] **[`tests/common/mod.rs`](../tests/common/mod.rs)**（及任何内嵌最小 registry 的测试夹具）：与真实 `RUNTIME_REGISTRY.json` 的 `host_targets` 块对齐，避免 CI 用「缩水 registry」与真源分叉。
- [ ] **[`scripts/router-rs/src/framework_host_targets.rs`](../scripts/router-rs/src/framework_host_targets.rs)**：确保只从注册表读取上述字段，fail-closed；必要时补充单元测试。
- [ ] **新增** `scripts/router-rs/src/<host>_hooks.rs`（命名对齐现有 [`codex_hooks.rs`](../scripts/router-rs/src/codex_hooks.rs) / [`cursor_hooks.rs`](../scripts/router-rs/src/cursor_hooks.rs)）：实现各生命周期分支；在 [`main.rs`](../scripts/router-rs/src/main.rs) 注册 `mod` 并导出入口。
- [ ] **[`scripts/router-rs/src/cli/dispatch_body.txt`](../scripts/router-rs/src/cli/dispatch_body.txt)** 与 [`scripts/router-rs/src/cli/dispatch.rs`](../scripts/router-rs/src/cli/dispatch.rs)：挂上 `router-rs <host> hook <event> …` 分发（与现有 `codex` / `cursor` 子命令并列）。
- [ ] **[`scripts/router-rs/src/host_integration.rs`](../scripts/router-rs/src/host_integration.rs)**：`framework host-integration install --to <tool>` 能解析注册表中的 `install_tool`；为该宿主增加投影写入（对标 `render_cursor_framework_entrypoint` / `render_codex_framework_entrypoint`），GSD/review 段落从 [`host_projection_narrative.json`](../configs/framework/host_projection_narrative.json) 读取；若产生新的生成物路径，同步 [`configs/framework/GENERATED_ARTIFACTS.json`](../configs/framework/GENERATED_ARTIFACTS.json)（及代码中 `REQUIRED_GENERATED_ARTIFACTS` 等常量，若有）。
- [ ] **L4 样例**：检出中的 [`.cursor/hooks.json`](../.cursor/hooks.json)（Cursor）与由 sync 写入的 `.codex/hooks.json`（Codex）应保持 **argv + 超时 + stdin 透传**，不在 shell 内复制 L3 业务分支；新宿主应对照新增同级配置。
- [ ] **[`tests/host_integration.rs`](../tests/host_integration.rs)**：增加 dry-run 或临时目录安装断言（沿用现有 `host_targets.metadata` / manifest 断言模式）。
- [ ] **[`tests/policy_contracts.rs`](../tests/policy_contracts.rs)**（根包）：registry / 契约回归。
- [ ] **验证**：`cargo test --manifest-path scripts/router-rs/Cargo.toml`；仓库根 `cargo test`。
- [ ] **[`AGENTS.md`](../AGENTS.md)**：若新宿主属于 Codex 投影链且涉及策略快照与 **`codex sync`** 生命周期，在「权威分层」表补一句说明（避免第二叙事真源；与现有 Codex 编译期嵌入段落对齐）。

### 3.2 Maint / supervisor / profile 硬编码宿主耦合盘点

以下为 **盘点结论**：新增或强化宿主能力时优先扩展对应 `match` / 维护序列，或在 [`RUNTIME_REGISTRY.json`](../configs/framework/RUNTIME_REGISTRY.json) / 文档中写明能力降级，避免 silent drift。

| 位置 | 硬编码内容 | 建议 |
|------|------------|------|
| [`scripts/router-rs/src/framework_maint.rs`](../scripts/router-rs/src/framework_maint.rs) | `refresh_host_projections` 从 `RUNTIME_REGISTRY` 派生 installable host projection tools；`codex-app` 这类 runtime-supported / non-installable host 不进入安装遍历 | 新增宿主时必须提供 maint verifier，或在 registry 中标记 `installable=false` 并给出 unsupported reason |
| [`scripts/router-rs/src/session_supervisor.rs`](../scripts/router-rs/src/session_supervisor.rs) | `classify_rate_limit_block` 仅接受 `codex` / `codex-cli`；`build_driver_command` 仅组装 Codex CLI；`driver_id_for_host` 非 codex 映射为 `unknown_driver` | 当前明确为 **Codex driver only**；为新 CLI 宿主补 driver / 限速模式前，registry 必须继续标记 `session_supervisor_driver=unsupported` |
| [`scripts/router-rs/src/framework_profile.rs`](../scripts/router-rs/src/framework_profile.rs) | **已收敛**：`build_profile_bundle` 从 `RUNTIME_REGISTRY.host_projections` 派生 `host_payloads`；保留 `codex_profile` / `full_codex_profile` 作为 Codex 兼容输出 | 新宿主若需要 profile payload，优先补 `host_projections` 与 contract tests；不要新增 Codex-only profile compiler 分支 |
| [`scripts/router-rs/src/hook_posttool_normalize.rs`](../scripts/router-rs/src/hook_posttool_normalize.rs) | Cursor `postToolUse` stdin → `try_append_post_tool_shell_evidence` 形状的 crate 级归一化（依赖 `cursor_hooks` 字段抽取 helper） | Codex 仍直连 append；terminal / rust-lint 等 Cursor 专有分支仍在 `cursor_hooks` |

### 3.3 Host entrypoint sync engine / provider 边界

[`host_entrypoint_sync.rs`](../scripts/router-rs/src/host_entrypoint_sync.rs) 只负责通用 sync engine：比较 provider 产出的文件、写入 manifest、同步匹配 worktree、汇总 `written` / `would_write` / `unchanged` 等报告字段。Codex 私有 payload 构建（`.codex/hooks.json`、`.codex/README.md`、`AGENTS.md` bootstrap）与 Codex skill surface 刷新由 [`codex_hooks.rs`](../scripts/router-rs/src/codex_hooks.rs) 中的 `codex provider` 负责。`router-rs codex sync` 是兼容 CLI 名称，不表示 shared sync engine 属于 Codex hook 模块。

同步范围语义：当前 root 使用 `full_sync`，会物化 text + JSON 入口；匹配到的其它 git worktree 只使用 `partial_sync`，只同步 JSON hook/manifest，不覆盖 `AGENTS.md` 或 `.codex/README.md` 等本地策略文本。

Projection install/status/remove 仍保留 `HostProjectionAdapter` thin adapter 表：registry 负责闭集、`projection_status`、`installable` 与 install tool 关系，adapter 表只负责宿主专用写盘、状态检查、删除和 HOME 解析。不新增第二套 host provider 框架；`RUNTIME_PROVIDER_REGISTRY` 中的 host provider lane 只作目录/报告面，不能驱动安装。

### 3.4 PostTool / 终端证据归一化

Codex 侧 [`codex_hooks.rs`](../scripts/router-rs/src/codex_hooks.rs) 中 `PostToolUse` 直接将原生事件传入 [`try_append_post_tool_shell_evidence`](../scripts/router-rs/src/framework_runtime/mod.rs)；Cursor 侧先用 crate 级 [`hook_posttool_normalize.rs`](../scripts/router-rs/src/hook_posttool_normalize.rs) 中 `synthetic_post_tool_evidence_shape` 将异构 `postToolUse` 合成 **同一 evidence 解析形状**，再调用同一 API；[`cursor_hooks/`](../scripts/router-rs/src/cursor_hooks/mod.rs) 仍承担终端归属、`rust-lint` 等 Cursor 专用分支。**共享 append 仍在 `framework_runtime`**；归一化模块仅收敛「stdin → append 形状」边界，字段抽取 helper 仍在 `cursor_hooks`（避免 crate 模块循环前提下保持单一合成入口）。**`hook_common::tool_input_value_from_map`** 对**单层** JSON object 的工具入参合并键优先级固定为：`tool_input` → `input` → `arguments` → `parameters`（同键竞争时按此顺序取第一个非缺失键；Cursor 对 `HOOK_EVENT_NESTED` 路径仍重复该规则扫描嵌套对象）。

---

## 4. 与五层模型的对齐（L4 / L5）

与 [`harness_architecture.md`](harness_architecture.md) §1–§2 一致：

| 层 | 允许 | 禁止 |
|----|------|------|
| **L4** | 调用 `router-rs` 子命令、固定超时、环境透传 | 长段策略 prose、复制 L3 门控、手写 `EVIDENCE_INDEX` 规则 |
| **L5** | SKILL 契约、`verify_commands`、拒因枚举、编排叙事 | 第二套连续性目录、或与 L2 schema 冲突的并行真源 |

L3（`cursor_hooks` / `codex_hooks` / `framework_runtime` 等）负责合并续跑提示、采样 PostTool、持久化 gate 状态；**不得**承载领域产品长篇文案（应进配置文件或 L5 文档）。

---

## 5. 可选演进（非本轮承诺）

若未来需要将「仅 portable core」独立发布为多 crate workspace，可把 `hook_common` + 无宿主 IO 的路径进一步拆仓；当前单 crate `router-rs` 仍为默认形态。
