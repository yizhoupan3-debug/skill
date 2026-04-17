# Framework Profile Contract

## Purpose

`framework_profile` 是外层融合框架的稳定真源，用来承载跨宿主复用的运行时配置、memory、artifact、orchestration、approval、tool policy、loadout policy、host capability requirements 等能力。

它不是 `aionrs` 配置镜像，也不是 AionUI 私有 schema。任何 host adapter 都只能消费这个 contract，不能反向把框架核心写死到单一宿主协议。
当前宿主推广目标已经扩到 `codex cli`、`claude code cli`、`gemini cli`；
这三个 CLI 仍只允许作为 `framework_profile` 的薄投影消费方。
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
目前默认 `emit_framework_contract_artifacts(...)` 会保留
`cli_common_adapter` / `cli_family_parity_snapshot` /
`codex_dual_entry_parity_snapshot` / inventory / retirement-status artifact /
live-fallback retirement-status artifact，
但不再默认写出 `codex_desktop_host_adapter` payload；兼容输出需要显式
opt-in。
同样，默认 runtime helper lookup 也保持 canonical-only：legacy alias 不再
通过通用 registry / lookup surface 作为 peer adapter 暴露；兼容消费者必须
显式走 compatibility escape hatch 或开启 legacy alias opt-in。

下一轮 runtime 深化会继续沿着这个边界推进：

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
  `execution_kernel_live_fallback_retirement_status` /
  `execution_kernel_live_response_serialization_contract`
- 当 Python emitter 同时请求 Rust artifacts 时，
  `emit_framework_contract_artifacts(...)` 现在还会额外产出
  `rust_python_artifact_parity_report.json`，把 Python / Rust 一等 artifact
  对齐关系外显成回归工件，而不是继续依赖人工抽查
- Rust 现在也会在 contract/artifact lane 内直接编译
  `codex_desktop_alias_retirement_status.inventory_summary`，通过仓库扫描把 alias
  retirement summary 纳入 parity report，而不是继续把这部分留给 Python-only
  emitter
- Rust 现在还会把 live Python fallback 的退休准备状态外显成
  `execution_kernel_live_fallback_retirement_status`，把“当前仍是 compatibility
  fallback、删除会进入 runtime control-flow lane”固定成 shared contract，而不是
  让这类判断继续散落在 runtime 代码注释里
- 这个 artifact 现在还会固定公开的 execution-kernel metadata 字段、
  dry-run/live delegate 关系，以及哪些 retirement gates 已满足或仍阻塞，
  这样后续判断是否能删 fallback 时，不需要再把 `services.py` 当成隐式真源
- 其中 `execution_kernel_delegate_family` /
  `execution_kernel_delegate_impl` 现在已经进入稳定 execution-kernel contract
  lane；调用方可以直接经由共享 contract 读取 dry-run/live delegate 的
  family/impl，而不需要反推某个宿主实现细节
- 它现在还会显式列出 remaining Python-owned surfaces，例如 dry-run prompt
  preview、compatibility fallback agent factory、compatibility live response
  serialization 与 fallback-reason metadata，方便后续一项项退休
- 新的一等 artifact
  `execution_kernel_live_response_serialization_contract` 还会把当前
  `RunTaskResponse` 的 top-level fields、usage contract、以及 live primary /
  compatibility fallback / dry-run 的 response metadata invariant 固定成 shared
  contract evidence；这仍属于 artifact/parity lane，不是 runtime control-flow
  rewrite
- 它现在还会把 response-only runtime metadata surface 一并 contract 化，
  其中 compatibility-owned 的 `execution_kernel_fallback_reason` 继续只停留在
  fallback response metadata / retirement artifact，不提升成 framework truth
- Claude hook path / policy / event metadata 只允许停留在 adapter
  `host_projection`；它们不是 canonical `framework_profile` 字段，也不能倒灌成
  shared runtime truth
- `--profile-json` 默认保持 canonical peer set；只有显式 continuity opt-in
  时，legacy alias 才允许出现在 `compatibility_lane`，而不是重新回到 bundle
  顶层
