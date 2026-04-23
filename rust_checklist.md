# Rust 化执行清单

> 目标：在不回退到 Python-first、也不把宿主私有语义反写进 framework truth 的前提下，把 framework/runtime/control-plane 的剩余 Python 主面继续压缩到**接近只剩薄包装与少量宿主私有脚本**。
>
> 约束：本清单只记录**下一轮**任务；所有任务必须能**互不打扰地并行推进**，因此每项都给出**独占写入范围**、**禁止越界范围**、**交付物**与**验收标准**。

## 当前状态快照

- route / kernel-metadata / attach / framework-profile 主链路已经是 Rust-first，并且最近一次 standalone re-verify 已重新通过。
- 当前更像“残余 Python 主面”的位置，主要不在 route authority 本身，而在：
  - Python router facade / projection
  - runtime health / control-plane 聚合层
  - runtime registry / host-entrypoint materialization
  - hook bridge / session artifact / memory-support 这类脚本型 glue
- 当前代码里还能直接看到这些残余面：
  - `framework_runtime/src/framework_runtime/router.py`
  - `framework_runtime/src/framework_runtime/runtime.py`
  - `framework_runtime/src/framework_runtime/services.py`
  - `framework_runtime/src/framework_runtime/runtime_registry.py`
  - `scripts/framework_hook_bridge.py`
  - `scripts/write_session_artifacts.py`
  - `scripts/memory_support.py`
  - `scripts/materialize_cli_host_entrypoints.py`
  - `scripts/install_codex_native_integration.py`
- 本轮目标不是“一次性删光所有 Python 文件”，而是把**framework/runtime/control-plane 的事实主权**继续推给 Rust，让 Python 不再持有关键路径事实来源。

---

## 本轮并行任务总表

- 本轮可并行执行任务总数：**6 项**
- 并行原则：**每项只改自己负责的文件族，不跨 lane 改公共 owner 文件**
- 完成定义：**6 项都满足各自验收标准，且默认 live/runtime/control-plane authority 仍保持 Rust-first；完成后 Python 只剩薄包装、宿主私有 glue、或明确保留的 policy 面**

| ID | 任务 | 主要目标 | 独占写入范围 |
|---|---|---|---|
| 1 | Python router facade 再收口 | 把 `router.py` 从实际路由入口压到更薄的 Rust projection / compatibility shell | `framework_runtime/src/framework_runtime/router.py`, `framework_runtime/src/framework_runtime/rust_router.py`, route/parity 相关测试 |
| 2 | Runtime control-plane 聚合去 Python 主面 | 把 `runtime.py` / `services.py` 中 control-plane、health、service-descriptor 聚合继续下沉到 Rust | `framework_runtime/src/framework_runtime/runtime.py`, `framework_runtime/src/framework_runtime/services.py`, runtime/trace/service 相关测试 |
| 3 | Runtime registry / host-entrypoint materialization Rust-owned | 把 runtime registry 读取、framework alias entrypoint、materialization 真源继续推向 Rust，减少 Python 物化器的事实权 | `framework_runtime/src/framework_runtime/runtime_registry.py`, `scripts/materialize_cli_host_entrypoints.py`, `scripts/install_codex_native_integration.py`, `scripts/router-rs/src/main.rs`, `scripts/host-integration-rs/src/main.rs`, registry/materialization 相关测试 |
| 4 | Hook bridge / artifact writer 脚本 Rust-first 化 | 把 hook dispatch、session artifact 写入、部分 deterministic audit/bridge 逻辑从 Python 脚本切到 Rust/更薄包装 | `scripts/framework_hook_bridge.py`, `scripts/write_session_artifacts.py`, `scripts/router-rs/src/main.rs`, hook/artifact 相关测试 |
| 5 | Memory-support / continuity glue 再收口 | 把 deterministic continuity / memory-support 辅助面继续压薄，减少 Python 对恢复锚点和 shared memory 结构的手工拼装 | `scripts/memory_support.py`, `scripts/install_codex_native_integration.py`, continuity/memory 相关测试 |
| 6 | Python 残余面最终清点与退场 | 在前五项完成后做一次系统性退场：删掉不再需要的 Python compatibility shim，只保留必要薄包装与宿主私有脚本 | 仅限本轮新增或确认可删除的 Python shim 文件、相关 docs、退场测试 |

---

## 1. Python router facade 再收口

### 当前状态

- `framework_runtime/src/framework_runtime/rust_router.py` 已经持有 Rust adapter、route/search/route-policy/route-report 等主要 contract。
- `framework_runtime/src/framework_runtime/router.py` 仍保留 `SkillRouter` 这类 Python facade，虽然 authority 已不在 Python，但 projection 面还偏厚。

### 目标

把 Python router facade 收口成更薄的 compatibility shell；steady-state route/search/report/policy 语义由 Rust contract lane 直接成立，而不是靠 Python facade 继续充当事实入口。

### 独占写入范围

- `framework_runtime/src/framework_runtime/router.py`
- `framework_runtime/src/framework_runtime/rust_router.py`
- route / parity / route-policy 相关测试文件

### 禁止越界

- 不改 `runtime.py`
- 不改 `services.py`
- 不改 host adapter / registry / hook bridge lane
- 不通过恢复 Python route authority 来保持兼容

### 交付物

- 更薄的 Python router facade
- 更清晰的 Rust-owned route/search/report/policy entry surface
- route/parity regression tests

### 验收标准

- steady-state route/search 不再需要 Python facade 才能表达主 contract
- `router.py` 若仍保留，只是 compatibility shell，不再持有额外决策语义
- parity 锁定的是 Rust contract 与证据，不是 Python authority 回退

---

## 2. Runtime control-plane 聚合去 Python 主面

### 当前状态

- `framework_runtime/src/framework_runtime/runtime.py` 仍在做 runtime-level control-plane descriptor 分发与 `health()` 聚合。
- `framework_runtime/src/framework_runtime/services.py` 仍有大量 `_runtime_control_plane_*` helper、`python_runtime_role` / `remaining_python_role` 拼装，以及多处 service health 聚合。

### 目标

继续把 runtime health、service descriptor、control-plane contract snapshot 的事实来源下沉到 Rust，让 Python 不再是这些观测面的主聚合者。

### 独占写入范围

- `framework_runtime/src/framework_runtime/runtime.py`
- `framework_runtime/src/framework_runtime/services.py`
- `framework_runtime/src/framework_runtime/rust_router.py`（仅限 control-plane descriptor 接口需要）
- `scripts/router-rs/src/main.rs`（仅限 control-plane payload / health payload 需要）
- `tests/test_framework_runtime_runtime.py`
- `tests/test_framework_runtime_trace.py`
- `tests/test_framework_runtime_services.py`

### 禁止越界

- 不改 `router.py`
- 不改 registry / materialization lane
- 不改 hook bridge / memory-support lane
- 不把 host-private observability / hook 语义混入 runtime truth

### 交付物

- 更 Rust-owned 的 control-plane / health payload
- 更薄的 Python runtime/service aggregation
- runtime/trace/service regression tests

### 验收标准

- runtime `health()` 和主要 service `health()` 的关键字段能直接落在 Rust-owned payload 上
- `python_runtime_role` / `remaining_python_role` 若继续存在，只能是显式 residual marker，不是逻辑必需字段
- Python 不再手工拼接多份重复 control-plane truth

---

## 3. Runtime registry / host-entrypoint materialization Rust-owned

### 当前状态

- `framework_runtime/src/framework_runtime/runtime_registry.py` 仍是 Python loader，负责 runtime registry 读取、fallback、alias 读取、plugin/defaults 展开。
- `scripts/materialize_cli_host_entrypoints.py` 和 `scripts/install_codex_native_integration.py` 仍承担大量 host-entrypoint 物化与安装 glue。

### 目标

把 runtime registry、framework native alias、host-entrypoint materialization 的真源继续推向 Rust，让 Python 只保留薄 CLI/文件包装。

### 独占写入范围

- `framework_runtime/src/framework_runtime/runtime_registry.py`
- `scripts/materialize_cli_host_entrypoints.py`
- `scripts/install_codex_native_integration.py`
- `scripts/router-rs/src/main.rs`
- `scripts/host-integration-rs/src/main.rs`
- `tests/test_cli_host_entrypoints.py`
- `tests/test_install_codex_native_integration.py`
- `tests/test_runtime_registry.py`

### 禁止越界

- 不改 `runtime.py`
- 不改 `services.py`
- 不改 hook bridge / artifact writer lane
- 不把宿主私有配置语义错误抬升为 framework truth

### 交付物

- 更 Rust-owned 的 registry/materialization contract
- 更薄的 Python materializer / installer
- registry/materialization regression tests

### 验收标准

- framework alias entrypoint、shared project MCP、workspace bootstrap defaults 等 registry 面不再依赖 Python 作为唯一解释器
- Python materialization 脚本若保留，主要只做文件落地和参数转发
- registry fallback / schema guard 仍 fail-closed

---

## 4. Hook bridge / artifact writer 脚本 Rust-first 化

### 当前状态

- `scripts/framework_hook_bridge.py` 仍负责 hook payload 过滤、shared hook 调度。
- `scripts/write_session_artifacts.py` 仍持有 session artifact 写入入口。
- 这些脚本已经较薄，但仍是关键执行面，不只是无关小工具。

### 目标

把 deterministic hook dispatch、artifact write contract、可前移的 audit/bridge 逻辑继续推进到 Rust，Python 只剩最薄的 CLI wrapper 或宿主私有 glue。

### 独占写入范围

- `scripts/framework_hook_bridge.py`
- `scripts/write_session_artifacts.py`
- `scripts/router-rs/src/main.rs`
- `tests/test_framework_hook_bridge.py`
- `tests/test_write_session_artifacts.py`

### 禁止越界

- 不改 runtime registry / materialization lane
- 不改 `runtime.py` / `services.py`
- 不把 hook 私有行为写回 framework shared truth

### 交付物

- 更 Rust-owned 的 hook bridge / artifact writer command surface
- 更薄的 Python wrapper
- hook/artifact regression tests

### 验收标准

- deterministic hook dispatch / artifact contract 不再以 Python 脚本为唯一实现真源
- Python wrapper 若仍存在，不含复杂状态机或隐式 contract translation
- hook payload 过滤、artifact 写入 schema、失败语义仍可验证且 fail-closed

---

## 5. Memory-support / continuity glue 再收口

### 当前状态

- `scripts/memory_support.py` 仍承担 shared memory / continuity 相关辅助逻辑。
- 这部分不是要把 memory policy 搬进 Rust，而是把 deterministic 的恢复锚点、结构拼装、安装 glue 压薄。

### 目标

继续压缩 deterministic continuity/memory-support glue，让 Python 不再手工持有恢复锚点结构与安装逻辑的主面。

### 独占写入范围

- `scripts/memory_support.py`
- `scripts/install_codex_native_integration.py`（仅限 memory/continuity glue）
- `tests/test_memory_support.py`
- 必要时新增 continuity / recovery 定向测试

### 禁止越界

- 不改 memory extraction policy
- 不改 summarization/compression policy
- 不改 `runtime.py` / `services.py`
- 不改 route / registry / hook bridge lane

### 交付物

- 更薄的 memory-support / continuity glue
- recovery / continuity regression tests
- 明确区分 deterministic structure 与 policy surface

### 验收标准

- shared memory / continuity 的 deterministic structure 不再主要靠 Python 手工拼装
- policy 仍明确留在非 Rust policy 面，不会混淆边界
- 恢复锚点读取 / 结构校验 / 安装 glue 继续 fail-closed

---

## 6. Python 残余面最终清点与退场

### 当前状态

- 完成前五项后，仓库里应只剩下少量薄 Python 包装、宿主私有 glue、以及明确不属于 Rust 接管范围的 policy 面。
- 但如果不做一次系统性清点，容易留下“实际上已无必要”的 compatibility shim。

### 目标

做一次最终退场，把已失去职责的 Python shim 明确删除或降级成最小 wrapper，并把“不该 Rust 化的残余 Python”与“还没收口完的技术债”分开。

### 独占写入范围

- 仅限本轮前五项收口后确认可退场的 Python shim 文件
- 对应 docs / tests / manifest 更新

### 禁止越界

- 不新增新的 Python compatibility layer
- 不借“最终清点”回头改前五项 owner 文件的核心逻辑
- 不把 host-private 语义强行塞进 Rust，只为了让 Python 文件数看起来更少

### 交付物

- Python 残余面清单
- 可删除 shim 的删除提交
- 明确保留面的边界说明
- 最终退场 regression tests / inventory

### 验收标准

- framework/runtime/control-plane 主面不再由 Python 持有事实权
- Python 剩余部分能被清晰分成：
  1. 宿主私有 glue
  2. 明确保留的 policy 面
  3. 极薄 CLI/file wrapper
- 不存在“名字叫 wrapper，实际上还在做主逻辑”的残余 Python 文件

---

## 不纳入本轮并行项的内容

以下内容**不纳入本轮**，因为它们不是当前剩余 Rust 化工作的正确下一 slice，或会重新混淆 authority 边界：

1. **把 memory extraction / summarization policy 搬进 Rust**
   - 当前只允许 Rust 接 deterministic structure / kernel，不接 policy。
2. **把 Claude / Gemini / Codex 宿主私有 hook/config 语义下沉到 framework truth**
   - 宿主投影仍然只是 host-private contract surface。
3. **为了删 Python 而把 host-private glue 粗暴并入 Rust 主 contract**
   - 目标是减少 Python 主面，不是污染 Rust truth。
4. **恢复 Python authority 以换取更容易通过测试**
   - parity / regression 必须继续锁定 Rust-first。
5. **一次性重写整个仓库到“零 Python”**
   - 本轮目标是把关键路径清干净，不是把所有辅助脚本全部灭绝。

---

## 建议执行顺序

这 6 项可以并行，但建议按下面方式组织：

### 第一优先级
- `1` Python router facade 再收口
- `2` Runtime control-plane 聚合去 Python 主面
- `3` Runtime registry / host-entrypoint materialization Rust-owned

### 第二优先级
- `4` Hook bridge / artifact writer 脚本 Rust-first 化
- `5` Memory-support / continuity glue 再收口

### 最后一刀
- `6` Python 残余面最终清点与退场

### 并行注意事项

- `1` 不得顺手改 `runtime.py` / `services.py`。
- `2` 独占 control-plane / health 聚合 lane；`3/4/5` 不得借机改 runtime 聚合 owner 文件。
- `3` 独占 registry/materialization lane；其他任务不得改 alias/entrypoint 真源。
- `4` 只动 hook bridge / artifact writer，不得借机改 shared contract truth。
- `5` 只收口 deterministic continuity/memory glue，不碰 policy。
- `6` 必须在前五项基本落稳后再做。

---

## 本轮总体验收线

当且仅当以下条件全部满足，可认为“完成这一轮后 framework/runtime/control-plane 基本已无 Python 主面”：

1. `1`、`2`、`3`、`4`、`5`、`6` 全部完成，且都通过各自验收标准。
2. 默认 live/runtime/control-plane authority 仍保持 Rust-first，不回退到 Python-first。
3. router facade、control-plane health、runtime registry、hook bridge、artifact writer、memory/continuity glue 都有对应回归测试锁定。
4. 本轮没有通过恢复 Python authority、扩大 host-private truth、或把 policy 硬塞进 Rust 来换取“Python 文件更少”。
5. 完成后 Python 只剩：宿主私有 glue、明确保留的 policy 面、极薄 CLI/file wrapper。
6. 若还存在“实质主逻辑仍在 Python”的文件，必须在最终清点里被点名，而不能被算作已 Rust 化完成。

## 当前完成统计模板

- 当前轮次总任务数：**6**
- 已完成：`0/6`
- 进行中：`0/6`
- 未开始：`6/6`

可按以下格式持续更新：

- [ ] 1 Python router facade 再收口
- [ ] 2 Runtime control-plane 聚合去 Python 主面
- [ ] 3 Runtime registry / host-entrypoint materialization Rust-owned
- [ ] 4 Hook bridge / artifact writer 脚本 Rust-first 化
- [ ] 5 Memory-support / continuity glue 再收口
- [ ] 6 Python 残余面最终清点与退场
