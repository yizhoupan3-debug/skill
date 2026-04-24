# Host Adapter Contracts

## Goal

用 **deep adaptation, not deep fork** 的方式，把统一的 `framework_profile` 投影到不同 host。

当前主线已经统一为 `Rust-owned contract truth + thin host projection`：

- `thin projection` = host 只消费 shared framework truth，不接管宿主原生 runtime truth
- `Rust-owned contract truth` = shared contract / workspace bootstrap /
  host adapter / artifact / parity / discovery / compatibility inventory 都由
  `router-rs` 编译；旧 Python projection 已退场

- `framework_profile` / framework core = 单真源
- `cli_common_adapter` = canonical CLI-family shared contract
- `codex_common_adapter` = Codex 兼容视图，不再是命名中心
- `codex_desktop_adapter` = 交互式 desktop 正式入口
- `codex_desktop_host_adapter` = retired desktop 兼容别名（仅显式 continuity lane）
- `codex_cli_adapter` / `claude_code_adapter` / `gemini_cli_adapter` = 三个薄 CLI 宿主投影
- `cli_family_parity_snapshot` = canonical CLI regression baseline
- `codex_dual_entry_parity_snapshot` = Codex desktop/headless compatibility view

`aionrs` / `AionUI` 相关 adapter 是 retired compatibility debt，不再由
artifact emitter 产出，也不再作为 fallback host lane 发布。需要兼容
清单时，只能通过 Rust `--profile-artifacts-json --include-compatibility-inventory`
输出 `upgrade_compatibility_matrix`。默认 artifact emission 不写
`codex_desktop_alias_inventory.json`，不写 Python/Rust parity report；legacy alias
retirement status 只在显式 Rust continuity opt-in 中输出。

## Adapter Set

### Retired: `aionrs_companion_adapter`

- host: `aionrs-companion`
- transport: `stdio-jsonl`
- role: retired legacy companion / former fallback bridge
- boundary: **outer-framework-owned**
- invariant: 只做伴随式接入，不修改 aionrs 内核语义
- invariant: 不属于 default host peer set；artifact emitter 不再产出
  该 adapter
- protocol hints:
  `deep_adaptation_not_fork = true`, `legacy_surface = true`,
  `legacy_lane = fallback`

### Retired: `aionui_host_adapter`

- host: `aionui`
- transport: `bridge-contract`
- role: retired legacy compatibility shell
- invariant: UI 只消费外层 contract，不反向成为框架真源
- invariant: 不属于 default host peer set；artifact emitter 不再产出
  该 adapter
- runtime event lane: `host_runtime_contract.event_transport`
- protocol hints:
  `state_source = framework_profile`, `preferred_backend = aionrs_companion_adapter`,
  `legacy_surface = true`, `legacy_lane = fallback`

### 1. `cli_common_adapter`

- host: `cli-family-shared`
- transport: `host-neutral-contract`
- role: Desktop 与 CLI-family 共享 contract 编译层
- invariant: `framework_truth = framework_core`
- invariant: `codexcli_is_controller = false`
- protocol hints: `single_framework_truth = true`

### 2. `codex_common_adapter` (Compatibility View)

- host: `codex-shared`
- transport: `host-neutral-contract`
- role: 对 `cli_common_adapter` 的 Codex 兼容命名视图
- invariant: `canonical_adapter_id = cli_common_adapter`
- invariant: 只允许镜像 shared contract，不得分叉出新的 common semantics

### 3. `codex_desktop_adapter`

- host: `codex-desktop`
- transport: `local-bridge`
- invariant: **works_without_aionrs = true**
- role: Codex Desktop 交互式入口；消费 common adapter 的 shared contract，
  并作为 desktop identity 的唯一正式名字
- protocol hints: `works_without_aionrs = true`

### 4. Retired: `codex_desktop_host_adapter`

- host: `codex-desktop`
- transport: `local-bridge`
- role: retired 兼容别名；仅供显式 continuity / compatibility 调用，payload 语义对齐 `codex_desktop_adapter`
- invariant: `canonical_adapter_id = codex_desktop_adapter`
- invariant: `retirement_mode = compatibility_only`
- invariant: `accepts_new_semantics = false`

### 5. `codex_cli_adapter`

- host: `codex-cli`
- transport: `headless-exec`
- invariant: `codexcli_is_controller = false`
- role: batch / cron / CI / non-interactive entrypoint
- protocol hints:
  `AGENTS.md` / `~/.codex/config.toml` / `.codex/config.toml`

### 6. `claude_code_adapter`

- host: `claude-code`
- transport: `headless-exec`
- invariant: `framework_truth = framework_core`
- role: Claude Code 的薄投影，不复制 framework core
- protocol hints:
  `CLAUDE.md` / `CLAUDE.local.md` /
  `~/.claude/settings.json` / `.claude/settings.json` /
  `.claude/settings.local.json` / `~/.claude.json`
- Claude-specific host projection 还会显式暴露：
  `managed -> command_line -> local -> project -> user` precedence、
  `.claude/agents/` / `~/.claude/agents/`、`.claude/hooks/`、
  hook event / control / source / env-marker metadata、checkpoint support、以及
  managed settings / managed MCP 文件落点；这些都只属于宿主投影，不进入
  shared runtime surface，也不把 hook execution / policy resolution 主导权交给
  framework runtime kernel

### 7. `gemini_cli_adapter`

- host: `gemini-cli`
- transport: `headless-exec`
- invariant: `framework_truth = framework_core`
- role: Gemini CLI 的薄投影，不复制 framework core
- protocol hints:
  `GEMINI.md` / `~/.gemini/settings.json` / `stream-json`

### 8. `cli_family_parity_snapshot`

- role: `codex_cli_adapter` / `claude_code_adapter` / `gemini_cli_adapter`
  的共享 contract 回归基线
- invariant: parity 检查围绕 shared runtime surface，而不是围绕宿主私有配置文件

### 9. `cli_family_capability_discovery`

- role: 为 `codex_cli_adapter` / `claude_code_adapter` / `gemini_cli_adapter`
  外显一份稳定的 capability discovery contract
- invariant: 只暴露 host capability / discovery surface / resolved requirement /
  compatibility 结果，不把宿主私有控制流反写进 shared runtime truth
- invariant: `codex cli` 与 `claude cli` 必须都能通过这份 discovery contract
  被独立识别和比较，而不是退回到单一 CLI 入口叙事

### Retired: `generic_host_adapter`

- host: `generic`
- transport: `inproc`
- role: retired minimal fallback host; artifact emitter no longer writes it

## `codex_desktop_host_adapter` Retirement Contract

`codex_desktop_host_adapter` 已退出默认路径，只允许承担历史兼容桥角色，不再承载新的设计中心语义。

- 当 alias 存在时，它必须镜像 `codex_desktop_adapter` 的 contract 语义；
  允许存在的差异只限于 `metadata.adapter_id`、兼容注解、或显式 legacy
  alias 标记。
- 新文档、新调用方、新 artifact 不得再把
  `codex_desktop_host_adapter` 记为 canonical desktop identity。
- Desktop / CLI parity、Rust bundle、以及后续 extracted artifacts 都必须以
  `cli_common_adapter` + `codex_desktop_adapter` 为真源，再决定是否额外附带 alias。
- 默认 runtime helper surface 也必须保持 canonical-only：`get_host_adapter`
  和 `list_host_adapters` 只返回正式 adapter；compatibility consumer 如需
  legacy surface，必须显式 opt-in Rust compatibility inventory。fallback host
  artifact lane 已退出默认 emitter。
- alias 的最终物理退场前提是：下游调用方迁移完成、双入口 parity 仍保持绿色、且
  连续性工件能说明剩余兼容风险与回滚点；在此之前它只能显式 opt-in。
- alias 退场不意味着回退到 `aionrs` / `AionUI` 主线，也不允许借机把
  `codexcli` 抬升为控制器。

## Legacy Surface Guardrails

- default host peer set 固定为：
  `codex_desktop_adapter` / `codex_cli_adapter` / `claude_code_adapter` /
  `gemini_cli_adapter`
- `aionrs_companion_adapter`、`aionui_host_adapter` 与
  `codex_desktop_host_adapter` 都只能通过显式 legacy opt-in 被发现：
  - `codex_desktop_host_adapter` 进入 `compatibility_lane`
  - `aionrs_companion_adapter`、`aionui_host_adapter` 只作为 compatibility
    inventory rows，不再作为 fallback artifacts 写出
- legacy surface 必须带 `legacy_boundary` contract，至少声明：
  - `adapter_lifecycle = legacy-compatibility`
  - `default_host_peer_set_member = false`
  - `may_become_framework_truth = false`
  - removal readiness 与 migration guardrails
- `compatibility_snapshot()` / `build_upgrade_compatibility_matrix()` 的 Python
  真源语义已退出；需要兼容清单时使用 Rust `upgrade_compatibility_matrix`。
- legacy rows 只允许在显式 Rust compatibility inventory 中出现，不得回到
  primary peer inventory。

## Adapter Payload Rules

所有 adapter 输出统一 payload：

- `profile_id`
- `display_name`
- `framework_profile_version`
- `host_family`
- `runtime_family`
- `capabilities`
- `rules_bundle`
- `skill_bundle`
- `session_policy`
- `tool_policy`
- `approval_policy`
- `loadout_policy`
- `artifact_contract`
- `model_policy`
- `memory_mounts`
- `mcp_servers`
- `workspace_bootstrap`
- `host_capability_requirements`
- `metadata.adapter_id / metadata.host_id / metadata.transport`
- shared contract surface from `cli_common_adapter.shared_contract` also includes:
  - `execution_controller_contract`
  - `delegation_contract`
  - `supervisor_state_contract`

## OMC Retirement Boundary

- `oh-my-claudecode` / OMC 是被替代对象，不是兼容内核。
- steady-state runtime truth 不得再落到 `.omc/**`。
- 新能力面固定为 framework-native capability：
  - `external_session_supervisor`
  - `rate_limit_auto_resume`
  - `host_resume_entrypoint`
  - `host_tmux_worker_management`
- `autopilot` / `deepinterview` 只保留为 framework-native alias：
  - canonical owner 在 framework
  - Claude / Codex 只暴露不同入口，不得分叉语义
  - 两者都必须承接原版 OMC 的核心能力，但实现标准必须更强
  - `autopilot` 额外强制：根因未知先定位、必须有验证证据、必须保留恢复续跑能力、必须推进到有界范围收敛
  - `deepinterview` 额外强制：根因未知先定位、findings 必须按严重度输出、必须给出验证证据、需要时进入 fix -> verify 收敛循环
- `cli_family_capability_discovery` 现在必须能直接回答：
  - 哪些 host 有 shared supervisor 能力
  - 哪些 host 有 resume 入口
  - 哪些 host 有 rate-limit auto-resume
  - 两端 alias 入口分别是什么

## Non-Goals

- 不在 adapter 中引入 aionrs 私有 runtime patch。
- 不在 adapter 中复制 AionUI 内部状态机。
- 不把 compatibility 逻辑写成单宿主特化分支泥球。
- 不重新把 `codex_desktop_host_adapter` 升格为正式 desktop API。

## Current Implementation

已实现并收口为 Rust 真源：

- `router-rs --profile-json --framework-profile <path>` 编译 profile bundle。
- `router-rs --profile-artifacts-json --framework-profile <path>` 编译 adapter
  artifacts、shared contract、capability discovery、parity snapshot、control-plane
  contracts、compatibility inventory。
- 旧 `emit_framework_contract_artifacts(...)` Python 入口已不再是当前实现面；
  contract/artifact 输出由 `router-rs` 直接生成和校验。
- 旧 `framework_runtime.host_adapters.compile_*_adapter(...)` Python 投影已退场；
  adapter truth 留在 Rust profile/artifact compiler。
- 旧 `framework_runtime.host_adapter_compatibility.*` 不再用于 artifact emission；
  fallback host artifacts (`aionrs_companion_adapter`、`aionui_host_adapter`、
  `generic_host_adapter`) 已退出。
- `rust_python_artifact_parity_report.json` 已退出；Rust 输出就是默认 contract
  truth。
- `codex_desktop_alias_inventory.json` 不再写出；`codex_desktop_alias_retirement_status`
  只在显式 Rust continuity opt-in 中输出。
- `router-rs --sync-host-entrypoints-json --repo-root <repo_root>` 继续负责 repo-level
  `AGENTS.md` / `CLAUDE.md` / `GEMINI.md` materialization plus `.claude/` /
  `.gemini/` bootstrap。

默认 lookup / registry helper 保持 canonical-only：不显式开启 compatibility
inventory 时，`codex_desktop_host_adapter` 不作为 peer adapter 出现在 runtime
helper surface。
执行内核这条 contract lane 也继续保持薄投影边界：稳定 shared contract 现在会
公开 `execution_kernel_delegate_family` /
`execution_kernel_delegate_impl`，但 compatibility-only 的
`execution_kernel_fallback_reason` 仍只停留在 fallback response metadata /
retirement artifact，不进入 framework truth。
`rust_execute_fallback_to_python` 这条 retired explicit-request surface 已经移除。
现在 host/runtime contract 只保留 retirement artifact 说明这条旧请求面曾经存在，
但 steady-state config、env 和 runtime health 都不再把它当作可探测开关。
现在还会额外产出
`execution_kernel_live_response_serialization_contract`，把 live primary /
compatibility fallback / dry-run 三种 response shape 的字段与 metadata invariant
冻结成 shared artifact evidence；这一步只固定 contract，不改 runtime branching。

## Continuity And Memory Notes

- root continuity artifacts
  `SESSION_SUMMARY.md` / `NEXT_ACTIONS.json` / `EVIDENCE_INDEX.json` /
  `TRACE_METADATA.json` / `.supervisor_state.json`
  是 supervisor-only 写入锚点，也是恢复与 sign-off 的 authoritative contract。
- `artifacts/current/*` 是给 bridge / MCP / recall 流程消费的当前会话镜像；
  它必须与 root continuity artifacts 讲同一个 task story，不能单独漂移。
- `./.codex/memory/` 是逻辑 memory root；本仓库当前通过 symlink 映射到
  `./memory/`。维护时应把它们视为同一棵 shared memory，而不是双根。

下一轮 runtime 优化的原则也固定如下：

- host adapter 继续只做 contract projection，不抢 runtime kernel 主导权
- 借鉴 DeerFlow 2.0 的 runtime decomposition 时，优先借 run-manager、
  stream bridge、sandbox control plane 这些控制面边界
- 不把 host adapter 演化成 DeerFlow 那种 gateway/app 层的替代物

当前实现边界补充：

- `host_runtime_contract.event_transport` 现在对齐 runtime 的 versioned
  event transport seam，而不是只停留在旧的 bridge binding 提示字段
- 该 contract 当前固定暴露：
  `schema_version` / `transport_family` / `transport_kind` /
  `endpoint_kind` / `remote_capable` / `handoff_supported` /
  `handoff_method` / `subscribe_method` / `cleanup_method` /
  `describe_method` / `resume_mode` / `heartbeat_supported` /
  `cleanup_semantics` /
  `cleanup_preserves_replay` / `replay_reseed_supported` /
  `chunk_schema_version` / `cursor_schema_version` / `replay_supported`
- session-scoped 的 `stream_id` / `latest_cursor` / `binding_artifact_path` /
  replay path 等动态信息应由 runtime transport 或 handoff descriptor 提供，
  adapter 只声明静态 host-facing seam
- `event_stream_binding` 继续保留为 compatibility alias，但 canonical 字段已是
  `event_transport`
- adapter 不应自己发明第二套 stream 状态机，而应消费 runtime 已暴露的
  transport / replay boundary
- 当前 `cleanup_runtime_events(...)` 的语义是清空 bridge cache，而不是删除
  replayable event stream；只要底层 replay sink 还在，runtime 允许后续

## Repo Entrypoint Materialization

为了避免 “contract 说支持，仓库入口却没落盘” 的失配，三 CLI 的入口文件必须在
repo 内真实存在：

- `AGENTS.md` for Codex
- `CLAUDE.md` for Claude Code
- `GEMINI.md` for Gemini CLI

这些文件都应是对共享 `AGENT.md` 的薄代理。宿主私有目录如 `.claude/hooks/`、
`.claude/settings.json`、`.gemini/settings.json` 允许存在，但它们只承载
host-private surface，不得分叉 shared routing / memory truth；本仓库当前不保留
项目级 `.claude/agents/` 投影。
Claude 入口维护边界见 `docs/claude_entrypoint_maintenance.md`：全局规则改
`AGENT.md`，生成投影改 `scripts/router-rs/src/claude_hooks.rs` 后同步，
机器本地偏好只放 `~/.claude/settings.json` 或 `.claude/settings.local.json`。
- repo entrypoint projection 还要区分两类输出约束：
  machine continuity contract 继续以 `SESSION_SUMMARY.md`、`NEXT_ACTIONS.json`、
  `TRACE_METADATA.json`、`.supervisor_state.json` 为恢复真源；human closeout
  contract 只约束给用户看的最终收尾话术。
- human closeout contract 默认应压缩成一小段人话，或在内容天然成列表时压缩成很短的 bullet；只讲“做了什么 / 现在达到什么效果 / 接下来还需要什么”。如果没有后续，就直接声明已完成。
- closeout 默认要讲得自然，不要写成 task artifact、审计日志、状态机播报或机器字段翻译。
- `verification`、`open_blockers`、`next_actions`、artifact path 等字段仍可用于
  runtime / recovery / tooling，但宿主投影不得默认把这些机器字段逐项翻译成
  changelog、文件清单或冗长收尾。
- `subscribe` 通过 reseed 恢复 resumability
- `describe_runtime_event_handoff(...)` 是当前推荐的跨进程/远端 attach seam；
  它返回 transport descriptor 加 replay/checkpoint 锚点，但不意味着 SSE、
  websocket 或 broker 传输已经实现
- `trace_resume_manifest_path` 现在只保留为 checkpoint/recovery metadata；
  host 若需要 attach，应优先读取 transport binding artifact，并把
  `describe_runtime_event_handoff(...)` 视为推荐入口，而不是把 resume manifest
  当作 primary attach authority
- 当 checkpoint backend 是 SQLite 且这些 artifact path 只是逻辑存储路径时，
  external attach bridge 会通过同一 storage root 下的 SQLite backing store
  读取 transport / resume / trace stream；它不再偷偷退化成 filesystem-only
