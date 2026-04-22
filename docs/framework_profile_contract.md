# Framework Profile Contract

## Purpose

`framework_profile` 是外层融合框架的稳定真源，用来承载跨宿主复用的运行时配置、memory、artifact、orchestration、approval、tool policy、loadout policy、host capability requirements 等能力。

它不是 `aionrs` 配置镜像，也不是 AionUI 私有 schema。任何 host adapter 都只能消费这个 contract，不能反向把框架核心写死到单一宿主协议。
当前宿主推广目标已经扩到 `codex cli`、`claude code cli`、`gemini cli`；
这三个 CLI 仍只允许作为 `framework_profile` 的薄投影消费方。
无论是 `codex cli`、`claude code cli` 还是 `gemini cli`，都不是 framework
truth；`codexcli` 也不例外，它只是 headless execution entrypoint。
当前主线口径统一为 `thin projection + Rust contract-first migration`：
先守住 host 只做投影的边界，再把 shared contract / artifact / parity /
discovery 这些稳定层持续 Rust 化。

## Boundary

- 允许落在外层框架、adapter、bridge、contract、artifacts、docs、tests。
- 禁止把框架核心嵌入 `aionrs` 内核。
- 禁止把 AionUI UI 壳层协议当成唯一真源。
- host adapter 可以读取 `host_capability_requirements` 做宿主能力解析，但不能反向改写 framework core 语义。
- 必须保证没有 `aionrs` 时，Codex Desktop 或 generic host 仍可运行：
  - runtime
  - memory
  - artifact
  - orchestration

## Canonical Fields

- `profile_id`
- `display_name`
- `framework_profile_version`
- `runtime_family`
- `host_family`
- `core_capabilities`
- `optional_capabilities`
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
- `metadata`

共享 contract artifact lane 还会把以下 execution/delegation/state descriptor
作为一等可发射工件挂在 adapter 共享 contract 上：

- `execution_controller_contract`
- `delegation_contract`
- `supervisor_state_contract`

## Invariants

1. `core_capabilities` 必须至少包含 `runtime / memory / artifact / orchestration`。
2. `host_family` 可标记当前宿主类别，但 framework core 不允许直接固定成 `aionrs`。
3. adapter 只能投影 `framework_profile`，不能篡改其核心语义。
4. nested policy 使用 merge override，不使用 host-specific hard fork。
5. artifact / memory / orchestration 的 schema 以框架 contract 为中心，不以单个 host 事件流为中心。

## Current Minimal Implementation

外层 Python contract 已实现：

- `FrameworkProfile`
- `build_framework_profile(...)`
- `merge_profile_overrides(...)`
- `ensure_capabilities(...)`
- `resolve_host_capability_requirements(...)`
- `emit_framework_contract_artifacts(...)`

它们为下一阶段 bridge / contract emission / runtime handshake 提供稳定入口，并把 nested override、host 能力解析和跨宿主复用边界固定在外层框架。
当前默认 `emit_framework_contract_artifacts(...)` 继续保留
`cli_common_adapter` / `cli_family_parity_snapshot` /
`codex_dual_entry_parity_snapshot` / inventory / contract / parity artifacts，
但 `codex_desktop_host_adapter` 只会出现在显式 compatibility escape hatch
里，不再作为默认 peer 输出。
默认 runtime helper lookup 也保持 canonical-only：legacy alias 不再通过通用
registry / lookup surface 作为 peer adapter 暴露；需要兼容 payload 的调用方
必须显式 opt-in compatibility lane。
execution-kernel 相关 contract 现在以 Rust-only 默认执行、prepare_session /
dry-run preview 走 router-rs、以及隔离的 compatibility lane 为主线，不再保留
过渡期口径作为 steady-state 叙事。

下一轮 runtime 深化仍然沿着这个边界推进：

- `framework_profile` 仍只定义 host-neutral truth
- runtime control plane 可以继续变强，但不能把宿主私有控制语义反写回
  `framework_profile`
- DeerFlow 2.0 式的 harness/app split、run-manager、stream bridge 只可作为
  runtime 借鉴，不改变 `framework_profile` 的真源定位

## Rust Lane

`framework_profile` 现在也可以被 Rust route/compiler lane 消费：

- `scripts/router-rs --profile-json --framework-profile <path>`
- Rust 侧会校验同样的核心边界：
  - 不允许把 framework core 直接固定到 `aionrs`
  - `core_capabilities` 仍必须覆盖 `runtime / memory / artifact / orchestration`
- Rust 输出的是外层 contract 的 companion projection，不是 `aionrs` 内核 patch
- Rust 也会镜像 CLI-family adapter artifacts：
  `cli_common_adapter` / `codex_common_adapter` /
  `codex_cli_adapter` / `claude_code_adapter` / `gemini_cli_adapter` /
  `cli_family_capability_discovery` / `cli_family_parity_snapshot` /
  `execution_controller_contract` / `delegation_contract` /
  `supervisor_state_contract` /
  `execution_kernel_delegate_family` /
  `execution_kernel_delegate_impl` /
  `execution_kernel_live_response_serialization_contract`
- 当 Python emitter 同时请求 Rust artifacts 时，
  `emit_framework_contract_artifacts(...)` 也会写出
  `rust_python_artifact_parity_report.json`，把 Python / Rust 一等 artifact 的
  对齐关系外显成回归工件，而不是依赖人工抽查
- Rust 现在会在 contract/artifact lane 内直接编译
  `codex_desktop_alias_retirement_status.inventory_summary`，但这类结果只用于
  parity / compatibility 证明，不再承载过渡期叙事
- `prepare_session(...)` 和 dry-run preview 已经走 router-rs，因此 Rust 侧
  现在是默认 runtime 路径，live execution 也以 Rust-only 为默认值
- runtime control plane 现在也通过 Rust authority descriptor 对外声明
  `router / state / trace / memory / background` 的默认 ownership；Python
  仅保留 thin projection / backend bridge 角色，不再是 steady-state authority
- compatibility lane 仍然保留，但只作为显式 continuity-only / compatibility-only
  escape hatch；`codex_desktop_host_adapter` 不再回到默认 peer set
- 共享 contract 现在只保留 compatibility-only metadata；steady-state truth 不再
  维护过渡期清单
- `execution_kernel_fallback_reason` 仅在 compatibility payload / legacy metadata
  中保留语义，不进入 framework truth 或默认控制流
- `rust_execute_fallback_to_python` 这条 retired explicit-request surface 已经移除；
  现在只保留 retirement artifact 作为历史说明，steady-state runtime 与 config
  不再暴露设置字段或 env var，也不会再维护“显式请求后返回 rejection”的请求面
- Claude hook path / policy / event metadata 只允许停留在 adapter
  `host_projection`；它们不是 canonical `framework_profile` 字段，也不能倒灌成
  shared runtime truth
- `--profile-json` 默认保持 canonical peer set；只有显式 continuity opt-in
  时，legacy alias 才允许出现在 `compatibility_lane`，而不是重新回到 bundle
  顶层
- continuity artifacts 仍以 repo root 为恢复锚点；`artifacts/current/*` 只是给
  bridge / aggregation 消费的当前会话镜像。二者必须同步，但不能各自声明独立真源。
- `./.codex/memory/` 是共享逻辑路径；当前物理落点通过 symlink 指向 `./memory/`，
  所以 host/bridge 文档必须把这两者描述为同一 memory root。
