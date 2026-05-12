# Harness 提升建议（可落地路线图）— Backlog

本文档将「Harness 提升建议（可落地路线图）」中的表格项展开为 **P0 / P1 / P2** 可执行 backlog；真源仍以 [harness_architecture.md](../harness_architecture.md)、[AGENTS.md](../../AGENTS.md)、[host_adapter_contract.md](../host_adapter_contract.md) 与 `router-rs` 实现为准。

---

## Failure taxonomy 与 findings 映射

机器可读分类见 [`configs/framework/HARNESS_FAILURE_TAXONOMY.json`](../../configs/framework/HARNESS_FAILURE_TAXONOMY.json)；behavioral eval 用例见 [`configs/framework/HARNESS_BEHAVIORAL_EVAL_CASES.json`](../../configs/framework/HARNESS_BEHAVIORAL_EVAL_CASES.json)。分类只用于诊断与评估，不替代 `EVIDENCE_INDEX`、closeout 或路由真源。

| failure_class | 典型来源 | 对应吸收项 |
|---|---|---|
| `route_miss` / `owner_drift` | 路由 owner/gate/overlay 偏移 | routing eval + skill contract lint |
| `context_rot` | SessionStart、工具输出、账本整文件进入上下文 | token_efficiency + success-silent 输出 |
| `tool_contract_bad` | skill/tool 参数、错误、返回值不清晰 | skill/tool contract lint |
| `verification_missing` | 完成/通过声明缺 verifier 或 evidence row | closeout_integrity |
| `source_stale` | 易过期外部来源未验证 | source gate / external research trace |
| `side_effect_risk` | 外部副作用、迁移、审批边界不清 | tool policy / step side_effects |
| `subagent_misuse` | lane 无隔离、无 digest、无验证 | lane_contract integrity |
| `trace_gap` | 无法从 `TRACE_EVENTS` + evidence 复盘 | trajectory_health |
| `step_recovery_gap` | 长任务无 step 级恢复点 | `STEP_LEDGER.jsonl` |

新增执行面：`router-rs eval harness-contract` 输出 taxonomy/eval 合约；`router-rs eval skill-contract-lint` 输出共享 `findings / execution_items / verification_results`；`router-rs framework step-ledger` 追加或汇总 task-scoped step ledger。

---

## P0（立刻可验收、阻断「无证据完成」）

### P0-1：记账型证据（L1→L2）— PostTool / `EVIDENCE_INDEX` 契约对齐

- **做什么**：把「验证类命令被采样写入 `EVIDENCE_INDEX`」的路径与 **显式 append**（长尾命令）写进可测契约；避免 L5 技能与 L3 启发式各自发明第二套格式。上层原则见 [harness_architecture.md §3.1](../harness_architecture.md#31-证据流executable--audit)。
- **Done when**：`EVIDENCE_INDEX` 追加规则与 PostTool 启发式在文档与单测中有 **成对样例**（至少 1 条「命中启发式」+ 1 条「需显式 append」）；`router-rs` 中相关模块变更处有回归测试或快照断言。
- **Verify**：`cargo test --manifest-path scripts/router-rs/Cargo.toml`（若仅改文档则 `rg -n 'EVIDENCE_INDEX|hook-evidence-append' docs/harness_architecture.md scripts/router-rs/src` 作静态对照）。

### P0-2：硬门禁 — Closeout 与 CI 分层语义单一真源

- **做什么**：统一 **软/硬** closeout 分层（本地未设变量 vs CI / 显式 `ROUTER_RS_CLOSEOUT_ENFORCEMENT`）的叙述与实现，避免 `AGENTS.md` 与代码注释漂移。实现真源：[closeout_enforcement.rs](../../scripts/router-rs/src/closeout_enforcement.rs)；策略入口：[AGENTS.md](../../AGENTS.md)（Closeout、`ROUTER_RS_CLOSEOUT_ENFORCEMENT`）。
- **Done when**：同一套分层规则在 `AGENTS.md`、[`closeout_enforcement.md`](../closeout_enforcement.md)（若引用）与 `closeout_enforcement.rs` 中 **无矛盾表述**；空字符串 env 等边界有单测或文档脚注。
- **Verify**：`rg -n 'CLOSEOUT_ENFORCEMENT|closeout_record' AGENTS.md scripts/router-rs/src/closeout_enforcement.rs`；`cargo test --manifest-path scripts/router-rs/Cargo.toml`（触及 Rust 时必跑）。

### P0-3：`REVIEW_GATE` — 独立上下文子代理证据链 + **可复制的最小样例**

- **做什么**：为 Cursor **review** 路径固化「`.cursor/hook-state` phase + Stop 单行 `router-rs REVIEW_GATE`」所需的 **磁盘证据最小集**（含成功 / 失败对照），写入本 backlog 或链到 `docs/` 专节，避免团队口头约定。宿主契约：[host_adapter_contract.md](../host_adapter_contract.md)；执行叙事：[AGENTS.md](../../AGENTS.md) Host Boundaries / Execution Ladder。
- **Done when**：存在 **1 份**可被新人按步骤复现的样例（目录树 + 关键文件片段 + 期望 hook 输出关键词）；与 `review_gate` 相关 Rust 测试或 `tests/host_integration.rs` 断言不冲突。
- **Verify**：`rg -n 'REVIEW_GATE|review_gate' docs scripts/router-rs/src .cursor/hooks.json`；`cargo test --manifest-path scripts/router-rs/Cargo.toml`（全量；或改为显式过滤子串前先在 `tests/host_integration.rs` 内 `rg '#\[test\]'` 对齐测试名，避免 `-- host_integration` 与命名漂移）。

### P0-4：宿主漂移 — L4 hooks 与 **portable core** 对照表

- **做什么**：为每个支持宿主维护「事件 → `router-rs …` CLI → 写盘副作用」一行表，与 [host_adapter_contract.md](../host_adapter_contract.md) 快速路径对齐；集成回归锚点：[tests/host_integration.rs](../../tests/host_integration.rs)。
- **Done when**：`host_adapter_contract` 或 harness 映射表中出现与 `tests/host_integration.rs` **同名或可 grep 的锚点**（事件名 / flag / 文件路径）。
- **Verify**：`cargo test --manifest-path scripts/router-rs/Cargo.toml` 或通过 `rg -n 'host_integration|cursor hook|codex' tests/host_integration.rs docs/host_adapter_contract.md`。

---

## P1（中期：深度 / 外研 / 开关面收敛）

### P1-1：外研 JSON — 检索轨迹与「可复核外研」落盘形状

- **做什么**：将「外研加强」所需的 **结构化字段**（检索轨迹、多视角分离）与 L2 账本 / L5 契约对齐，避免仅靠 hook 长文案。契约长文：[reasoning-depth-contract.md](../references/rfv-loop/reasoning-depth-contract.md)（含调研深度、harness 方向）；上层位置：[harness_architecture.md §4](../harness_architecture.md#4-推理深度在上层的位置)。
- **Done when**：在 `docs/references/rfv-loop/` 或 `configs/framework/` 中存在 **JSON 形状草案或 schema 指针**（字段级）；与 `HARNESS_OPERATOR_NUDGES` / RFV skill 交叉引用一致。
- **Verify**：`rg -n 'retrieval|外研|JSON|trace' docs/references/rfv-loop/reasoning-depth-contract.md configs/framework/HARNESS_OPERATOR_NUDGES_SCHEMA.json`（路径以仓库为准）。

### P1-2：可程序化硬门禁 — `completion_gates` / `close_gates` 与 `DepthCompliance` 分工

- **做什么**：落实 [reasoning-depth-contract.md §可程序化硬门禁](../references/rfv-loop/reasoning-depth-contract.md) 与 [harness_architecture.md §4](../harness_architecture.md#4-推理深度在上层的位置) 的分工：默认 advisory、opt-in 硬门禁、`resolve_task_view` 校验路径无第二真源。
- **Done when**：开启硬门禁时的失败信息 **可定位到具体 gate 与缺失证据**；文档与 `task_state` / RFV 相关 Rust 行为一致。
- **Verify**：`rg -n 'completion_gates|close_gates|DepthCompliance' scripts/router-rs/src docs/references/rfv-loop/reasoning-depth-contract.md`。

### P1-3：RFV 高轮次与显式 close — 防无限 `append_round` 与续跑噪声

- **做什么**：为 **高轮次** RFV 场景定义 **显式 `append_round` close**、`max_rounds` 耗尽语义或 `close_gates` 组合策略，避免账本无限增长与续跑噪声。参考：[harness_architecture.md §8 开关矩阵](../harness_architecture.md#8-开关取舍矩阵深度注入相关)、[rfv_loop_harness.md](../rfv_loop_harness.md)。
- **Done when**：RFV 文档或 schema 中有一条 **可执行** 的「何时必须 close / pause」规则；必要时补 `router-rs` 警告或 metrics 钩子（若已有扩展点则复用）。
- **Verify**：`rg -n 'append_round|close_gates|RFV_LOOP|max_rounds' scripts/router-rs/src docs/rfv_loop_harness.md`。

#### 术语辨析：Closeout **R9**（与 RFV 轮次编号无关）

- **R9** 在仓库内指 **CloseoutRecord** 与 **任务级 depth 策略**（如 `GOAL_STATE.completion_gates`）对齐的 **推迟规则**，见 [closeout_enforcement.rs](../../scripts/router-rs/src/closeout_enforcement.rs) 内 R9 注释与 [reasoning-depth-contract.md §Closeout R9](../references/rfv-loop/reasoning-depth-contract.md)。**不得**与「RFV 第 N 轮」混为一谈。现阶段替代路径：用 `completion_gates` / `close_gates` 在 GOAL complete / RFV close 上硬拦。

### P1-4：开关 **preset** — `ROUTER_RS_*` 组合写入文档与可选 shell 片段

- **做什么**：把常见组合（如「全静默注入」「仅证据无续跑」「论文对抗仅 beforeSubmit」）整理为 **具名 preset**，真源落在 [router_env_flags.rs](../../scripts/router-rs/src/router_env_flags.rs) 注释 + [harness_architecture.md §5–§8](../harness_architecture.md#5-扩展规则避免继续加抽象失控) + [AGENTS.md](../../AGENTS.md) 个人使用节，避免口口相传。
- **Done when**：至少 **2 个**具名 preset，每个列出 env 三元组与影响面（对照 §8 矩阵）；与 `ROUTER_RS_OPERATOR_INJECT` 聚合语义无冲突表述。
- **Verify**：`rg -n 'ROUTER_RS_OPERATOR_INJECT|preset|矩阵' docs/harness_architecture.md AGENTS.md scripts/router-rs/src/router_env_flags.rs`。

---

## P2（长期：生态、多宿主、观测）

### P2-1：宿主漂移（工程化）— `RUNTIME_REGISTRY` 与 hook 模板生成单测

- **做什么**：新宿主接入时，强制走 [host_adapter_contract.md §3.1](../host_adapter_contract.md#31-可复制执行清单工程顺序)；将 `host_targets.supported` 与 `tests/host_integration.rs` 的覆盖范围对齐，减少「文档已写、集成未测」。
- **Done when**：每新增闭集宿主，**README 表 + 一条集成测试或 `router-rs framework maint verify-*`** 同步更新。
- **Verify**：`cargo test --manifest-path scripts/router-rs/Cargo.toml`；`rg -n 'RUNTIME_REGISTRY|supported' configs/framework/RUNTIME_REGISTRY.json tests/host_integration.rs`。

### P2-2：`REVIEW_GATE` 证据样例库 — 多场景 fixture 目录

- **做什么**：在 `tests/fixtures/` 或 `docs/plans/` 维护 **只读 fixture**（脱敏），供 `review_gate` 与文档共用，降低「子代理 lane 是否算数」争议。
- **Done when**：≥2 个 fixture（例如：缺证据 / 证据完整）；文档链接到路径。
- **Verify**：`test -d tests/fixtures`（若创建）或 `ls docs/plans/*review*`；`cargo test` 含相关用例。

### P2-3：外研 JSON 与 operator 注入的统一配置面

- **做什么**：若外研轨迹进入 `HARNESS_OPERATOR_NUDGES` 或并行 JSON 真源，保证 **schema 版本化**与 [harness_architecture.md §5 规则 4](../harness_architecture.md#5-扩展规则避免继续加抽象失控) 一致（不匹配则回退内置默认的行为有测）。
- **Done when**：schema + 单测 + 文档「读模型」一节同步更新。
- **Verify**：`rg -n 'HARNESS_OPERATOR_NUDGES|harness_operator' scripts/router-rs/src configs/framework`。

### P2-4：与 Plan 闸门对齐的 harness 验收钩子

- **做什么**：在走 Cursor Plan / `/gitx plan` 收口时，将本 backlog 的 **Verify** 行与 `skills/plan-mode` 的验证形状对齐（本仓库内由用户在 Cursor 宿主执行 **`/gitx plan`**；代理无法在子环境代跑）。
- **Done when**：`plan-mode` 或 checklist 文档中可链回本文 **P0 验证命令**。
- **Verify**：用户于 Cursor 执行 `/gitx plan`（**此处不可由 CI 子代理替代**）。

### P2-5：Cursor `additional_context` 无总 cap（弱模型 / 长会话噪声）

- **做什么**：记录 Cursor 侧 `merge_additional_context` **无合并后总字符 cap** 的产品风险与缓解依赖（`retired silent-mode branch`、段落 strip 等）；可选后续增加 **`ROUTER_RS_*` 总 cap** 或宿主侧观测，属 **产品决策**，不在未立项前改 `cursor_hooks/` 默认行为。
- **证据与路径**：[`context_token_audit_deep_dive.md`](context_token_audit_deep_dive.md) §2–§3；[`scripts/router-rs/src/cursor_hooks/mod.rs`](../../scripts/router-rs/src/cursor_hooks/mod.rs)（同目录 `frag_*.rs` 内含 `merge_additional_context`，以 `rg` 为准）。
- **Done when**：本小节可被 `docs/plans/RESEARCH_harness_weak_model_top_tier.md` / `docs/harness_architecture.md` §6 指针命中；执行 execution 时优先 **文档 + preset** 再议 cap。
- **Verify**：`rg -n 'merge_additional_context|cursor_hooks|context_token_audit' docs/plans/harness_improvement_backlog.md docs/plans/context_token_audit_deep_dive.md`。

---

## 建议 PR切片（独立可合并方向）

以下为 **2–4 个**可独立合并的方向（按依赖弱→强排序）；每个 PR 应自带文档 + 测或 grep 验收。

| 方向 | 范围（示例路径） | 合并价值 |
|------|------------------|----------|
| **PR-A：证据链与 EVIDENCE_INDEX** | `scripts/router-rs/src/hook_common*.rs`（若适用）、`docs/harness_architecture.md`、`docs/plans/*` | 降低「完成了但没记录」风险；与 L1/L2 边界一致。 |
| **PR-B：Closeout 硬门禁与叙事对齐** | `scripts/router-rs/src/closeout_enforcement.rs`、`AGENTS.md`、`docs/closeout_enforcement.md` | CI/本地分层单一真源；减少误设空字符串 env。 |
| **PR-C：REVIEW_GATE fixture + 文档样例** | `.cursor/hooks.json`（仅当契约变）、`tests/host_integration.rs`、`docs/host_adapter_contract.md`、`tests/fixtures/` | 可复现 review 证据链；利于 onboarding。 |
| **PR-D：开关 preset + 外研 JSON schema 草案** | `scripts/router-rs/src/router_env_flags.rs`、`configs/framework/`、`docs/references/rfv-loop/reasoning-depth-contract.md` | 调试体验与长期配置面收敛；可与 PR-A/B 并行若文件不重叠。 |
| **PR-E：Claude Code L4 薄壳（已落地，后续仅增量）** | `RUNTIME_REGISTRY`、`scripts/router-rs/src/claude_hooks.rs`、`scripts/router-rs/src/host_integration.rs`、[host_adapter_contract.md §3.1](../host_adapter_contract.md#31-可复制执行清单工程顺序) | Claude Code 已进入闭集宿主与 project projection；后续只做 hooks/投影增量强化，不再作为未立项第三宿主。 |

---

## 参考链接（真源索引）

| 主题 | 链接 |
|------|------|
| 五层模型、证据流、续跑流、开关矩阵 | [docs/harness_architecture.md](../harness_architecture.md) |
| 推理深度、外研、harness 方向、可程序化硬门禁 | [docs/references/rfv-loop/reasoning-depth-contract.md](../references/rfv-loop/reasoning-depth-contract.md) |
| Closeout 分层实现 | [scripts/router-rs/src/closeout_enforcement.rs](../../scripts/router-rs/src/closeout_enforcement.rs) |
| 环境变量集中与开关语义 | [scripts/router-rs/src/router_env_flags.rs](../../scripts/router-rs/src/router_env_flags.rs) |
| 跨宿主策略与 Host Boundaries | [AGENTS.md](../../AGENTS.md) |
| 新宿主 / portable core / 工程顺序 | [docs/host_adapter_contract.md](../host_adapter_contract.md) |
| 宿主集成回归 | [tests/host_integration.rs](../../tests/host_integration.rs) |

---

## `/gitx plan` 说明（阻塞提示）

- **`/gitx plan`** 仅在用户 **Cursor 宿主** 侧执行；本任务交付为磁盘文档与索引更新，**不包含**该命令的自动运行。
- 若 Plan 要求对照本 backlog 的 **Verify** 列，请在合并前于本地依次执行表中命令，或在 Cursor 中运行 `/gitx plan` 做计划对照收口。
