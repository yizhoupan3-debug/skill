# Host Adapter Contracts

## Goal

用 **deep adaptation, not deep fork** 的方式，把统一的 `framework_profile` 投影到不同 host。

当前主线已经统一为 `thin projection + Rust contract-first migration`：

- `thin projection` = host 只消费 shared framework truth，不接管宿主原生 runtime truth
- `Rust contract-first migration` = 优先把 shared contract / artifact / parity /
  discovery 等稳定面迁到 Rust，而不是先做 runtime replacement

- `framework_profile` / framework core = 单真源
- `cli_common_adapter` = canonical CLI-family shared contract
- `codex_common_adapter` = Codex 兼容视图，不再是命名中心
- `codex_desktop_adapter` = 交互式 desktop 正式入口
- `codex_desktop_host_adapter` = desktop 兼容别名（仅 compatibility escape hatch）
- `codex_cli_adapter` / `claude_code_adapter` / `gemini_cli_adapter` = 三个薄 CLI 宿主投影
- `cli_family_parity_snapshot` = canonical CLI regression baseline
- `codex_dual_entry_parity_snapshot` = Codex desktop/headless compatibility view

`aionrs` / `AionUI` 相关 adapter 仍可保留，但只作为 upstream-facing
legacy migration debt；它们不是未来主线、不是控制器候选，也不是双入口
叙事的中心。
`codex_agno_runtime.compatibility.build_upgrade_compatibility_matrix(...)`
也只允许停留在 secondary
compatibility inventory / smoke 角色，不能替代 `cli_family_parity_snapshot`
或 `codex_dual_entry_parity_snapshot` 成为主回归基线。
默认 Python artifact emission 不再携带
`codex_desktop_alias_inventory.json` 或
`codex_desktop_alias_retirement_status.json`；默认 Rust
`--profile-artifacts-json` 也不再把 `codex_desktop_alias_retirement_status`
当作 first-class 输出，默认 Rust `--profile-json` / `ProfileBundle`
也同样不再内联这类 retirement 证据。这些 alias continuity 证据只在显式
continuity lane 里保留。

## Adapter Set

### 1. `aionrs_companion_adapter`

- host: `aionrs-companion`
- transport: `stdio-jsonl`
- role: legacy companion / fallback bridge
- boundary: **outer-framework-owned**
- invariant: 只做伴随式接入，不修改 aionrs 内核语义
- invariant: 不属于 default host peer set；只允许在显式 fallback lane 中出现
- protocol hints:
  `deep_adaptation_not_fork = true`, `legacy_surface = true`,
  `legacy_lane = fallback`

### 2. `aionui_host_adapter`

- host: `aionui`
- transport: `bridge-contract`
- role: legacy compatibility shell
- invariant: UI 只消费外层 contract，不反向成为框架真源
- invariant: 不属于 default host peer set；只允许在显式 fallback lane 中出现
- runtime event lane: `host_runtime_contract.event_transport`
- protocol hints:
  `state_source = framework_profile`, `preferred_backend = aionrs_companion_adapter`,
  `legacy_surface = true`, `legacy_lane = fallback`

### 3. `cli_common_adapter`

- host: `cli-family-shared`
- transport: `host-neutral-contract`
- role: Desktop 与 CLI-family 共享 contract 编译层
- invariant: `framework_truth = framework_core`
- invariant: `codexcli_is_controller = false`
- protocol hints: `single_framework_truth = true`

### 4. `codex_common_adapter` (Compatibility View)

- host: `codex-shared`
- transport: `host-neutral-contract`
- role: 对 `cli_common_adapter` 的 Codex 兼容命名视图
- invariant: `canonical_adapter_id = cli_common_adapter`
- invariant: 只允许镜像 shared contract，不得分叉出新的 common semantics

### 5. `codex_desktop_adapter`

- host: `codex-desktop`
- transport: `local-bridge`
- invariant: **works_without_aionrs = true**
- role: Codex Desktop 交互式入口；消费 common adapter 的 shared contract，
  并作为 desktop identity 的唯一正式名字
- protocol hints: `works_without_aionrs = true`

### 6. `codex_desktop_host_adapter` (Compatibility Escape Hatch)

- host: `codex-desktop`
- transport: `local-bridge`
- role: 临时兼容别名；仅供 continuity / compatibility 调用，payload 语义对齐 `codex_desktop_adapter`
- invariant: `canonical_adapter_id = codex_desktop_adapter`
- invariant: `retirement_mode = compatibility_only`
- invariant: `accepts_new_semantics = false`

### 7. `codex_cli_adapter`

- host: `codex-cli`
- transport: `headless-exec`
- invariant: `codexcli_is_controller = false`
- role: batch / cron / CI / non-interactive entrypoint
- protocol hints:
  `AGENTS.md` / `~/.codex/config.toml` / `.codex/config.toml`

### 8. `claude_code_adapter`

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

### 9. `gemini_cli_adapter`

- host: `gemini-cli`
- transport: `headless-exec`
- invariant: `framework_truth = framework_core`
- role: Gemini CLI 的薄投影，不复制 framework core
- protocol hints:
  `GEMINI.md` / `~/.gemini/settings.json` / `stream-json`

### 10. `cli_family_parity_snapshot`

- role: `codex_cli_adapter` / `claude_code_adapter` / `gemini_cli_adapter`
  的共享 contract 回归基线
- invariant: parity 检查围绕 shared runtime surface，而不是围绕宿主私有配置文件

### 10.5. `cli_family_capability_discovery`

- role: 为 `codex_cli_adapter` / `claude_code_adapter` / `gemini_cli_adapter`
  外显一份稳定的 capability discovery contract
- invariant: 只暴露 host capability / discovery surface / resolved requirement /
  compatibility 结果，不把宿主私有控制流反写进 shared runtime truth
- invariant: `codex cli` 与 `claude cli` 必须都能通过这份 discovery contract
  被独立识别和比较，而不是退回到单一 CLI 入口叙事

### 11. `generic_host_adapter`

- host: `generic`
- transport: `inproc`
- role: 最小非绑定 fallback host

## `codex_desktop_host_adapter` Retirement Contract

`codex_desktop_host_adapter` 只允许承担迁移桥角色，不再承载新的设计中心语义。

- 当 alias 存在时，它必须镜像 `codex_desktop_adapter` 的 contract 语义；
  允许存在的差异只限于 `metadata.adapter_id`、兼容注解、或显式 legacy
  alias 标记。
- 新文档、新调用方、新 artifact 不得再把
  `codex_desktop_host_adapter` 记为 canonical desktop identity。
- Desktop / CLI parity、Rust bundle、以及后续 extracted artifacts 都必须以
  `cli_common_adapter` + `codex_desktop_adapter` 为真源，再决定是否额外附带 alias。
- 默认 runtime helper surface 也必须保持 canonical-only：`get_host_adapter`
  和 `list_host_adapters` 只返回正式 adapter；compatibility consumer 如需
  legacy surface，必须显式 opt-in compatibility lane / fallback lane。
- alias 的退场前提是：下游调用方迁移完成、双入口 parity 仍保持绿色、且
  连续性工件能说明剩余兼容风险与回滚点。
- alias 退场不意味着回退到 `aionrs` / `AionUI` 主线，也不允许借机把
  `codexcli` 抬升为控制器。

## Legacy Surface Guardrails

- default host peer set 固定为：
  `codex_desktop_adapter` / `codex_cli_adapter` / `claude_code_adapter` /
  `gemini_cli_adapter`
- `aionrs_companion_adapter`、`aionui_host_adapter` 与
  `codex_desktop_host_adapter` 都只能通过显式 legacy opt-in 被发现：
  - `codex_desktop_host_adapter` 进入 `compatibility_lane`
  - `aionrs_companion_adapter`、`aionui_host_adapter` 进入 `fallback_lane`
- legacy surface 必须带 `legacy_boundary` contract，至少声明：
  - `adapter_lifecycle = legacy-compatibility`
  - `default_host_peer_set_member = false`
  - `may_become_framework_truth = false`
  - removal readiness 与 migration guardrails
- `compatibility_snapshot()` 默认只暴露 canonical peer；只有显式
  `include_legacy_aliases=True` 时，才额外附带 compatibility / fallback lane。
- `build_upgrade_compatibility_matrix()` 默认也不得重新把 legacy surface 抬回
  primary peer inventory；legacy rows 只允许在显式 opt-in inventory 中出现。

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
- `autopilot` / `deepreview` 只保留为 framework-native alias：
  - canonical owner 在 framework
  - Claude / Codex 只暴露不同入口，不得分叉语义
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

## Current Minimal Implementation

已实现：

- `compile_cli_common_adapter(...)`
- `codex_agno_runtime.compatibility.compile_codex_common_adapter(...)`
- `codex_agno_runtime.compatibility.compile_aionrs_companion_adapter(...)`
- `codex_agno_runtime.compatibility.compile_aionui_host_adapter(...)`
- `compile_codex_desktop_adapter(...)`
- `compile_codex_cli_adapter(...)`
- `compile_claude_code_adapter(...)`
- `compile_gemini_cli_adapter(...)`
- `build_cli_family_capability_discovery(...)`
- `build_cli_family_parity_snapshot(...)`
- `build_codex_dual_entry_parity_snapshot(...)`
- framework-native alias registry for `autopilot` / `deepreview`
- OMC retirement contract in `RUNTIME_REGISTRY.json`
- CLI-family capability discovery now exposes:
  - `supervisor_capabilities`
  - `session_supervisor_driver`
  - `resume_command_examples`
  - `framework_alias_entrypoints`
- `codex_agno_runtime.compatibility.build_codex_desktop_alias_retirement_status(...)`
- `build_execution_kernel_live_fallback_retirement_status(...)`
- `build_execution_kernel_live_response_serialization_contract(...)`
- `codex_agno_runtime.compatibility.build_upgrade_compatibility_matrix(...)`
- `emit_framework_contract_artifacts(...)`: emits the core bridge / contract artifact set under `default/`; fallback host artifacts (`aionrs_companion_adapter`, `aionui_host_adapter`, `generic_host_adapter`) go to `fallback/`, legacy alias inventory / retirement status go to `continuity/`, and Rust companion outputs go to `rust/`, all behind explicit opt-in lanes where applicable.
- `router-rs --profile-json --framework-profile <path>` for Rust-side profile compilation
- thin projection / validation helpers used by the adapter contract and tests
- `scripts/materialize_cli_host_entrypoints.py` for repo-level `AGENTS.md` /
  `CLAUDE.md` / `GEMINI.md` materialization plus `.claude/` / `.gemini/` bootstrap

这为下一步继续把 CLI-family parity、artifact layout、Rust contract lane 收口到
跨宿主 CLI 主线打下接口基础，并保持 adapter 只做编译与投影，不承接
framework core 治理。
其中 `codex_desktop_host_adapter` 继续保留，但文档语义已经收紧为
compatibility-only bridge，而不是下一阶段的命名中心。
当前 artifact lane 默认不再产出 alias inventory / retirement status；这两项
只在显式 continuity opt-in 时保留，用来证明 alias 仍然只是迁移桥，而不
是新的 desktop 真源。
`codex_agno_runtime.compatibility.build_upgrade_compatibility_matrix(...)`
也只提供 compatibility inventory / smoke evidence，不承担 canonical parity truth。
默认 artifact emission 也不再写出这份 secondary inventory；如需这类兼容清单，
必须显式开启 compatibility inventory lane。
当前默认 artifact emission 已不再把 `codex_desktop_host_adapter` 作为一等
输出；如需兼容 continuity lane，必须显式 opt-in legacy alias artifact。
同样，legacy / compatibility compiler 入口不再作为根包 `codex_agno_runtime`
的 public export；兼容消费者必须显式改走
`codex_agno_runtime.compatibility.compile_codex_common_adapter(...)`、
`codex_agno_runtime.compatibility.build_codex_desktop_alias_retirement_status(...)`
等显式 compatibility surface。
其中 `codex_desktop_host_adapter` 的 Python 直编译入口已移除；如需产出 legacy
alias artifact，必须走显式 continuity opt-in 的 artifact lane，由 Rust 兼容面
负责写出。
默认 lookup / registry helper 也已与该收口方向对齐：不显式开启
compatibility lane 时，`codex_desktop_host_adapter` 不再作为 peer adapter
出现在 runtime helper surface。
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
`.claude/agents/`、`.claude/settings.json`、`.gemini/settings.json` 允许存在，
但它们只承载 host-private surface，不得分叉 shared routing / memory truth。
- repo entrypoint projection 还要区分两类输出约束：
  machine continuity contract 继续以 `SESSION_SUMMARY.md`、`NEXT_ACTIONS.json`、
  `TRACE_METADATA.json`、`.supervisor_state.json` 为恢复真源；human closeout
  contract 只约束给用户看的最终收尾话术。
- human closeout contract 默认应压缩成一小段人话，只讲“现在达到什么效果”与
  “接下来还需要什么”；如果没有后续，就直接声明已完成。
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
