# Rust 化执行清单

> 目标：在不回退到 Python-first、也不把宿主私有语义反写进 framework truth 的前提下，继续推进仓库更大范围的 Rust 化。
>
> 约束：本清单只记录**当前这一轮**任务；所有任务必须能**互不打扰地并行推进**，因此每项都给出**独占写入范围**、**禁止越界范围**、**交付物**与**验收标准**。

## 当前状态快照

- 上一轮 checklist 已完成，本文件不再保留历史归档与旧轮次编号。
- 默认 live/runtime/control-plane authority 仍保持 Rust-first；Python 当前只保留 thin projection、diagnostic lane、以及少量明确标注的 continuity / compatibility surface。
- 本轮 `1-5` 已全部执行完成；当前剩余工作不在本文件内续写，后续应新开下一 safe slice。
- 当前下一 safe slice 主要集中在：
  - 非-filesystem concrete backend 的 steady-state 强化
  - process-external runtime event transport / attach bridge
  - route diagnostic lane 的进一步去 Python 依赖
  - alias / continuity artifact surface 的继续收窄
  - observability exporter / metrics implementation 的 concrete 化

---

## 本轮并行任务总表

- 本轮可并行执行任务总数：**5 项**
- 并行原则：**每项只改自己负责的文件族，不跨 lane 改公共 owner 文件**
- 完成定义：**5 项都满足各自验收标准，且默认 live/runtime/control-plane authority 仍保持 Rust-first**

| ID | 任务 | 主要目标 | 独占写入范围 |
|---|---|---|---|
| 1 | SQLite backend steady-state 强化 | 把已存在的非-filesystem backend 从“可用能力面”推进到更强的 steady-state runtime backend | `codex_agno_runtime/src/codex_agno_runtime/checkpoint_store.py`, `codex_agno_runtime/src/codex_agno_runtime/state.py`, `codex_agno_runtime/src/codex_agno_runtime/config.py`, `tests/test_codex_agno_runtime_state_checkpoint_control_plane.py`，必要时新增 backend-family 定向测试 |
| 2 | Runtime event transport 外部 attach bridge 落地 | 把当前本地/内存 transport seam 推进到 process-external attach / handoff / replay-capable bridge | `codex_agno_runtime/src/codex_agno_runtime/runtime.py`，必要时新增 `codex_agno_runtime/src/codex_agno_runtime/event_transport.py`，`tests/test_codex_agno_runtime_runtime.py`，必要时新增 transport 定向测试 |
| 3 | Route diagnostic lane 继续 de-Pythonization | 继续缩小 verify / shadow / rollback 语义对 Python route execution 的依赖 | `codex_agno_runtime/src/codex_agno_runtime/services.py`, `codex_agno_runtime/src/codex_agno_runtime/schemas.py`, `codex_agno_runtime/src/codex_agno_runtime/rust_router.py`, `scripts/router-rs/src/main.rs`, route-policy / parity 相关测试文件 |
| 4 | Desktop continuity artifact surface 再收口 | 继续压缩 alias / continuity artifact 的默认暴露面，只保留 continuity-only 最小证据 | `codex_agno_runtime/src/codex_agno_runtime/host_adapters.py`, `codex_agno_runtime/src/codex_agno_runtime/profile_artifacts.py`, `codex_agno_runtime/src/codex_agno_runtime/compatibility.py`, `docs/host_adapter_contracts.md`, `docs/upgrade_compatibility_matrix.md`, `tests/test_framework_profile_adapters.py`, `tests/test_framework_contract_artifacts.py` |
| 5 | Observability exporter / metrics concrete implementation | 把已冻结的 observability contract 落到更具体的 exporter / metric path，而不是只停在 vocabulary ownership | `docs/runtime_observability_contract.md`，必要时新增 `codex_agno_runtime/src/codex_agno_runtime/observability.py`，`tests/test_runtime_observability_contracts.py`，必要时新增 exporter / metrics 定向测试 |

---

## 1. SQLite backend steady-state 强化

### 当前状态

- backend family seam 与 capability surface 已稳定，filesystem / in-memory lane 已被锁定。
- SQLite 这类更强的非-filesystem backend 已不再只是纯 regression double 候选，但其 steady-state runtime 能力还未完全收口成默认可信的 concrete implementation 面。

### 目标

把 SQLite backend 从“可存在、可验证”推进到“更接近 steady-state runtime backend”，同时保持 capability contract 显式、稳定、fail-closed。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/checkpoint_store.py`
- `codex_agno_runtime/src/codex_agno_runtime/state.py`
- `codex_agno_runtime/src/codex_agno_runtime/config.py`
- `tests/test_codex_agno_runtime_state_checkpoint_control_plane.py`
- 如确有必要，可新增一个只覆盖 backend family / sqlite lane 的定向测试文件

### 禁止越界

- 不改 `trace.py`
- 不改 `runtime.py`
- 不改 `services.py`
- 不改 host adapter / route / observability lane
- 不把 backend-specific 假设重新散落回业务层手写逻辑

### 交付物

- 更强的 SQLite steady-state backend 路径
- 清晰的 backend selection / capability contract
- sqlite/backend-family regression tests

### 验收标准

- runtime 能在 filesystem 外，以 SQLite 这类更强 concrete backend 稳定完成 checkpoint / durable state 关键读写
- `supports_atomic_replace` / `supports_compaction` / `supports_snapshot_delta` / `supports_remote_event_transport` 仍保持显式 contract 字段
- 若某些能力仍未支持，必须 fail-closed 且 capability surface 明确外显，而不是伪装支持
- steady-state 路径不重新引回裸 `Path` 假设

---

## 2. Runtime event transport 外部 attach bridge 落地

### 当前状态

- runtime 已有 replay / handoff / binding artifact / local bridge seam。
- 但 process-external attachable transport 仍不够 concrete；当前更多还是 control-plane seam，而不是更完整的 runtime-usable external attach bridge。

### 目标

把 runtime event transport 从“本地 replay-capable seam”推进到“可被进程外 attach / handoff / resume 的 concrete bridge”。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/runtime.py`
- 如确有必要，可新增 `codex_agno_runtime/src/codex_agno_runtime/event_transport.py`
- `tests/test_codex_agno_runtime_runtime.py`
- 如确有必要，可新增 transport / handoff / attach 定向测试文件

### 禁止越界

- 不改 `trace.py`
- 不改 `services.py`
- 不改 `docs/runtime_observability_contract.md`
- 不改 storage backend / host adapter / route control-plane lane
- 不通过放宽 replay contract 来换取 transport “看起来可用”

### 交付物

- 更 concrete 的 external attach / handoff / resume transport lane
- transport / handoff / replay regression tests
- 持续保持 binding artifact / handoff descriptor 的可恢复性

### 验收标准

- runtime 能提供进程外 attach 所需的明确 transport descriptor / binding / handoff 信息
- attach / resume / cleanup / replay 语义不会因为 bridge concrete 化而漂移
- external transport 的存在不改变 JSONL vocabulary、OTel vocabulary、或 compaction contract
- 若仍未实现某些远端 transport 形态，必须显式声明“未实现”，不能以模糊 bridge 描述替代

---

## 3. Route diagnostic lane 继续 de-Pythonization

### 当前状态

- 默认 route authority 已 Rust-first。
- verify / shadow / rollback 等 diagnostic 语义仍有 Python participation，虽然已不是 live authority，但仍属于 residual Python diagnostic debt。

### 目标

继续把 route diagnostic lane 收口到 Rust contract lane，使 Python 只保留更窄的 diagnostic / legacy projection，而不是 route control-plane 的事实依赖。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/services.py`
- `codex_agno_runtime/src/codex_agno_runtime/schemas.py`
- `codex_agno_runtime/src/codex_agno_runtime/rust_router.py`
- `scripts/router-rs/src/main.rs`
- route-policy / route-parity 相关测试文件

### 禁止越界

- 不改 `runtime.py`
- 不改 `trace.py`
- 不改 host adapter / profile artifact lane
- 不改 storage backend lane
- 不通过恢复 Python route authority 来维持 parity 通过

### 交付物

- 更收敛的 route diagnostic contract
- 更窄的 Python verify / shadow / rollback participation boundary
- route-policy / parity regression tests

### 验收标准

- steady-state route result 与 route-policy 语义不再需要 Python route execution 才能成立
- verify / shadow / rollback 若继续保留，必须被明确定义为 diagnostic / compatibility lane
- Rust route policy 与 Python host projection 之间的双维护进一步减少
- parity 回归锁定的是 contract 与 evidence，不是 Python authority 回退

---

## 4. Desktop continuity artifact surface 再收口

### 当前状态

- `codex_desktop_host_adapter` 已经退到 compatibility-only。
- 默认 artifact emission 已不再把 alias 当作 canonical surface，但 continuity lane 里仍保留若干显式 artifact / inventory / retirement evidence。

### 目标

继续收紧 alias / continuity artifact surface，让 legacy continuity 证据只保留“证明可退场”的最小面，而不是长期持有过宽默认形态。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/host_adapters.py`
- `codex_agno_runtime/src/codex_agno_runtime/profile_artifacts.py`
- `codex_agno_runtime/src/codex_agno_runtime/compatibility.py`
- `docs/host_adapter_contracts.md`
- `docs/upgrade_compatibility_matrix.md`
- `tests/test_framework_profile_adapters.py`
- `tests/test_framework_contract_artifacts.py`

### 禁止越界

- 不改 `execution_kernel.py`
- 不改 `services.py`
- 不改 `runtime.py`
- 不改 storage / trace / observability lane
- 不让任何 continuity artifact 借机回到 canonical identity surface

### 交付物

- 更窄的 continuity-only artifact contract
- 更少的默认 alias / inventory 暴露面
- adapter / artifact regression tests

### 验收标准

- `codex_desktop_host_adapter` 不会重新回到 default artifact / canonical identity surface
- continuity artifact 只保留退场所必需的最小证据面
- 默认 consumer 不会再被动看到 alias inventory / retirement artifact
- tests 能证明收口后不会污染 canonical peer set 或 shared contract truth

---

## 5. Observability exporter / metrics concrete implementation

### 当前状态

- observability vocabulary 与 ownership 边界已冻结，且已声明 Rust contract-owned。
- 但 concrete exporter / metrics implementation 仍偏薄；当前更多是 vocabulary / contract readiness，而不是更完整的 exporter path。

### 目标

把 observability 从“contract 已冻结”推进到“exporter / metrics path 更 concrete”，同时保持 vocabulary、replay seam、compaction seam 不漂移。

### 独占写入范围

- `docs/runtime_observability_contract.md`
- 如确有必要，可新增 `codex_agno_runtime/src/codex_agno_runtime/observability.py`
- `tests/test_runtime_observability_contracts.py`
- 如确有必要，可新增 exporter / metrics 定向测试文件

### 禁止越界

- 不改 `trace.py`
- 不改 `runtime.py`
- 不改 `services.py`
- 不改 storage backend / host adapter lane
- 不重写 JSONL / OTel vocabulary，只在现有 contract 内落地 exporter / metrics path

### 交付物

- 更 concrete 的 exporter / metric implementation lane
- JSONL / OTel vocabulary alignment regression tests
- observability contract 对齐说明

### 验收标准

- JSONL 与 OTel vocabulary 仍使用同一 canonical contract
- exporter / metrics path 不再只是 ownership 声明，而有更明确的 concrete implementation evidence
- 任何新增 exporter / metric path 都不破坏 replay / compaction / resume 语义
- 不引入高基数、宿主私有、或未版本化的 observability 字段

---

## 不纳入本轮并行项的内容

以下内容**不纳入本轮**，因为它们不是当前剩余 Rust 化工作的正确下一 slice，或会重新混淆 authority 边界：

1. **把 memory extraction policy 搬进 Rust**
   - 当前只允许 Rust 接管 deterministic kernel，不接管 policy。
2. **把 compression wording / summarization policy 搬进 Rust**
   - 当前只允许 Rust 接管 deterministic pruning mechanics。
3. **把 Claude / Gemini / Codex 宿主私有 hook/config 语义下沉到 runtime kernel**
   - 宿主投影仍然只是 host-private contract surface，不是 framework truth。
4. **把 Python diagnostic lane 整体粗暴删除**
   - 如果要继续收口，只能把它们压成更窄 diagnostic / compatibility lane，不能直接丢失 parity / observability evidence。
5. **把全面 runtime replacement 当作本轮目标**
   - 本轮目标是把已经 contract-owned 的边界继续 concrete 化，而不是一次性替换整个 Python runtime。

---

## 建议执行顺序

这 5 项可以并行，但建议按下面方式组织：

### 第一优先级
- `1` SQLite backend steady-state 强化
- `2` Runtime event transport 外部 attach bridge 落地
- `3` Route diagnostic lane 继续 de-Pythonization

### 第二优先级
- `4` Desktop continuity artifact surface 再收口
- `5` Observability exporter / metrics concrete implementation

### 并行注意事项

- `1` 独占 backend/state/config lane；其他任务不得顺手改 backend capability surface。
- `2` 独占 runtime transport lane；`3` 不得为 route diagnostic 需要而顺手改 transport owner 文件。
- `4` 独占 alias / compatibility artifact lane；其他任务不得借 continuity artifact 改写 framework truth。
- `5` 只推进 observability exporter / metrics concrete path，不得反向侵入 route / storage / transport owner 文件。

---

## 本轮总体验收线

当且仅当以下条件全部满足，可认为“下一阶段剩余 Rust 化工作”完成：

1. `1`、`2`、`3`、`4`、`5` 全部完成，且都通过各自验收标准。
2. 默认 live/runtime/control-plane authority 仍保持 Rust-first，不回退到 Python-first。
3. concrete backend、external transport、route diagnostic lane、continuity artifact、observability exporter 都有对应回归测试锁定。
4. 本轮没有通过恢复 Python authority、扩大 host-private truth、或绕过 frozen contract 来换取通过率。
5. 高层总账、专项 contract 文档、当前代码、当前 checklist 之间的状态叙事继续保持一致。

## 当前完成统计模板

- 当前轮次总任务数：**5**
- 已完成：`5/5`
- 进行中：`0/5`
- 未开始：`0/5`

可按以下格式持续更新：

- [x] 1 SQLite backend steady-state 强化
- [x] 2 Runtime event transport 外部 attach bridge 落地
- [x] 3 Route diagnostic lane 继续 de-Pythonization
- [x] 4 Desktop continuity artifact surface 再收口
- [x] 5 Observability exporter / metrics concrete implementation
