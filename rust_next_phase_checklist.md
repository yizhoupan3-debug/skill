# Rust 化下一阶段执行清单

> 目标：在当前 phase 已收口的前提下，开启下一阶段 Rust 化工作，把剩余高价值 contract、bootstrap、observability、host consumer adoption 收敛成可并行推进的新一轮执行面。
>
> 约束：本清单是**下一阶段** checklist；默认 live/runtime/control-plane authority 继续保持 Rust-first，不回退到 Python-first，不把宿主私有语义反写进 framework truth。

## 当前起点

- 上一阶段已经完成 backend steady-state、process-external attach、route diagnostic 收窄、以及相邻 observability/health 的 targeted 验证。
- 当前最值钱的工作已经不再是继续局部修补旧 lane，而是把 Rust-first contract 扩展到 execution kernel、bootstrap、observability activation、以及 host consumer adoption。
- 本阶段按 **4 个并行 lane + 1 个集成 lane** 推进。

## 2026-04-22 进度同步

### 已推进到的状态

- **Lane 1 / Kernel Contract Canonicalization** 已完成当前 phase 收口：
  - `RouterService` 默认消费 typed Rust contract，而不是继续手拼原始 route JSON。
  - `RustRouteAdapter` 已暴露 `route_contract / route_policy_contract / route_report_contract / route_snapshot_contract`。
  - fixture/live parity 回归也已经优先走 typed contract；unknown skill 会 fail-closed，而不是让 Python 默默兜底。
  - `tests/test_routing_parity.py`、`tests/test_codex_agno_runtime_services.py`、`tests/test_execution_kernel_router_rs_contract.py` 已覆盖这条收口。
- **Lane 2 / Native Install / Bootstrap** 已完成当前 phase 收口：
  - `scripts/install_skills.sh` 现在按完整默认 contract 判定 Codex ready，而不是只看 config / skills link / bootstrap 文件是否存在。
  - bootstrap 校验会确认 payload 仍对齐当前 repo root 与 `skills/SKILL_ROUTING_RUNTIME.json` 这条 Rust-first runtime surface，不再把坏文件误判成 ready。
  - `tests/test_install_codex_native_integration.py` 已补齐 `all/status` shell 回归，证明新机器安装后默认入口直接落到当前 contract。
- **Lane 3 / Observability Activation** 已完成当前目标收口：
  - `codex_agno_runtime.observability` 已提供 exporter descriptor / metric record / dashboard schema / health snapshot。
  - `docs/runtime_observability_contract.md` 与 `tests/test_runtime_observability_contracts.py` 已把 vocabulary、exporter、metric、dashboard 对齐锁住。
  - `scripts/router-rs/src/main.rs` 的 observability exporter / dashboard / metric record contract 也已和 Python thin projection 对齐，并有 targeted `cargo test` 覆盖。
- **Lane 4 / Host Consumer Adoption** 已完成当前 phase 收口：
  - `tools/browser-mcp/src/runtime.ts` 已围绕 Rust attach descriptor / handoff / binding artifact 做 replay-capable attach 消费，不再只是旧三路径手搓逻辑。
  - 更广 host family adoption 也已核查并统一到同一条 Rust-first outward surface：默认 contract 输出只保留 default host peer set，`aionrs_companion_adapter` / `aionui_host_adapter` / `generic_host_adapter` 改为显式 fallback lane，`codex_desktop_host_adapter` 与 alias 产物继续只留在 continuity lane。
- **Lane 5 / Integrator / Regenerate** 已完成本轮集成收口：
  - framework/profile artifact emitter 已统一输出 `default/`、`fallback/`、`continuity/`、`rust/` 四类物理落点，并带 layout manifest。
  - `scripts/sync_skills.py`、`scripts/skill-compiler-rs`、`tests/test_framework_contract_artifacts.py` 已证明当前 generated artifacts / docs / registry contract 可以一起刷新并保持通过。

### 仍未收口的内容

- 当前这轮 checklist 的 1-5 已全部收口；后续只在出现新的 contract 变更时再开下一轮 safe slice。

---

## 本阶段并行任务总表

- 本轮可并行执行任务总数：**5 项**
- 并行原则：**前 4 项并行推进，最后 1 项只做集成，不提前掺杂实现细节**
- 完成定义：**前 4 项各自通过本 lane 验收后，第 5 项统一刷新 artifacts / docs / generated outputs / targeted verification**

| ID | 任务 | 主要目标 | 独占写入范围 |
|---|---|---|---|
| 1 | Kernel Contract Canonicalization | 把 execution kernel 的核心输出契约进一步收口到 Rust | `codex_agno_runtime/src/codex_agno_runtime/execution_kernel.py`, `codex_agno_runtime/src/codex_agno_runtime/execution_kernel_contracts.py`, `codex_agno_runtime/src/codex_agno_runtime/rust_router.py`, `scripts/router-rs/src/main.rs`, `tests/test_execution_kernel_router_rs_contract.py` |
| 2 | Native Install / Bootstrap | 让安装与初始化入口默认落在当前 Rust-first contract 上 | `scripts/install_codex_native_integration.py`, `scripts/install_skills.sh`, `tests/test_install_codex_native_integration.py` |
| 3 | Observability Activation | 把 observability 从 contract/health snapshot 推到更明确的 exporter / metric / dashboard 输出 | `codex_agno_runtime/src/codex_agno_runtime/observability.py`, `docs/runtime_observability_contract.md`, `tests/test_runtime_observability_contracts.py`, 必要时 `scripts/router-rs/src/main.rs` |
| 4 | Host Consumer Adoption | 让 host/runtime consumer 真正消费新的 Rust-first surface，尤其是 attach descriptor | `tools/browser-mcp/src/runtime.ts`，必要时相邻 host-facing bridge / adapter 文件 |
| 5 | Integrator / Regenerate | 只在前 4 项都稳定后，统一刷新生成物、文档、注册表和回归证据 | `skills/SKILL_*`, 相关 docs, health/routing/generated artifacts, targeted verification surfaces |

---

## 1. Kernel Contract Canonicalization

### 当前状态

- execution kernel 已经明显 Rust-first，但仍有部分 contract surface 由 Python 帮忙拼接、转述、或者做 naming bridge。
- 当前高价值目标不是重写 kernel，而是把 **kernel 输出语义的真源** 进一步交给 Rust。

### 目标

让 execution kernel 的关键 metadata / contract 字段以 Rust 为 canonical producer，Python 只做 thin projection / transport / compatibility bridge。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/execution_kernel.py`
- `codex_agno_runtime/src/codex_agno_runtime/execution_kernel_contracts.py`
- `codex_agno_runtime/src/codex_agno_runtime/rust_router.py`
- `scripts/router-rs/src/main.rs`
- `tests/test_execution_kernel_router_rs_contract.py`

### 禁止越界

- 不改 checkpoint/state backend lane
- 不改 host adapter continuity lane
- 不把 kernel canonicalization 变成 runtime rewrite
- 不通过 Python 重新接管 kernel live contract 来换测试通过

### 交付物

- 更收口的 Rust-owned execution kernel contract
- Python thin projection 语义边界更清楚
- kernel contract regression tests

### 验收标准

- runtime metadata 里的 kernel/live contract 字段由 Rust 产出为准
- Python 不再二次拼核心语义，只做 projection / compatibility bridge
- targeted kernel contract tests 通过

---

## 2. Native Install / Bootstrap

### 当前状态

- 安装与初始化入口已经收口到当前 Rust-first contract。
- `install_skills.sh` 的 shell status 也已经和默认 contract 对齐，不再把缺 plugin / marketplace / refresh / overlay / 有效 bootstrap 的安装误判为 ready。

### 目标

把 native install / bootstrap 入口和当前 Rust-first runtime contract 对齐，减少人工补配置或旧默认路径。

### 独占写入范围

- `scripts/install_codex_native_integration.py`
- `scripts/install_skills.sh`
- `tests/test_install_codex_native_integration.py`

### 禁止越界

- 不改 runtime kernel 主逻辑
- 不顺手改 generated skill registry
- 不把 bootstrap lane 变成 host-specific special case 泥球

### 交付物

- 更稳定的 bootstrap/install 默认行为
- install/bootstrap regression tests

### 验收标准

- 新机器 bootstrap 后默认落到当前 Rust-first contract
- 不需要额外手工切换旧 surface
- install tests 能证明默认入口已经对齐当前 contract

---

## 3. Observability Activation

### 当前状态

- observability contract、health snapshot、exporter descriptor、metric record、dashboard schema 已经形成一套可消费输出。
- 当前 lane 的实现目标已经收口；后续若继续推进，重点不再是补 observability vocabulary，而是服务于更大的 phase 集成或 host consumer 扩展。

### 目标

把 observability 从“有 vocabulary 和 schema”推进到“有可消费输出”，同时不漂移已有 vocabulary / replay / compaction contract。

### 独占写入范围

- `codex_agno_runtime/src/codex_agno_runtime/observability.py`
- `docs/runtime_observability_contract.md`
- `tests/test_runtime_observability_contracts.py`
- 如确有必要，可改 `scripts/router-rs/src/main.rs`

### 禁止越界

- 不改 trace vocabulary 主定义
- 不改 runtime.py 主调度语义
- 不引入高基数或宿主私有字段污染 shared contract

### 交付物

- 更 concrete 的 exporter descriptor / metric record / dashboard contract
- observability targeted tests
- docs alignment 说明

### 验收标准

- 不再只有 health snapshot，而是有明确 exporter payload / metric record / dashboard output 证据
- vocabulary 仍保持稳定
- observability tests 通过

---

## 4. Host Consumer Adoption

### 当前状态

- 上一阶段已经把 process-external attach bridge 做成更稳定的 artifact replay seam，并引入 attach descriptor。
- browser-mcp 已经消费这套 Rust attach / replay surface，其余 host-facing consumer 也已完成边界核查：默认 outward surface 只保留 canonical host peers，fallback / continuity consumer 改为显式 opt-in。

### 目标

让 host-facing consumer 真正吃上新的 Rust-first attach / replay / health surface，不再手搓旧的三路径拼接逻辑，同时把 fallback / continuity consumer 明确隔离到非默认 lane。

### 独占写入范围

- `tools/browser-mcp/src/runtime.ts`
- 如确有必要，可扩到邻近 host-facing bridge / adapter 文件

### 禁止越界

- 不回头改 runtime attach core 语义
- 不让 host consumer 自己重新发明第二套 attach contract
- 不把未实现的 live remote streaming 伪装成已支持

### 交付物

- host consumer adoption patch
- attach descriptor consumption path
- host-family default/fallback/continuity boundary
- host-facing targeted verification

### 验收标准

- consumer 直接使用 attach descriptor，而不是继续手拼 `binding/handoff/resume`
- host-facing调用链与当前 artifact replay contract 对齐
- default host peer set 不再混入 fallback / continuity consumer
- adoption regression 通过

---

## 5. Integrator / Regenerate

### 当前状态

- 本轮已经对当前稳定 surfaces 做完一轮 integrator/regenerate 收口：generated artifacts、docs、registry 和 targeted verification 已刷新并可落盘验证。
- 更大范围的 phase 切换仍然要等 lane 1 / 2 继续稳定后再做，但这不影响本轮 lane 5 的 bounded closeout。

### 目标

对当前已经稳定的 Rust-first surfaces 做统一的 docs / generated artifacts / registry / manifests / targeted verification 收口，并为后续更大 phase 切换保留一致的 layout / verification 基线。

### 独占写入范围

- `skills/SKILL_*`
- 相关 docs
- health / routing / generated artifacts
- 需要统一刷新的 targeted verification surfaces

### 禁止越界

- 不提前替前 4 个 lane 擦屁股
- 不在 lane 未稳定时刷新全局生成物
- 不把 integration lane 变成偷渡实现 lane

### 交付物

- 刷新的 generated artifacts / manifests / docs
- 总体验证结果
- 最终 continuity handoff 材料

### 验收标准

- 当前要集成的稳定 surfaces 已完成 generated outputs 与 docs 同步刷新
- skill routing / framework profile / Rust artifact emission 的 targeted verification 通过
- layout manifest 与默认 `default/ fallback/ continuity/ rust` 落点一致，可作为下一次更大 phase 切换的集成基线

---

## 推荐执行顺序

1. 先并行打开 `1 / 2 / 3 / 4`
2. 每条 lane 只在自己写入范围内推进，不跨 lane 乱改公共 owner 文件
3. 待前 4 条都稳定后，再单独执行 `5`
4. 集成完成后统一做最终验证与 continuity 收口
