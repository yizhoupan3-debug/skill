---
last_verified: "2026-06-09"
depends_on:
  - harness_architecture/index.md
  - rust_contracts.md
---

# 宿主适配契约（Host adapter contract）

本文件描述 **新宿主如何接入** 本仓库的连续性 harness 与 `router-rs` 控制面：哪些能力可移植复用、L4/L5 边界，以及工程清单。**Hook 事件 → CLI → 写盘** 矩阵已下沉至各 [`docs/hosts/`](hosts/) 手册 **Hook 事件矩阵** 节。

**权威列表**：`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported` 为 **当前闭集宿主 id**；安装/状态/卸载与 Codex **`host_entrypoints_sync_manifest`** 的 `supported_hosts` / `host_entrypoints` 由同一注册表推导（实现见 `router-rs` 中 `framework_host_targets`）。

**历史字段**：`host_targets.entrypoint_files` 已从注册表移除；宿主策略入口与 sync manifest 入口集合以 `host_targets.metadata.<host>.host_entrypoints` 为唯一权威（`framework_host_targets::host_entrypoints_value_for_id`）。

**相关契约**：[`rust_contracts.md`](rust_contracts.md) · [`harness_architecture/`](harness_architecture/index.md)。

## 快速路径（我要接新宿主）

- **先读**：[`harness_architecture/03-hook-and-switches.md`](harness_architecture/03-hook-and-switches.md) **§5** 与 [`harness_architecture/04-closeout-and-depth.md`](harness_architecture/04-closeout-and-depth.md) **§6**，再读本文件 **§0** 与 **[§3.1](#31-可复制执行清单工程顺序)**。
- **再改**：`RUNTIME_REGISTRY` → hooks 模块 → `dispatch` → `host_integration` → L4 → 测试（详见 §3.1）。
- **跑测**：`cargo test --manifest-path core/router-rs/Cargo.toml`；改动根 [`tests/`](../tests/) 时再 `cargo test`。
- **入口同步**：`router-rs framework sync-entrypoints --repo-root "$PWD"`。

---

## 0. North-star contract 与维护地图

**解耦**：宿主差异只允许停留在 L4 适配壳与 `RUNTIME_REGISTRY.host_targets` 元数据；L2 工件 schema、L3 CLI 行为、门控/证据追加逻辑必须复用 Rust owner。**无感**：用户在 **任一直持闭集宿主**上得到同一套 harness 连续性、证据、closeout 与热路由闭集语义。

**非目标**：不把每个宿主做成独立 runtime fork；不在 hook shell、`.mdc` 或 skill prose 中复制 L3 决策。

**双文件注入（硬约束）**：各闭集宿主须**同时**注入仓库根 **`AGENTS.md`**（跨宿主内核）与 **`AGENTS_<HOST>.md`**（transport delta）；**禁止**合并为单文件。`AGENTS_<HOST>.md` 仅 transport 差异，**不**重复内核 policy（路由、Closeout、Execution Ladder 等仍以 `AGENTS.md` 为准）。Codex 为编译期 concat embed；其余宿主为仓库根双文件 + 各宿主投影（framework rule / MCP / hook）。

## 0.1 Hook 契约（Claude Code canonical）

**策略真源**：lane 判定、fork 证据、Stop 清门规则在 **`core-policy`**（[`review_gate_engine.rs`](../core/core-policy/src/review_gate_engine.rs)、[`hook_common`](../core/core-policy/src/hook_common.rs)）；**Claude Code hook 语义为跨宿主 canonical**。Cursor / Codex / MCP 宿主**不得**在 L4 复制第二套清门逻辑——仅投影出站 JSON（字段名、`followup_message` / `stopReason` 前缀、事件观测路径）。

### Claude-canonical review gate（hook 宿主）

| 阶段 | 规则（`core-policy`） |
|------|----------------------|
| 武装 | `review_required`（review 信号且非 My 执行区入口）且未 `review_override` |
| 可数证据 | PostTool / subagent 事件：`reviewer_lanes` + `review_independent_reviewer_evidence`（`fork_context=false` 或显式 enable 缺省推断）→ `independent_reviewer_seen` |
| Stop 清门 | `review_gate_satisfied` ⇔ override **或** `independent_reviewer_seen`（**不**要求 phase≥3、multiset settle、compact-only bump） |
| Stop 出站 | **advisory-only**：`review_gate_blocks_stop` 仅决定是否注入单行 nudge；**不** `permission: deny` / `decision: block` / `continue: false` |
| spawn-first | registry `spawn_first_enabled` + `spawn_first_nudge_by_host`；**my-light** suppress |
| 窄范围 | `is_narrow_review_prompt` → 不武装 `review_required` |

**Registry 真源**（[`configs/framework/RUNTIME_REGISTRY.json`](../configs/framework/RUNTIME_REGISTRY.json)，[`registry_review_gate.rs`](../core/core-policy/src/registry_review_gate.rs)，ADR-005）：

| 字段 | 消费宿主 | 含义 |
|------|----------|------|
| `review_gate.reviewer_lanes` | **全部 hook/MCP 宿主** | 可数独立审稿 lane 闭集（Claude Code canonical）：`general-purpose`、`best-of-n-runner`、`deep-reviewer`、`review`、`reviewer`、`critic`、`code-review` 及归一化等价名 |

**跨宿主不变量（2026-06）**：**`REVIEW_GATE` 在 Stop 上全局 advisory-only**——任何宿主均不得以 review gate 硬拦 Stop。Hook 宿主在 `followup_message` 等字段注入 `router-rs REVIEW_GATE` / `CODEX_REVIEW_GATE` / `CLAUDE_REVIEW_GATE` 单行提示；MCP 宿主在 `closeout_gate` / `goal_state_manage` 路径报告 review 缺口（`ADVISORY`）。**Closeout 硬门禁**与 review gate 分层：`ROUTER_RS_CLOSEOUT_ENFORCEMENT`、completion claim guard、PreToolUse 生命周期门控等可仍 fail-closed，见 [`harness_architecture/04-closeout-and-depth.md`](harness_architecture/04-closeout-and-depth.md)。

增删 lane **只改 registry `reviewer_lanes`**；重启 hook 子进程即可。dispatch 须绑定 `repo_root`（`HookRegistryRepoGuard`）。

| 字段 | 含义 |
|------|------|
| `review_gate.spawn_first_enabled` | 默认 true；false 时跳过 spawn-first nudge（清门阈值不变） |
| `review_gate.spawn_first_nudge` | 全局回退文案 |
| `review_gate.spawn_first_nudge_by_host` | 按宿主一行配对审稿 |

### 按 registry 名接入 review gate（四步）

1. **`RUNTIME_REGISTRY.json`**：在 `host_targets.supported` 加入宿主 id；`review_gate.spawn_first_nudge_by_host.<id>` 可选一行文案；**不必**复制 lane 列表（共用 `reviewer_lanes`）。
2. **L4 hook/MCP 模块**：PostTool/subagentStart 调用 `review_independent_reviewer_evidence` + `is_reviewer_lane_from_registry`；Stop 调用 `review_gate_satisfied` / `review_gate_blocks_stop` 判定是否须 nudge，**仅**投影 advisory 文案（不硬拦 Stop）。
3. **`dispatch.rs`**：注册 CLI 入口；dispatch 路径包 `HookRegistryRepoGuard::new(repo_root)`。
4. **测试**：`assert_reviewer_lane_matrix` + 宿主 integration 一条 PostTool→Stop  happy path。

Canonical API（`core-policy`）：`is_reviewer_lane_from_registry`、`review_independent_fork`、`review_independent_reviewer_evidence`、`review_gate_satisfied` / `review_gate_blocks_stop`；registry 加载真源仅 [`registry_review_gate.rs`](../core/core-policy/src/registry_review_gate.rs)。

**窄范围 skip**（四宿主）：`hook_common::is_narrow_review_prompt` → 不武装 `review_required`。

### 跨宿主差异（operator · transport / 遥测 only）

清门语义上表 **Claude-canonical** 一行通吃 hook 宿主；下表**仅**列 transport 与 operator 面，**不**构成第二套 Stop 规则。

| 能力 | Cursor | Codex | Claude Code | Antigravity | OpenCode |
|------|--------|-------|-------------|-------------|----------|
| transport | shell hook | shell hook | shell hook（**canonical 参考实现**） | MCP stdio | MCP stdio |
| 可数 reviewer lane 真源 | `reviewer_lanes` | `reviewer_lanes` | `reviewer_lanes` | `review-lanes/*.md` + skill | `review-lanes/*.md` + skill |
| 子代理观测路径 | PostTool + `subagentStart`/`Stop` | PostTool（v0.133+ 可有 Start/Stop **遥测**） | PostTool | MCP 自检 | MCP 自检 |
| 宿主专有遥测（**非**清门条件） | `review_subagent_pending_cycle_keys` multiset / review-lite vec | `subagent_start_count`（phase bump 辅助） | — | — | — |
| Stop nudge 文案前缀 | `router-rs REVIEW_GATE incomplete` | `router-rs CODEX_REVIEW_GATE incomplete` | `router-rs CLAUDE_REVIEW_GATE incomplete` | MCP `ADVISORY` | MCP `ADVISORY` |
| 用户 prompt override 粘贴面 | `reject_reason` / `rg_clear` | 同左 | **无**（须完成 lane 或自然语言 override） | — | — |
| Review gate 关闭 / 抑制 | canonical `ROUTER_RS_REVIEW_GATE_DISABLE`；legacy `ROUTER_RS_{CURSOR,CODEX,CLAUDE}_*`；`my-light` suppress | 同左 | 同左 | `my-light`；MCP advisory | `my-light`；MCP advisory |
| Goal/RFV hook 续跑 | ✗ | ✗ | ✗ | ✗ | ✗ |

**MCP vs hook（transport）**：hook 宿主经 `hooks.json` 子进程写 `followup_message` / `decision` 等 JSON；MCP 宿主经 `router-rs-framework` 的 `closeout_gate` / `goal_state_manage` 返回结构化 verdict（review 缺口为 advisory，closeout 硬拦见 `closeout_evidence_hooks` exception 与 `ROUTER_RS_CLOSEOUT_ENFORCEMENT`）。

细节见 [`harness_architecture/03-hook-and-switches.md`](harness_architecture/03-hook-and-switches.md) §5.0 与各 [`docs/hosts/*.md`](hosts/)。

- **Harness hook 面矩阵**：`host_projections.*.harness_capabilities` 须满足 [`RUNTIME_REGISTRY_SCHEMA.json`](../configs/framework/RUNTIME_REGISTRY_SCHEMA.json) 的 `harness_capability_policy`；例外须声明 `harness_capability_exceptions`（`tests/policy_contracts.rs` **`runtime_registry_host_projections_split_harness_capabilities`** 机读校验）。
- **热路由宿主展开**：`SKILL.md` 缺省 `platforms` 时 `framework skills refresh` 展开为 `host_targets.supported` 全量；与 hook 级 harness 能力矩阵**分工**，勿混读。
- **NL suppress/boost**：[`NL_ROUTE_ADJUSTMENTS.json`](../configs/framework/NL_ROUTE_ADJUSTMENTS.json)；跨查询 marker：[`ROUTING_SIGNAL_MARKERS.json`](../configs/framework/ROUTING_SIGNAL_MARKERS.json)。

**宿主集合术语**：`host_targets.supported` = 全局闭集 id；`host_projections` = 可生成 profile payload 的宿主；`framework_commands.*.host_entrypoints` = 显式命令入口；`SKILL_PLUGIN_CATALOG.json` `host_support.platforms` = skill body 可用宿主。

| 维护面 | 不变量（共享） | 变量（宿主元数据） |
|--------|----------------|-------------------|
| L2 连续性 | `artifacts/current`、`EVIDENCE_INDEX`、`GOAL_STATE` schema | 指针路径由 repo root 解析 |
| 事件 × CLI × L2 | 宿主事件调用 `router-rs`，验证类命令追加同一 evidence 形状 | 事件字段归一化（见各宿主 Hook 矩阵） |
| 宿主安装/入口 | 闭集 id 来自 `host_targets.supported` | `install_tool`、`host_entrypoints` |
| L4 / L5 边界 | L4 只做 argv/stdin/超时/路径转发 | `.mdc`、`AGENTS.md` 投影形状不同 |

**闭集宿主**（`host_targets.supported`，2026-06）：`codex`、`cursor`、`claude-code`、`antigravity`、`opencode`（共 5 个）。手册：[`codex.md`](hosts/codex.md)、[`cursor.md`](hosts/cursor.md)、[`claude.md`](hosts/claude.md)、[`antigravity.md`](hosts/antigravity.md)、[`opencode.md`](hosts/opencode.md)。退役 stub：[`claude-desktop.md`](hosts/claude-desktop.md)、[`codex-cli.md`](hosts/codex-cli.md)、[`antigravity-app.md`](hosts/antigravity-app.md)、[`antigravity-cli.md`](hosts/antigravity-cli.md)。

**Cursor 投影 scope**：`framework.mdc` 仅 **user** scope（`~/.cursor/rules/`）；项目保留 `.cursor/hooks.json`。

**为何 L4 仍按宿主命名**：「去宿主化」指 review/closeout 等策略沉入 `core-policy`（L2），而非把各宿主的 stdin 信封、状态目录与出站 JSON 压进单文件。L4 模块名对齐 `RUNTIME_REGISTRY` 的 `host_id`（如 `claude-code` → `claude_code_hooks`），与 `mcp_stdio_harness` 按 MCP 宿主分层同理；transport 层合并（Epic B）是后续工作，不在单会话内强行合一。

---

## 3. 新宿主接入 Checklist

| 职责 | 仓库路径 |
|------|----------|
| Cursor hook | [`cursor_hooks/mod.rs`](../core/host-projection/src/hosts/cursor_hooks/mod.rs) |
| Codex hook | [`codex_hooks/mod.rs`](../core/host-projection/src/hosts/codex_hooks/mod.rs) |
| Claude Code hook | [`claude_code_hooks.rs`](../core/host-projection/src/hosts/claude_code_hooks.rs) |
| Antigravity MCP | `antigravity` agent / MCP 工具层 |
| OpenCode MCP | `opencode` agent / MCP 工具层 |
| entrypoint sync | [`host_entrypoint_sync.rs`](../core/runtime-core/src/host_entrypoint_sync.rs) |
| install / 投影 | [`host_integration/mod.rs`](../core/runtime-core/src/host_integration/mod.rs) |
| CLI 分发 | [`dispatch.rs`](../core/runtime-core/src/cli/dispatch.rs) |
| 闭集 id / install_tool | `configs/framework/RUNTIME_REGISTRY.json` |
| review_gate loader | [`registry_review_gate.rs`](../core/core-policy/src/registry_review_gate.rs)（`runtime_registry` re-export） |
| lifecycle 文案 | [`host_projection_narrative.json`](../configs/framework/host_projection_narrative.json) |
| 生成物 manifest | [`GENERATED_ARTIFACTS.json`](../configs/framework/GENERATED_ARTIFACTS.json) |

1. 扩展 `RUNTIME_REGISTRY.json`（`framework_host_targets.rs` fail-closed 单测）。
2. **薄 hook**：解析 workspace/repo root → `router-rs …` argv；stdin 透传 JSON。
3. **验证 L2**：确认 `EVIDENCE_INDEX` / `NEXT_ACTIONS` 可按 schema 写入。
4. **安装/投影**：`framework host-integration install --to …`。
5. **文档**：更新本节或 `rust_contracts.md` Host 小节；Hook 矩阵写入 `docs/hosts/<host>.md`。

### 3.1 可复制执行清单（工程顺序）

**PR 顺序**：`RUNTIME_REGISTRY` → `framework_host_targets` → `<host>_hooks` + `main` → `dispatch` → `host_integration` → L4 `hooks.json` → 测试。

**日常接满**：`framework skills validate` → `framework sync-entrypoints` → 按需 `host-integration install --to <host>`。

| 阶段 | 主要落点 |
|------|----------|
| 注册表 | `RUNTIME_REGISTRY.json`，[`tests/common/mod.rs`](../tests/common/mod.rs)，[`framework_host_targets.rs`](../core/runtime-core/src/framework_host_targets.rs) |
| L3 入口 | `hosts/<host>_hooks/`（如 [`codex_hooks/mod.rs`](../core/host-projection/src/hosts/codex_hooks/mod.rs)）或 `<host>_hooks.rs`，`lib.rs` |
| CLI 分发 | [`dispatch_body.txt`](../core/runtime-core/src/cli/dispatch_body.txt)，[`dispatch.rs`](../core/runtime-core/src/cli/dispatch.rs) |
| 安装 / 投影 | [`host_integration/mod.rs`](../core/runtime-core/src/host_integration/mod.rs)，[`GENERATED_ARTIFACTS.json`](../configs/framework/GENERATED_ARTIFACTS.json) |
| L4 + 验证 | 宿主 `hooks.json`，[`tests/host_integration.rs`](../tests/host_integration.rs)，[`tests/policy_contracts.rs`](../tests/policy_contracts.rs) |

- [ ] **`RUNTIME_REGISTRY.json`**：`host_targets.supported`、`install_tool`、`host_entrypoints` 与现网对称。
- [ ] **`tests/common/mod.rs`**：测试夹具 registry 与真源对齐。
- [ ] **`framework_host_targets.rs`**：只读注册表，fail-closed。
- [ ] **`hosts/<host>_hooks/`** 或 **`<host>_hooks.rs`** + **`main.rs`** mod。
- [ ] **`dispatch_body.txt`** / **`dispatch.rs`**：`router-rs <host> hook <event> …`。
- [ ] **`host_integration/mod.rs`**：install + 投影 + `GENERATED_ARTIFACTS.json`。
- [ ] **L4 样例**：argv + 超时 + stdin 透传，shell 不复制 L3。
- [ ] **测试**：`cargo test --manifest-path core/router-rs/Cargo.toml`；根 `cargo test`。
- [ ] **`AGENTS.md`**：Codex 投影链变更时补一句权威分层说明。

### 3.1.1 Registry-driven HostProvider 注册（plug-in-by-name，2026-06）

**已注册表驱动**（真源 `RUNTIME_REGISTRY.json` → `host_targets.host_providers`）：

| 步骤 | 落点 | 说明 |
|------|------|------|
| 闭集 id | `host_targets.supported` | 全局宿主 id 列表 |
| Provider 注册 | `build.rs` → `OUT_DIR/generated_host_providers.rs` | 按 `supported` 顺序 `#[cfg(feature)]` push `HostProvider` |
| `mod` / feature 对齐 | `build.rs`（编译期）+ `validate_host_provider_mod_declarations`（单测） | 每个 `host_providers` 行须有 `Cargo.toml` feature、`hosts/mod.rs` 中 `#[cfg(feature)] mod <provider_module>`；有 `hooks_module` 时须有对应 `mod` |
| 运行时校验 | `validate_host_providers_against_registry` | 每个 `supported` id 须有运行时 `HostProvider` 条目 |
| 单测 | `host_provider_mod_declarations_align_with_registry` 等 | `framework_host_targets.rs` + `host_provider.rs` |

`host_providers.<id>` 字段：

| 字段 | 必填 | 说明 |
|------|------|------|
| `cargo_feature` | 是 | Cargo feature 名（须手写在 `Cargo.toml`，**不**从 JSON 生成） |
| `provider_module` | 是 | `hosts/<module>.rs`，与 `#[cfg(feature)] mod` 同名 |
| `provider_type` | 是 | `HostProvider` 实现类型名 |
| `hooks_module` | 否 | L4 hook / MCP agent 模块（`cursor_hooks`、`claude_code_hooks`、`codex_hooks`、`opencode_agent` 等） |
| `cli_hook_subcommand` | 否 | 有原生 hook 时 CLI 子命令（通常 `hook`）；L4 形如 `router-rs <install_tool> hook` |
| `cli_agent_subcommand` | 否 | MCP stdio 宿主 CLI 子命令（通常 `agent`） |
| `dispatch_fn` | 否 | `router_command_dispatch.rs` 中分发函数名（维护者对照；基于手动 `mod.rs` + `build.rs` 验证） |

**新增宿主最小手改清单**（registry 行 + 下列实现；`build.rs` / 单测会在 feature、`mod`、registry 三方不一致时 fail-closed）：

1. `RUNTIME_REGISTRY.json`：`supported`、`metadata`、`host_projections`、**`host_providers` 一行**（含上表字段）。
2. `core/router-rs/Cargo.toml`：手加 `[features]` 条目并列入 `default`。
3. `hosts/mod.rs`：`#[cfg(feature)] mod <provider_module>`；有 hook 时 `pub mod <hooks_module>`。
4. 实现 `hosts/<provider_module>.rs`（及可选 `hooks_module`）。
5. `router_command_dispatch.rs` / `args.rs`：实现 registry 中 `dispatch_fn` 指向的分发（`cli_*_subcommand` 为对照）。
6. `host_integration/mod.rs`、L4 `hooks.json`、`docs/hosts/<host>.md`（安装与文档面）。

**推荐顺序**：registry 行 → `Cargo.toml` feature → `hosts/mod.rs` + provider/hooks 实现 → dispatch / host_integration → `cargo test --manifest-path core/router-rs/Cargo.toml`（编译期与 `host_provider_mod_declarations_align_with_registry` 会校验三方一致）。

### 3.2 Maint / supervisor / profile 硬编码宿主耦合盘点

| 位置 | 硬编码内容 | 建议 |
|------|------------|------|
| [`framework_maint.rs`](../core/runtime-core/src/framework_maint.rs) | `refresh_host_projections` 遍历 installable 宿主 | 新宿主提供 maint verifier 或 `installable=false` |
| [`session_supervisor/mod.rs`](../core/runtime-core/src/session_supervisor/mod.rs) | **Codex driver only** | 新 CLI 宿主补 driver 前 registry 标记 `session_supervisor_driver=unsupported` |
| `framework_profile.rs` | `host_payloads` 从 `host_projections` 派生 | 新宿主补 `host_projections` + contract tests |
| [`hook_posttool_normalize.rs`](../core/runtime-core/src/utils/hook_posttool_normalize.rs) | Cursor `postToolUse` → 共享 append 形状 | Codex 直连 append；Cursor 专有分支在 `cursor_hooks` |

### 3.3 Host entrypoint sync engine / provider 边界

[`host_entrypoint_sync.rs`](../core/runtime-core/src/host_entrypoint_sync.rs) 负责通用 sync engine；Codex provider（`.codex/hooks.json`、**`AGENTS_CODEX.md`**、`.codex/README.md`）在 [`codex_hooks/mod.rs`](../core/host-projection/src/hosts/codex_hooks/mod.rs)。hook 编译期 embed **`AGENTS.md` + `AGENTS_CODEX.md`**（见 `policy_embed.rs`）；**不** materialize 或 overwrite 仓库根 [`AGENTS.md`](../AGENTS.md)。`router-rs framework sync-entrypoints` 为统一 CLI 入口。root 用 `full_sync`；匹配 worktree 仅 `partial_sync`（JSON hook/manifest，不覆盖本地策略文本）。`HostProjectionAdapter` 为薄 adapter 表；`RUNTIME_PROVIDER_REGISTRY` 的 host provider lane 只作目录/报告面。

### 3.4 PostTool / 终端证据归一化

Codex `PostToolUse` 直连 [`try_append_post_tool_shell_evidence`](../core/runtime-core/src/framework_runtime/mod.rs)；Cursor 经 [`hook_posttool_normalize.rs`](../core/runtime-core/src/utils/hook_posttool_normalize.rs) `synthetic_post_tool_evidence_shape` 合成同一形状再 append。**`hook_common::tool_input_value_from_map`** 单层 JSON 键优先级：`tool_input` → `input` → `arguments` → `parameters`。

---

## 4. 与五层模型的对齐（L4 / L5）

| 层 | 允许 | 禁止 |
|----|------|------|
| **L4** | 调用 `router-rs` 子命令、固定超时、环境透传 | 长段策略 prose、复制 L3 门控、手写 `EVIDENCE_INDEX` 规则 |
| **L5** | SKILL 契约、`verify_commands`、拒因枚举、编排叙事 | 第二套连续性目录、或与 L2 schema 冲突的并行真源 |

L3 负责 review/closeout/`AG_FOLLOWUP` 门控、PostTool 采样、gate 状态；宏目标续跑用 MCP `goal_state_manage`（Claude Desktop / Antigravity / OpenCode）或 `framework_goal_drive` stdio（CLI / Cursor / Codex）/ `framework_rfv_loop` stdio 与 `artifacts/current/<task_id>/` 手动画板（**无** hook `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE`，2026-05）。
