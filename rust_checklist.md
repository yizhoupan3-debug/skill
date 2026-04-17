# Rust 化执行清单

> 目标：在不回退到 Python-first、也不把宿主私有语义反写进 framework truth 的前提下，继续推进仓库更大范围的 Rust 化。
>
> 约束：本清单里的任务必须能**互不打扰地并行推进**；因此每项都给出**独占写入范围**、**禁止越界范围**、**交付物**与**验收标准**。

## 当前状态快照

- 2026-04-18 review 结论：上一轮 `R1-R9` 的真实完成度是 **8/9**，不是旧统计里的 `6/9`。
- 上一轮实际已完成：`R1`、`R2`、`R3`、`R4`、`R5`、`R6`、`R8`、`R9`。
- 上一轮唯一未闭环的主 lane：`R7 Storage backend family 扩展`。
- 随着 `R6` / `R8` / `R9` 已落地，之前挂起的两项后续工作现在可以提升为**下一轮并行 lane**：
  1. `Compaction / snapshot-delta / generation rollover` 真正实现
  2. `Compatibility fallback` 最终退场
- 设计边界说明：`memory extraction policy` 与 `prompt wording policy` 仍故意留在 Python policy layer；它们**不是**当前剩余 Rust 化工作的待办项。

## 上一轮归档状态（R1-R9）

| ID | 任务 | 状态 | 备注 |
|---|---|---|---|
| R1 | 文档叙事收口 | 已完成 | 主线叙事已统一到 `thin projection + Rust contract-first migration` |
| R2 | Legacy adapter 降级 | 已完成 | legacy boundary / compatibility lane / canonical peer set 已锁定 |
| R3 | Shared contract / common adapter 稳态化 | 已完成 | shared contract schema 与 artifact parity 已稳态化 |
| R4 | Capability discovery / host contract 收口 | 已完成 | CLI-family / Desktop discovery contract 已收口 |
| R5 | Route/result shaping 继续向 Rust 收口 | 已完成 | route/result/diff vocabulary 已由 Rust lane 定义并校验 |
| R6 | Stream bridge 远端化边界 | 已完成 | transport / handoff / replay anchor / cleanup-preserves-replay 已落地 |
| R7 | Storage backend family 扩展 | 进行中 | backend seam 与非-filesystem regression backend 已存在；剩余主债务收敛为 compaction family 与更强 live backend family |
| R8 | Sandbox lifecycle 落地 | 已完成 | lifecycle / budget / cleanup / failure isolation 已达 contract-backed minimal implementation |
| R9 | Memory / compression kernel Rust 化边界 | 已完成 | deterministic kernel 与 Python-owned policy boundary 已明确 |

### R7 当前状态

- `RuntimeStorageBackend`、`FilesystemRuntimeStorageBackend`、`InMemoryRuntimeStorageBackend` 已存在。
- `BackgroundJobStore` 与 `FilesystemRuntimeCheckpointer` 已能通过 backend seam 工作。
- `N1` 已完成：checkpoint / durable state 的 capability surface 已显式外露到 control-plane contract 与 health surface，filesystem 与 in-memory backend 都有定向回归锁定。
- 但当前 live 路径仍主要依赖 filesystem concrete backend；`compaction` / `snapshot-delta` 仍未 live。
- 因此，下一轮不再重开 `R1-R9`，而是把剩余工作重组为新的并行 lane。

---

## 下一轮并行任务总表

- 本轮可并行执行任务总数：**3 项**
- 并行原则：**每项只改自己负责的文件族，不跨 lane 改公共 owner 文件**
- 完成定义：**3 项都满足各自验收标准，且默认 live/runtime/control-plane authority 仍保持 Rust-first**

| ID | 任务 | 主要目标 | 独占写入范围 |
|---|---|---|---|
| N1 | Storage backend family 真正闭环 | 把 backend seam 从“可测试”推进到“live path 可替换” | `codex_agno_runtime/src/codex_agno_runtime/checkpoint_store.py`, `codex_agno_runtime/src/codex_agno_runtime/state.py`, `tests/test_codex_agno_runtime_state_checkpoint_control_plane.py`，必要时新增 backend-family 定向测试文件 |
| N2 | Compaction / snapshot-delta / generation rollover 落地 | 把 compaction contract 从 design-only 推到最小可验证实现 | `codex_agno_runtime/src/codex_agno_runtime/trace.py`, `docs/runtime_compaction_contract.md`, `tests/test_runtime_compaction_contracts.py`, `tests/test_codex_agno_runtime_trace.py` |
| N3 | Compatibility fallback 最终退场 | 去掉 live kernel 的兼容 fallback 运行路径，只保留 Rust-only steady state | `codex_agno_runtime/src/codex_agno_runtime/execution_kernel.py`, `codex_agno_runtime/src/codex_agno_runtime/execution_kernel_contracts.py`, `codex_agno_runtime/src/codex_agno_runtime/host_adapters.py`, `codex_agno_runtime/src/codex_agno_runtime/profile_artifacts.py`, `docs/rust_contracts.md`, `tests/test_execution_kernel_router_rs_contract.py`, `tests/test_framework_profile_adapters.py`, `tests/test_framework_contract_artifacts.py` |

---

## N1. Storage backend family 真正闭环

### 当前状态

- 2026-04-18 review：`RuntimeStorageBackend` seam 已存在，`BackgroundJobStore` / `FilesystemRuntimeCheckpointer` 也已支持 filesystem 与 in-memory backend 回归。
- 2026-04-18 execution：backend capability surface 现已成为 checkpoint / durable state 的显式 control-plane 语义，`supports_atomic_replace` / `supports_compaction` / `supports_snapshot_delta` / `supports_remote_event_transport` 都会随 backend family 一起落到 manifest / transport / health surface。
- 当前 live runtime 仍主要站在 filesystem concrete backend 上，但非-filesystem concrete backend 语义已经通过定向 regression 稳定接入。

### 目标

把 runtime storage/checkpointer 从“已有 seam + test backend”推进到“live path 真正可替换的 backend family”。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/checkpoint_store.py`
- `codex_agno_runtime/src/codex_agno_runtime/state.py`
- `tests/test_codex_agno_runtime_state_checkpoint_control_plane.py`
- 如确有必要，可新增一个只覆盖 backend family 的定向测试文件，但不得触碰 `trace.py` 相关测试文件

### 禁止越界

- 不改 `trace.py`
- 不改 `docs/runtime_compaction_contract.md`
- 不改 `execution_kernel.py` / `execution_kernel_contracts.py`
- 不改 route / adapter / sandbox lane

### 交付物

- 更清晰的 backend capability surface
- 至少一个可被 runtime 稳定接入的非-filesystem concrete backend 语义
- backend-family regression tests

### 验收标准

- `RuntimeCheckpointer` 与 durable background state 的关键读写语义由 backend family 统一承载，而不是隐含依赖 `Path` 直写
- filesystem 与非-filesystem backend 都能通过定向回归测试
- `supports_compaction` / `supports_snapshot_delta` / `supports_remote_event_transport` 等 capability 字段保持显式、可验证、不可隐式漂移

### 执行结果

- 已完成：checkpoint manifest、transport binding、durable background state、health surface 现在都显式携带 backend capability 字段。
- 已完成：filesystem 与 in-memory backend 均可通过同一 backend seam 完成定向 round-trip，不再把关键语义隐含绑定到 `Path` 直写。
- 验证通过：
  - `pytest -q tests/test_codex_agno_runtime_state_checkpoint_control_plane.py`
  - `pytest -q tests/test_codex_agno_runtime_trace.py tests/test_codex_agno_runtime_services.py tests/test_codex_agno_runtime_runtime.py`

---

## N2. Compaction / snapshot-delta / generation rollover 落地

### 当前状态

- 2026-04-18 execution：`trace.py` 现已具备最小 compaction lane：支持型 backend 会写入 stable snapshot / artifact refs / generation-local delta / manifest，并把 active stream rollover 到 successor generation。
- `replay()` 与 `recover_compacted_state()` 现在都能基于 `latest stable snapshot + generation-local deltas` 找到最新可恢复 generation，而不是只依赖全量历史流。
- 当前 filesystem / in-memory 默认 backend 仍保持 fail-closed，因为 `supports_compaction` / `supports_snapshot_delta` 仍由 backend capability surface 显式控制；这属于刻意保留的 `N1` 只读前提，不是 `N2` 未实现。

### 目标

把 compaction 边界从“冻结契约”推进到“最小实现可验证”，同时保持 replay / recovery / artifact ref 语义稳定。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/trace.py`
- `docs/runtime_compaction_contract.md`
- `tests/test_runtime_compaction_contracts.py`
- `tests/test_codex_agno_runtime_trace.py`

### 禁止越界

- 不改 `checkpoint_store.py`
- 不改 `state.py`
- 不改 `execution_kernel.py` / `execution_kernel_contracts.py`
- 不改 host adapter / profile artifact lane
- 如发现需要新增 backend capability，只记录缺口，不直接改 N1 文件

### 交付物

- compaction snapshot / delta / generation rollover 的最小实现
- replay / recovery / artifact ref 回归测试
- contract 与 live 行为对齐说明

### 验收标准

- trace/replay 在 compaction 后仍能找到最新可恢复 generation，不破坏 cursor 语义
- snapshot / delta / artifact ref 的恢复路径可测试验证
- compaction 不会删除 recovery 所必需的 artifact 引用或 supervisor/trace 连续性信息
- 对不支持 compaction 的 backend，行为必须是显式 contract 化的 fail-closed / no-op，而不是隐式部分支持

---

## N3. Compatibility fallback 最终退场

### 当前状态

- 2026-04-18 review：默认 live path 已是 Rust-only，`execution_kernel_live_fallback_*` 元数据与 retirement status artifact 也已外显。
- 但兼容 fallback 的最终控制流与配套 contract/status 仍然存在，属于尚未收口的连续性债务。

### 目标

把 live kernel 从“默认禁用但仍保留 compatibility escape hatch”推进到“steady-state 不再提供可运行 fallback 路径”。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/execution_kernel.py`
- `codex_agno_runtime/src/codex_agno_runtime/execution_kernel_contracts.py`
- `codex_agno_runtime/src/codex_agno_runtime/host_adapters.py`
- `codex_agno_runtime/src/codex_agno_runtime/profile_artifacts.py`
- `docs/rust_contracts.md`
- `tests/test_execution_kernel_router_rs_contract.py`
- `tests/test_framework_profile_adapters.py`
- `tests/test_framework_contract_artifacts.py`

### 禁止越界

- 不改 `runtime.py` / `trace.py`
- 不改 `checkpoint_store.py` / `state.py`
- 不改 sandbox / memory-compression lane
- 不通过恢复 Python live path 或新增 host-specific branching 来达成“退场”

### 交付物

- fallback 最终退场后的 live kernel contract
- 精简后的 artifact / status / response metadata 语义
- retirement regression tests

### 验收标准

- 正常 live execution 与 dry-run contract 均保持 Rust-only steady state
- 显式 compatibility fallback 请求要么不可表达，要么被明确拒绝，而不是再落回 Python 执行路径
- response metadata / health / artifact 不再把 compatibility fallback 表达成可运行 steady-state 能力
- 不再存在 live primary 失败后可回落到 Python-agno kernel 的控制流分支

---

## 不纳入本轮并行项的内容

以下内容**不纳入本轮**，因为它们不是当前 Rust 化剩余债务，或会重新把 policy authority 搞混：

1. **把 memory extraction policy 搬进 Rust**
   - 当前只允许 Rust 接管 deterministic kernel，不接管 policy。
2. **把 compression wording / summarization policy 搬进 Rust**
   - 当前只允许 Rust 接管 deterministic pruning mechanics。
3. **把 Claude / Gemini / Codex 宿主私有 hook/config 语义下沉到 runtime kernel**
   - 宿主投影仍然只是 host-private contract surface，不是 framework truth。

---

## 建议执行顺序

这 3 项可以并行，但建议按下面方式组织：

### 第一优先级
- `N1` Storage backend family 真正闭环
- `N3` Compatibility fallback 最终退场

### 第二优先级
- `N2` Compaction / snapshot-delta / generation rollover 落地

### 并行注意事项
- `N2` 必须把 `N1` 暴露出来的 backend capability surface 当作只读前提；如果发现能力不够，记录缺口，等下一轮再开，不直接改 `N1` 文件。
- `N3` 只收口 live fallback 与 artifact/status 语义，不碰 trace/storage/sandbox。

---

## 本轮总体验收线

当且仅当以下条件全部满足，可认为“本阶段剩余 Rust 化工作”完成：

1. `N1`、`N2`、`N3` 全部完成，且都通过各自验收标准。
2. 默认 live/runtime/control-plane authority 仍保持 Rust-first，不回退到 Python-first。
3. backend family、compaction、fallback retirement 都有对应回归测试锁定。
4. `framework_profile` 仍是唯一真源，没有被任一 host/CLI 私有语义污染。
5. 本轮没有通过恢复 Python fallback、增加 host-specific special case、或绕过 frozen contract 来换取通过率。

## 当前完成统计模板

- 当前轮次总任务数：**3**
- 已完成：`1/3`
- 进行中：`0/3`
- 未开始：`2/3`

可按以下格式持续更新：

- [ ] N1 Storage backend family 真正闭环
- [x] N2 Compaction / snapshot-delta / generation rollover 落地
- [ ] N3 Compatibility fallback 最终退场
