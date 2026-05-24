# 宿主适配契约（Host adapter contract）

本文件描述 **新宿主如何接入** 本仓库的连续性 harness 与 `router-rs` 控制面：哪些能力可移植复用、L4/L5 边界，以及工程清单。**Hook 事件 → CLI → 写盘** 矩阵已下沉至各 [`docs/hosts/`](hosts/) 手册 **Hook 事件矩阵** 节。

**权威列表**：`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported` 为 **当前闭集宿主 id**；安装/状态/卸载与 Codex **`host_entrypoints_sync_manifest`** 的 `supported_hosts` / `host_entrypoints` 由同一注册表推导（实现见 `router-rs` 中 `framework_host_targets`）。

**历史字段**：`host_targets.entrypoint_files` 已从注册表移除；宿主策略入口与 sync manifest 入口集合以 `host_targets.metadata.<host>.host_entrypoints` 为唯一权威（`framework_host_targets::host_entrypoints_value_for_id`）。

**相关契约**：[`rust_contracts.md`](rust_contracts.md) · [`harness_architecture.md`](harness_architecture.md)。

## 快速路径（我要接新宿主）

- **先读**：[`harness_architecture.md`](harness_architecture.md) **§5–§6**，再读本文件 **§0** 与 **[§3.1](#31-可复制执行清单工程顺序)**。
- **再改**：`RUNTIME_REGISTRY` → hooks 模块 → `dispatch` → `host_integration` → L4 → 测试（详见 §3.1）。
- **跑测**：`cargo test --manifest-path scripts/router-rs/Cargo.toml`；改动根 [`tests/`](../tests/) 时再 `cargo test`。
- **入口同步**：`router-rs framework sync-entrypoints --repo-root "$PWD"`（或兼容别名 `router-rs codex sync`）。

---

## 0. North-star contract 与维护地图

**解耦**：宿主差异只允许停留在 L4 适配壳与 `RUNTIME_REGISTRY.host_targets` 元数据；L2 工件 schema、L3 CLI 行为、门控/证据追加逻辑必须复用 Rust owner。**无感**：用户在 **任一直持闭集宿主**上得到同一套 harness 连续性、证据、closeout 与热路由闭集语义。

**非目标**：不把每个宿主做成独立 runtime fork；不在 hook shell、`.mdc` 或 skill prose 中复制 L3 决策。

## 0.1 Countable REVIEW_GATE deep review lanes

**权威真源**（[`configs/framework/RUNTIME_REGISTRY.json`](../configs/framework/RUNTIME_REGISTRY.json)，[`registry_loader.rs`](../scripts/router-rs/src/registry_loader.rs)，ADR-005）：

| 字段 | 消费宿主 | 含义 |
|------|----------|------|
| `review_gate.deep_gate_lanes` | **Cursor**、**Codex CLI** | 可数深度 lane：`general-purpose`、`best-of-n-runner`、`deep-reviewer` 及归一化等价名 |
| `review_gate.claude_reviewer_lanes` | **Claude Code** | 上表四拼写 **加上** `review`、`reviewer`、`critic`、`code-review` |

**勿混读**：`deep_gate_lanes` **不是** Claude 超集；Cursor/Codex 上 `subagent_type: "review"` 且 `fork_context=false` **不计**独立审稿证据。增删 lane 只改 registry；重启 hook 子进程即可。dispatch 须绑定 `repo_root`（`HookRegistryRepoGuard`）。

| 字段 | 含义 |
|------|------|
| `review_gate.spawn_first_enabled` | 默认 true；false 时跳过 spawn-first nudge（清门阈值不变） |
| `review_gate.spawn_first_nudge` | 全局回退文案 |
| `review_gate.spawn_first_nudge_by_host` | 按宿主一行配对审稿 |

**窄范围 skip**（四宿主）：`hook_common::is_narrow_review_prompt` → 不武装 `review_required`。

### 跨宿主 REVIEW_GATE 差异（operator）

| 能力 | Cursor | Codex CLI | Claude Code | Claude Desktop / codex-app | Antigravity |
|------|--------|-----------|-------------|----------------------------|-------------|
| 可数深度 lane 真源 | `deep_gate_lanes` | `deep_gate_lanes` | `claude_reviewer_lanes` | 无 hook 面 | 物理 `review-lanes/` 目录 |
| subagentStart/Stop multiset | ✓ | ✗ | ✗ | ✗ | ✗ |
| wave-2 compact 清门 | ✓ | ✓（部分） | ✗ | ✗ | ✗（需物理文件） |
| `reject_reason` / `rg_clear` Stop 清门 | ✓ | ✓ | ✗ | ✗ | ✗ |
| Stop 硬短码 | `router-rs REVIEW_GATE incomplete` | `CODEX_REVIEW_GATE` | `CLAUDE_REVIEW_GATE` | — | `[Antigravity Hard Block]` |
| Review gate disable env | `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` | `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE` | `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE` | — | 无（`my-light` 控制） |
| Goal/RFV hook 续跑 | ✗ | ✗ | ✗ | ✗ | ✗ |

细节见 [`harness_architecture.md`](harness_architecture.md) §5.0 与各 [`docs/hosts/*.md`](hosts/)。

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

**闭集宿主**：`claude-code`（`router-rs claude hook`）、`claude-desktop`（MCP，`router-rs claude-desktop agent`）、`antigravity`（Planning Mode + `.gemini/` 投影）。操作手册：[`cursor.md`](hosts/cursor.md)、[`codex-cli.md`](hosts/codex-cli.md)、[`claude.md`](hosts/claude.md)、[`claude-desktop.md`](hosts/claude-desktop.md)、[`antigravity.md`](hosts/antigravity.md)。

**Cursor 投影 scope**：`framework.mdc` 仅 **user** scope（`~/.cursor/rules/`）；项目保留 `.cursor/hooks.json`。

---

## 3. 新宿主接入 Checklist

| 职责 | 仓库路径 |
|------|----------|
| Cursor hook | [`cursor_hooks/mod.rs`](../scripts/router-rs/src/hosts/cursor_hooks/mod.rs) |
| Codex hook | [`codex_hooks.rs`](../scripts/router-rs/src/hosts/codex_hooks/mod.rs) |
| Claude Code hook | [`claude_hooks.rs`](../scripts/router-rs/src/hosts/claude_hooks.rs) |
| Claude Desktop MCP | `claude_desktop_hooks.rs` / `router-rs claude-desktop agent` |
| entrypoint sync | [`host_entrypoint_sync.rs`](../scripts/router-rs/src/host_entrypoint_sync.rs) |
| install / 投影 | [`host_integration.rs`](../scripts/router-rs/src/host_integration.rs) |
| CLI 分发 | [`dispatch.rs`](../scripts/router-rs/src/cli/dispatch.rs) |
| 闭集 id / install_tool | `configs/framework/RUNTIME_REGISTRY.json` |
| review_gate loader | [`registry_loader.rs`](../scripts/router-rs/src/registry_loader.rs) |
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
| 注册表 | `RUNTIME_REGISTRY.json`，[`tests/common/mod.rs`](../tests/common/mod.rs)，[`framework_host_targets.rs`](../scripts/router-rs/src/framework_host_targets.rs) |
| L3 入口 | `<host>_hooks.rs`，[`main.rs`](../scripts/router-rs/src/main.rs) |
| CLI 分发 | [`dispatch_body.txt`](../scripts/router-rs/src/cli/dispatch_body.txt)，[`dispatch.rs`](../scripts/router-rs/src/cli/dispatch.rs) |
| 安装 / 投影 | [`host_integration.rs`](../scripts/router-rs/src/host_integration.rs)，[`GENERATED_ARTIFACTS.json`](../configs/framework/GENERATED_ARTIFACTS.json) |
| L4 + 验证 | 宿主 `hooks.json`，[`tests/host_integration.rs`](../tests/host_integration.rs)，[`tests/policy_contracts.rs`](../tests/policy_contracts.rs) |

- [ ] **`RUNTIME_REGISTRY.json`**：`host_targets.supported`、`install_tool`、`host_entrypoints` 与现网对称。
- [ ] **`tests/common/mod.rs`**：测试夹具 registry 与真源对齐。
- [ ] **`framework_host_targets.rs`**：只读注册表，fail-closed。
- [ ] **`<host>_hooks.rs`** + **`main.rs`** mod。
- [ ] **`dispatch_body.txt`** / **`dispatch.rs`**：`router-rs <host> hook <event> …`。
- [ ] **`host_integration.rs`**：install + 投影 + `GENERATED_ARTIFACTS.json`。
- [ ] **L4 样例**：argv + 超时 + stdin 透传，shell 不复制 L3。
- [ ] **测试**：`cargo test --manifest-path scripts/router-rs/Cargo.toml`；根 `cargo test`。
- [ ] **`AGENTS.md`**：Codex 投影链变更时补一句权威分层说明。

### 3.2 Maint / supervisor / profile 硬编码宿主耦合盘点

| 位置 | 硬编码内容 | 建议 |
|------|------------|------|
| [`framework_maint.rs`](../scripts/router-rs/src/framework_maint.rs) | `refresh_host_projections` 遍历 installable 宿主 | 新宿主提供 maint verifier 或 `installable=false` |
| [`session_supervisor.rs`](../scripts/router-rs/src/session_supervisor.rs) | **Codex driver only** | 新 CLI 宿主补 driver 前 registry 标记 `session_supervisor_driver=unsupported` |
| [`framework_profile.rs`](../scripts/router-rs/src/framework_profile.rs) | `host_payloads` 从 `host_projections` 派生 | 新宿主补 `host_projections` + contract tests |
| [`hook_posttool_normalize.rs`](../scripts/router-rs/src/utils/hook_posttool_normalize.rs) | Cursor `postToolUse` → 共享 append 形状 | Codex 直连 append；Cursor 专有分支在 `cursor_hooks` |

### 3.3 Host entrypoint sync engine / provider 边界

[`host_entrypoint_sync.rs`](../scripts/router-rs/src/host_entrypoint_sync.rs) 负责通用 sync engine；Codex provider（`.codex/hooks.json`、`AGENTS.md` bootstrap）在 [`codex_hooks.rs`](../scripts/router-rs/src/hosts/codex_hooks/mod.rs)。`router-rs codex sync` 为兼容 CLI 名。root 用 `full_sync`；匹配 worktree 仅 `partial_sync`（JSON hook/manifest，不覆盖本地策略文本）。`HostProjectionAdapter` 为薄 adapter 表；`RUNTIME_PROVIDER_REGISTRY` 的 host provider lane 只作目录/报告面。

### 3.4 PostTool / 终端证据归一化

Codex `PostToolUse` 直连 [`try_append_post_tool_shell_evidence`](../scripts/router-rs/src/framework_runtime/mod.rs)；Cursor 经 [`hook_posttool_normalize.rs`](../scripts/router-rs/src/utils/hook_posttool_normalize.rs) `synthetic_post_tool_evidence_shape` 合成同一形状再 append。**`hook_common::tool_input_value_from_map`** 单层 JSON 键优先级：`tool_input` → `input` → `arguments` → `parameters`。

---

## 4. 与五层模型的对齐（L4 / L5）

| 层 | 允许 | 禁止 |
|----|------|------|
| **L4** | 调用 `router-rs` 子命令、固定超时、环境透传 | 长段策略 prose、复制 L3 门控、手写 `EVIDENCE_INDEX` 规则 |
| **L5** | SKILL 契约、`verify_commands`、拒因枚举、编排叙事 | 第二套连续性目录、或与 L2 schema 冲突的并行真源 |

L3 负责 review/closeout/`AG_FOLLOWUP` 门控、PostTool 采样、gate 状态；宏目标续跑仅 **`framework_goal_drive` / `framework_rfv_loop` stdio** 与 `artifacts/current/<task_id>/` 手动画板（**无** hook `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE`，2026-05）。
