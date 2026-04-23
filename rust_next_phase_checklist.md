# Rust 化新轮次执行清单

> 目标：上一轮 lane 1-5 已经收口；这一轮不再补旧账，而是把剩余高价值的 Rust-first 收口面拆成可以并行推进的新 lane。
>
> 约束：默认 live/runtime/control-plane authority 继续保持 Rust-first；不回退到 Python-first，不把宿主私有语义反写进 framework truth，不把集成 lane 混成偷渡实现 lane。

## 当前起点

- 上一轮已经完成 execution kernel 基本收口、native install/bootstrap 对齐、observability activation、host consumer adoption、以及 integrator/regenerate 的阶段性闭环。
- 当前还能继续推进的，不是旧 lane 的扫尾，而是下一批还留着 Python compatibility 影子的 surface。
- 本轮按 **4 个并行 lane + 1 个集成 lane** 推进。

## 上一轮关闭结论

- `rust_checklist.md` 对应的 lane 1-5 已经完成，不再作为活跃 checklist 继续编号。
- `rust_next_phase_checklist.md` 上一版也已完成当前 round closeout；本文件现在接管为**下一轮主清单**。
- 本轮如果没有新的 contract 变更，就不重跑全量 lane 5，只做定点验证。

---

## 本轮并行任务总表

- 本轮可并行执行任务总数：**5 项**
- 并行原则：**前 4 项并行推进，最后 1 项只做集成与验证**
- 完成定义：**前 4 项各自稳定后，再统一刷新需要刷新的 docs / generated artifacts / targeted verification**

| ID | 任务 | 主要目标 | 独占写入范围 |
|---|---|---|---|
| 1 | Route Consumer Typed-First Cutover | 让 route CLI / adjacent helpers 不再做特权 raw-JSON consumer，而是稳定消费 Rust-owned typed contract | `scripts/route.py`, `codex_agno_runtime/src/codex_agno_runtime/rust_router.py`, `codex_agno_runtime/src/codex_agno_runtime/schemas.py`, 相关 route/parity 测试 |
| 2 | Execution Kernel Metadata Canonicalization Phase B | 把 execution-kernel metadata / naming bridge 的真源继续交给 Rust，压缩 Python compatibility projection | `codex_agno_runtime/src/codex_agno_runtime/execution_kernel.py`, `codex_agno_runtime/src/codex_agno_runtime/execution_kernel_contracts.py`, `codex_agno_runtime/src/codex_agno_runtime/services.py`, `scripts/router-rs/src/main.rs`, 相关 execution-kernel 测试 |
| 3 | Workspace Bootstrap / Shared Contract Parity | 把 `workspace_bootstrap` 与 host-adapter shared contract 的 Python / Rust 双实现继续锁成一条 truth | `codex_agno_runtime/src/codex_agno_runtime/framework_profile.py`, `codex_agno_runtime/src/codex_agno_runtime/host_adapters.py`, `scripts/router-rs/src/framework_profile.rs`, `docs/framework_profile_contract.md`, 相关 framework-profile / contract-artifact 测试 |
| 4 | Process-External Attach Surface Hardening | 把 attach descriptor / replay / cleanup 的 process-external surface 再收口一层，避免 consumer 各自补第二套协议 | `codex_agno_runtime/src/codex_agno_runtime/event_transport.py`, `codex_agno_runtime/src/codex_agno_runtime/runtime.py`, 必要时 `tools/browser-mcp/src/runtime.ts`, 相关 runtime/browser-mcp 测试 |
| 5 | Integrator / Regenerate / Verify | 只在前 4 项稳定后，统一刷新真正需要变更的 docs / generated outputs / evidence | `skills/SKILL_*`, 相关 docs, generated artifacts, targeted verification surfaces |

---

## 1. Route Consumer Typed-First Cutover

### 当前状态

- `scripts/route.py` 已经是 Rust transport shim，不再是第二套 route authority。
- 但 route-side compatibility helpers、非 runtime CLI surface、以及一些测试辅助路径，仍有 raw JSON / ad hoc hydration 的影子。

### 目标

让 route CLI 和相邻 helper 默认围绕 typed route contract 工作，而不是继续把 Rust payload 当成“先吐 JSON，再由 Python 自己猜语义”。

### 独占写入范围

- `scripts/route.py`
- `codex_agno_runtime/src/codex_agno_runtime/rust_router.py`
- `codex_agno_runtime/src/codex_agno_runtime/schemas.py`
- route/parity 相关测试文件

### 禁止越界

- 不回头把 route authority 重新搬回 Python
- 不改 execution-kernel 主逻辑
- 不把 fixture convenience path 做成新的 live truth

### 交付物

- typed-first route helper surface
- 更薄的 JSON hydration / compatibility bridge
- route consumer regression tests

### 验收标准

- route CLI / helper 默认消费 typed contract
- raw JSON 只保留 transport 边界职责
- route/parity targeted tests 通过

### 任务覆盖

1. 盘点 `scripts/route.py` 里仍在直接信任 payload shape 的入口。
2. 把可结构化的 route decision / match result / diff report 收口到 shared schema。
3. 给 fixture/live parity 两条路补同一套 typed-first regression。

---

## 2. Execution Kernel Metadata Canonicalization Phase B

### 当前状态

- Rust 已经拥有 live execution 的主通路，`execution_kernel_live_response_serialization_contract` 也已冻结了当前 response shape。
- 但 execution-kernel metadata / naming bridge 仍有一部分由 Python projection 帮忙补齐或转述。

### 目标

把 execution-kernel metadata 的 canonical producer 继续收口到 Rust，让 Python 更接近纯 projection / transport / compatibility adapter。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/execution_kernel.py`
- `codex_agno_runtime/src/codex_agno_runtime/execution_kernel_contracts.py`
- `codex_agno_runtime/src/codex_agno_runtime/services.py`
- `scripts/router-rs/src/main.rs`
- execution-kernel / runtime-service 相关测试

### 禁止越界

- 不重开 Python live fallback
- 不把 metadata canonicalization 扩成 kernel rewrite
- 不改 background queue / checkpoint backend 的主职责

### 交付物

- 更收口的 Rust-owned kernel metadata contract
- 更薄的 Python metadata projection
- execution-kernel targeted regression tests

### 验收标准

- runtime/service health 里的 kernel metadata 以 Rust 输出为准
- Python 不再补写核心 steady-state 语义
- execution-kernel targeted tests 通过

### 任务覆盖

1. 盘点 `execution_kernel_contracts.py` 与 `services.py` 里仍由 Python 兜写的 kernel metadata。
2. 把 naming bridge 能 Rust-owned 的部分继续前移到 `router-rs`。
3. 验证 live primary / dry-run / retired compatibility metadata 三条 shape 仍然稳定。

---

## 3. Workspace Bootstrap / Shared Contract Parity

### 当前状态

- `workspace_bootstrap` 已经是 shared contract 的一部分，Python / Rust 两边也都能编译出对应 surface。
- 但这块仍是典型“双实现并存”的区域，最容易在 host-adapter / framework-profile 演进时出现漂移。

### 目标

锁住 `workspace_bootstrap`、bridge contract、以及 host-adapter shared-contract projection 的 Rust/Python parity，避免后续 host family 演进时再次出现双真源。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/framework_profile.py`
- `codex_agno_runtime/src/codex_agno_runtime/host_adapters.py`
- `scripts/router-rs/src/framework_profile.rs`
- `docs/framework_profile_contract.md`
- framework-profile / framework-contract-artifact 相关测试

### 禁止越界

- 不把 host-specific metadata 混入 framework truth
- 不把 default host peer set 再次扩大
- 不顺手改 installer lane 的默认行为

### 交付物

- 更稳定的 workspace bootstrap parity contract
- docs 对齐说明
- framework-profile targeted tests

### 验收标准

- Python / Rust 产出的 `workspace_bootstrap` 语义一致
- shared contract 不引入新的 host-private 漂移字段
- framework-profile / contract-artifact tests 通过

### 任务覆盖

1. 对齐 Python / Rust 的 `workspace_bootstrap.bridges` 编译逻辑。
2. 明确哪些字段属于 framework truth，哪些只能停留在 host projection。
3. 给 host-adapter parity / contract artifact 增加回归。

---

## 4. Process-External Attach Surface Hardening

### 当前状态

- browser-mcp 已经吃上 attach descriptor / handoff / binding artifact 这套 Rust-first replay surface。
- 但 process-external attach 仍然容易在 cleanup、resume、以及其他 consumer 演进时出现“各自再发明半套协议”的风险。

### 目标

把 attach descriptor / replay / cleanup 的 process-external surface 再硬化一轮，让 runtime、consumer、以及恢复路径共享同一条 descriptor contract。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/event_transport.py`
- `codex_agno_runtime/src/codex_agno_runtime/runtime.py`
- 如确有必要，可改 `tools/browser-mcp/src/runtime.ts`
- runtime/browser-mcp 相关测试

### 禁止越界

- 不回头改 attach core 的 steady-state 语义
- 不把 remote streaming 冒充成已完成能力
- 不让 consumer 自己扩写第二套 descriptor schema

### 交付物

- 更稳定的 attach descriptor / replay / cleanup contract
- runtime/browser consumer targeted tests
- process-external recovery alignment

### 验收标准

- attach descriptor schema 仍是单一真源
- replay / cleanup / resume 语义在 runtime 与 consumer 之间一致
- runtime/browser-mcp targeted tests 通过

### 任务覆盖

1. 盘点 event transport 与 runtime attach 入口的 descriptor 解析边界。
2. 对齐 cleanup / replay / resume 的 contract vocabulary。
3. 给 process-external recovery 路径补 fail-closed regression。

---

## 5. Integrator / Regenerate / Verify

### 当前状态

- 上一轮 integrator/regenerate 已经证明 generated artifacts / docs / registry 可以一起刷新。
- 本轮不默认重跑全量生成；只有当前 1-4 的稳定 lane 真改到了 contract / generated outputs，才做对应刷新。

### 目标

对本轮真正变动到的 Rust-first surface 做统一集成、定点刷新与最终验证，而不是机械地全量重建所有东西。

### 独占写入范围

- `skills/SKILL_*`
- 相关 docs
- generated artifacts
- targeted verification surfaces

### 禁止越界

- 不提前替前 4 个 lane 擦屁股
- 不为了“看起来完整”而刷新无关 generated outputs
- 不覆盖其他 active task 的 continuity 镜像

### 交付物

- 只针对真实变更面的 docs / generated artifacts / evidence 刷新
- 最终验证结果
- handoff-ready closeout 材料

### 验收标准

- 只刷新本轮真的改到的 surface
- bootstrap / framework profile / route / attach 的 targeted verification 通过
- 如需刷新 generated outputs，刷新后 docs 与测试口径一致

### 任务覆盖

1. 汇总前 4 条 lane 的真实 contract 变化。
2. 判断哪些变化需要 `sync_skills` / docs / generated artifact refresh。
3. 做最终 verification，并把本轮 closeout 口径固定下来。

### lane5 小任务 1：真实 contract 变化与最小刷新清单（基于 2026-04-23 live diff）

#### 前 4 条 lane 的真实 contract 变化

- `lane1 / Route Consumer Typed-First Cutover`
  route search 的 JSON 输出不再只是裸 row 列表，而是 typed envelope：显式带上
  `search_schema_version`、`authority`、`query`、`matches`，并保留 `rows` 作为
  transport alias；routing eval 输入/输出也开始走 typed schema，而不是继续吃任意
  JSON shape；diagnostic report 新增结构化 `route_diff` 承载 contract mismatch。
- `lane2 / Execution Kernel Metadata Canonicalization Phase B`
  steady-state execution metadata 现在只把 Rust canonical 字段当真源；旧的
  live-fallback / compatibility-fallback 字段不再属于 steady-state contract，
  若混入 live payload 必须直接拒绝；retired response shape 被显式收口成单独 shape，
  不再靠 Python compatibility 痕迹偷带语义。
- `lane3 / Workspace Bootstrap / Shared Contract Parity`
  host adapter 现在显式区分 framework truth 与 host-private override：任何 host 私有
  字段都必须通过 `host_private` 显式 opt-in；CLI-family / desktop adapter 产物新增
  first-class `bridge_contract` 与 `source_contract`，把 bridge 来源与 adapter 别名关系
  写成 contract；native install / host integration 也开始从
  `configs/framework/RUNTIME_REGISTRY.json` 读取 plugin 与 skill bridge 默认值，而不是
  继续写死路径。
- `lane4 / Process-External Attach Surface Hardening`
  process-external attach descriptor 现在有了 Python 侧 typed schema 校验；browser-mcp
  对 attach descriptor、binding artifact、handoff、resume manifest 的解析入口被收口到
  同一套 hydrate/fallback 路径，避免 consumer 再各自补第二套 descriptor 语义。

#### 最小化 docs / generated artifacts 刷新清单

- 现在必须刷新的 docs：
  `docs/framework_profile_contract.md`。
- 现在必须刷新的 generated artifacts：
  framework-profile / host-adapter contract artifacts，重点是所有会落出
  `bridge_contract`、`source_contract`、compatibility alias mirror、shared contract
  projection report 的输出面；native integration / host integration 相关派生产物也要跟着
  runtime registry 的 plugin / skill bridge 默认值一起重投影。
- 现在明确不需要刷新的面：
  `skills/SKILL_*`、`skills/SKILL_ROUTING_*`、`sync_skills` 全部先不动，因为本轮没有改
  skill catalog 或 routing source；`docs/host_adapter_contracts.md`、
  `docs/rust_contracts.md`、`docs/upgrade_compatibility_matrix.md` 也先不机械重写，只有
  regenerate 后出现文档口径缺口时再补。
- lane5 集成时应配套跑的 targeted verification：
  `tests/test_routing_parity.py`、`tests/test_framework_runtime_rust_projection.py`、
  `tests/test_execution_kernel_router_rs_contract.py`、
  `tests/test_framework_runtime_services.py`、
  `tests/test_framework_profile_adapters.py`、
  `tests/test_framework_contract_artifacts.py`、
  `tests/test_install_codex_native_integration.py`、
  `tests/test_runtime_registry.py`。

---

## 推荐执行顺序

1. 并行打开 `1 / 2 / 3 / 4`
2. 每条 lane 只在自己的独占写入范围内推进
3. 任一 lane 先稳定，就先做该 lane 的 targeted verification，不抢 integrator
4. 待前 4 条达到可集成状态后，再单独执行 `5`

## 停止条件

- 如果某条 lane 想通过“把语义重新搬回 Python”来换测试通过，立即停止，视为偏航。
- 如果某条 lane 需要改动别的 lane 的 owner 文件，先拆任务，不直接越界硬改。
- 如果本轮没有新的 shared contract 变化，就不要为了仪式感重跑全量 regenerate。
