# Framework Profile Contract

## Purpose

`framework_profile` 是外层融合框架的稳定真源，用来承载跨宿主复用的运行时配置、memory、artifact、orchestration、approval、tool policy、loadout policy、host capability requirements 等能力。

它不是 `aionrs` 配置镜像，也不是 AionUI 私有 schema。任何 host adapter 都只能消费这个 contract，不能反向把框架核心写死到单一宿主协议。
当前宿主推广目标已经扩到 `codex cli`、`claude code cli`、`gemini cli`；
这三个 CLI 仍只允许作为 `framework_profile` 的薄投影消费方。
无论是 `codex cli`、`claude code cli` 还是 `gemini cli`，都不是 framework
truth；`codexcli` 也不例外，它只是 headless execution entrypoint。
当前主线口径统一为 `Rust-owned contract truth + thin host projection`：
shared contract、workspace bootstrap、memory/mcp/session normalization、host
adapter projection、artifact emission、compatibility inventory、parity snapshot
都由 `router-rs` 编译。旧 Python projection 已退场；不能再维护第二套默认值、
bridge 表、fallback emitter 或 Python/Rust parity lane。

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
- `framework_surface_policy`
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
6. `workspace_bootstrap` 的补默认与桥接归一化只允许来自 `router-rs`
   profile compiler；adapter 的 `host_overrides` 已退出 artifact emission，不能回写
   `cli_common_adapter.shared_contract`、`common_contract`、`runtime_surface`
   里的 shared contract truth。
   同理，adapter payload 里的 `host_capability_requirements` 只允许是当前
   `host_id + adapter_id` 解析后的结果，不能把 `framework_profile` 里的原始
   requirements 总表整包重新发成 host-facing contract。
7. `metadata` 必须保持 host-neutral；像 `transport`、`context_files`、
   `mcp_config_paths`、`hook_event_names`、`settings_scope_order` 这类宿主投影字段，
   必须直接拒绝写进 framework truth。
8. `bridge_contract` 只能从 canonical shared contract 的
   `workspace_bootstrap.bridges` 投影出来，不能再单独维护第二份 bridge 默认值。

## 字段所有权说明

`framework_profile` 的字段只允许三类写入所有权：

- framework-owned（真源）：所有 `Canonical Fields`（含
  `execution_controller_contract`、`delegation_contract`、`supervisor_state_contract`）默认由外层框架持有，adapter 只能读、过滤、重投影，不能改写这些字段的语义。
- projection-owned（宿主投影）：host 私有运行语义只能在 host projection 层表达，例如
  `transport`、`context_files`、`mcp_config_paths`、`hook_event_names`、`settings_scope_order`
  等字段。它们只允许出现在兼容/continuity lane 或 host_projection 输出，不可进入 canonical 真源层。
- continuity-owned：兼容债务证据（如 alias retirement / compatibility inventory）仅在显式
  continuity/compatibility lane 中输出，不能进入 steady-state 默认集合。

不可回落规则（重点）：

- canonical lane 与 default lane 不得回落到 host 私有语义；一旦 host-only 字段试图回填
  `framework_profile` 真源字段，应直接拒绝并报错。
- `metadata` 只能承载 host-neutral 描述；若出现 host 私有子键，必须将其移到 projection/continuity lane，或移除再发出。
- 通过 `shared_contract_surface()` 或 Rust 对应 lane 产物生成的 canonical output
  是最终真源落点，adapter 只能在该输出之外承接私有语义。

## Surface Compaction Direction

为避免和当前 Rust 主线冲突，后续收口优先落在外层 surface policy，而不是重新改
runtime kernel 语义。

- 核心主航道只保留 4 个轴：
  `routing / memory / continuity / host_projection`
- 其余能力默认都当成 opt-in capability：
  通过 loadout、tier、或显式 compatibility lane 开启
- 默认面只保留一条正规路径：
  `default_surface_loadout`
  是默认启用面；research / implementation / audit / framework / ops 都是显式
  opt-in
- 技能体系按 `core / optional / experimental / deprecated` 分级：
  `experimental` 默认不进默认面，`deprecated` 默认禁用
- 物理边界必须继续分开：
  source roots、compiled outputs、generated artifacts、session artifacts
  不允许混写
- 后续评价主指标改成 4 个结果：
  第一次成功率、跨宿主一致性、断点恢复成功率、新任务接入成本

这些收口规则的机器可读真源放在
`configs/framework/FRAMEWORK_SURFACE_POLICY.json`，并由
`skills/SKILL_LOADOUTS.json` 与 `skills/SKILL_TIERS.json` 共同支撑。
当 artifact emitter 写出 `framework_profile.json`、CLI common adapter、以及
多宿主 parity artifacts 时，也应同步把这份 policy 作为 shared contract 的一等字段
和独立 artifact 带出，而不是只留在 repo 配置目录里。

## Current Implementation

`router-rs` 是 profile/shared-contract/adapter/artifact 的真源。

- `router-rs --profile-json --framework-profile <path>` 编译 profile bundle。
- `router-rs --profile-artifacts-json --framework-profile <path>` 编译默认
  contract artifacts、adapter artifacts、capability discovery、parity snapshot、
  control-plane contracts、compatibility inventory。
- 旧 `emit_framework_contract_artifacts(...)` Python 入口不再是当前实现面；
  contract/artifact 输出由 `router-rs` 直接生成和校验。
- 旧 Python `FrameworkProfile.shared_contract_surface()`、memory/mcp/session
  normalization、workspace bootstrap projection 已退场；这些语义由 Rust 编译结果表达。
- memory policy 的持久化语义也属于 Rust 真源：SQLite row、stable
  `decisions.md` journal、source/fact 计数都由 `router-rs
  --framework-memory-policy-json` 发出，Python 不再补写第二份 memory artifact。
- artifact emission 不再写 fallback host artifacts，不再写
  `rust_python_artifact_parity_report.json`，不再写
  `codex_desktop_alias_inventory.json`。
- `host_overrides` 不再参与 artifact emission；需要宿主私有字段时必须进入
  Rust host projection 输出。

当前默认 contract/artifact emission 只发布 Rust 编译出的
`cli_common_adapter` / `cli_family_parity_snapshot` /
`codex_dual_entry_parity_snapshot` / contract / parity artifacts。`upgrade_compatibility_matrix`
和 `codex_desktop_alias_retirement_status` 只在显式 Rust continuity/inventory
lane 中输出；`aionrs_companion_adapter`、`aionui_host_adapter`、`generic_host_adapter`
不再作为 fallback host artifacts 写出。

`workspace_bootstrap` 的 steady-state 口径也同样收敛到一条线：
Rust shared contract 里的 `workspace_bootstrap.bridges` 是唯一 bridge 默认来源。
CLI common adapter、Codex Desktop adapter、Codex CLI adapter、Claude Code adapter、
Gemini CLI adapter 的 `common_contract` / `runtime_surface` 都必须等于这份
surface。`bridge_contract` 只能从这份 bootstrap 的 `bridges` 字段投影，
不能和 shared contract 平行生长。

`host_capability_requirements` 也分成两层：`framework_profile` 顶层继续保留原始
requirements 总表，adapter payload 则只发当前宿主解析后的视图，避免把 host-private
差异重新回灌成 shared contract 漂移面。
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
- Python/Rust parity report 已退出；如果 artifact 与 Rust 产物不一致，修
  Rust 真源或调用边界，不允许新增 Python-owned ignored path。
- Rust 现在会在 contract/artifact lane 内直接编译
  `codex_desktop_alias_retirement_status.inventory_summary`，但这类结果只用于
  parity / compatibility 证明，不再承载过渡期叙事
- `prepare_session(...)` 和 dry-run preview 已经走 router-rs，因此 Rust 侧
  现在是默认 runtime 路径，live execution 也以 Rust-only 为默认值
- runtime control plane 现在也通过 Rust authority descriptor 对外声明
  `router / state / trace / memory / background` 的默认 ownership；host 侧只保留
  thin projection / backend bridge 角色，不再是 steady-state authority
- compatibility lane 仍然保留，但只作为显式 continuity-only / compatibility-only
  lane；`codex_desktop_host_adapter` 不再回到默认 peer set
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
- `controller_boundary.host_entrypoints` 的默认 peer set 固定为
  `codex_desktop_adapter + codex_cli_adapter + claude_code_adapter + gemini_cli_adapter`；
  compatibility alias 只能留在显式 Rust continuity/inventory lane，fallback
  host artifacts 不再由默认 emitter 产出
- continuity artifacts 仍以 repo root 为恢复锚点；`artifacts/current/*` 只是给
  bridge / aggregation 消费的当前会话镜像。二者必须同步，但不能各自声明独立真源。
- `./.codex/memory/` 是共享逻辑路径；当前物理落点通过 symlink 指向 `./memory/`，
  所以 host/bridge 文档必须把这两者描述为同一 memory root。
- stable memory journal 的默认写入点是同一 memory root 下的 `decisions.md`；
  consumer 只能读取这份 Rust 产物，不能另起 Python journal writer。
