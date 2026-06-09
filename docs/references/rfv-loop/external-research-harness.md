---
last_verified: "2026-06-04"
depends_on:
  - ../../spec.md
  - lane-templates.md
  - math-reasoning-harness.md
---

# External research harness（结构化 `external_research`）

> **status: aspirational** — RFV 多轮 loop 在 `my-light` profile 下很少使用；本文件描述的结构化 external_research 流程为计划中的深度调研模式，当前大部分任务未触发此路径。

**Rust 校验真源**：`core/runtime-core/src/rfv_loop/`。`append_round` 在传入 **`external_research`** 且为 **JSON 对象**（非 `null`）时：**先** `validate_external_research_structured`（字段存在、非空字符串等基线）；**若** 当前任务 `RFV_LOOP_STATE.external_research_strict == true`（`start` 默认写入；磁盘缺键或非布尔则视为 `false` 以兼容旧账本），**再** `validate_external_research_strict`（可追溯来源、矛盾扫描体量、检索轨迹长度、`unknowns` 键等）。任一阶段失败 → **`Err`，不写盘**。机读草稿 schema：`configs/framework/RFV_EXTERNAL_RESEARCH.schema.json`（字段说明含 strict 期望；**数值下限以 Rust 为准**，避免 schema 与执行双真源）。

**Runbook**：外研有两种叙述形态——**compact**（由 supervisor 在 `external_research_summary` 压缩 prose）与 **structured**（本页的 JSON 负载，便于审计与 rollup）。结构化块**不负责**顶替 **verifier**：**PASS/FAIL** 仍以 **`verify_commands` 执行记录**、`EVIDENCE_INDEX`（及 `verify_result`）为准；定量复算同属「可执行验证」精神与 `lane-templates` 中的 `replay_command`。与 **STEM** **`adversarial_findings` / `falsification_tests`** 正交——后者管数理推翻面，本节管可追溯外研/API 形状。

**STEM 扩展块（与 `claims` 正交）**：`conjecture_list`（§D）、`model_spec`（§F，`external_mode=modeling`）、`theory_background`（§G，`external_mode=math_background`）见 `RFV_EXTERNAL_RESEARCH.schema.json`。**不得**把猜想/模型/背景散文写进 `claims` 冒充文献。**不受** strict 的 witness `sources` 规则约束的块仍须 downstream **`falsification_tests` + verify**。编排见 [math-reasoning-harness.md](math-reasoning-harness.md) §D–G。

| 字段 | 必填 | 形状（摘要） | strict 附加（当任务 `external_research_strict=true`） |
|------|------|----------------|-------------------------------------------|
| `claims` | 是 | 非空数组；每项含 `claim`（非空字符串）、`sources`（非空字符串数组） | 每项 `sources` **≥2**；每条字符串须通过 `source_traceable_heuristic`（`http(s)://`、`doi:10.` / `10…/`、不区分大小写前缀 `arxiv:`/`pmid:`/`isbn:`/`dataset:`/`official_doc:`） |
| `contradiction_sweep` | 是 | 非空数组；每项含 `related_claim_or_topic`、`contradicting_or_limiting_evidence`、`sources`（非空数组） | 数组长度 **≥ max(2, claims.len())**；每项 `sources` 仍 **≥1**（基线），且每条通过同一启发式 |
| `retrieval_trace` | 是 | 对象：`queries_used`（非空字符串数组）、`inclusion_rules`、`exclusions`、`exclusion_rationale`（皆非空字符串） | `queries_used` **≥3**；三 prose 字段 **trim 后长度各 ≥40**（常量 `EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN`） |
| `unknowns` | 否（基线） | `null` 或对象数组；每项 `question`、`why_insufficient` | **必须出现 `unknowns` 键**；值为 **`[]` 或 `null`**（禁止省略键） |
| `quantitative_replays` | 否 | 省略 / `null` / 字符串 `"none"`（大小写不敏感），或非空对象数组（`dataset_or_source_id`、`version_or_snapshot`、`window`、`replay_command`） | 不变 |
| `conjecture_list` | 否 | `null` 或非空数组；每项 `id`、`statement`、`predicted_witnesses`（≥1 字符串）；可选 `status`（`open`/`promoted`/`rejected`）、`verify_command_draft` | **不变**（strict 不额外加文献式 `sources` 要求） |
| `model_spec` | 否 | `null` 或对象：`phenomenon`、`state_variables`、`governing_equations` 必填；可选 regime_chart、identifiability_risks 等 | **不变** |
| `theory_background` | 否 | `null` 或对象：必填 `problem_class`；可选 `standard_objects`、`key_theorems_named`、`analogy_candidates`、`theorem_applicability`、`cross_domain_bridges`、`proof_strategy_hints`、`retrieval_fanout_plan`、`open_mathematical_gaps` | **不变**；深度 runbook [spec.md](../../spec.md#12-math-background-inquiry) |

**操作员提示（2026-05 连续性退出）**：**无** hook `RFV_LOOP_CONTINUE` / digest。外研缺口时 supervisor 用 `framework_rfv_loop` stdio + 本文 / `RFV_EXTERNAL_RESEARCH.schema.json`；`HARNESS_OPERATOR_NUDGES.json` 仅为文案真源。深度合规 rollup：`task_state` 的 **`rfv_external_deep_structured_round_count`**（有对象即计数）与 **`rfv_external_strict_ok_round_count`**（仅当任务 `external_research_strict` 为真且该轮 blob 通过 strict 校验时递增）。**账本式外研路径**与 Execute `research_mode=deep`/Plan `plan_profile` 的职责分工（不自动合并）分层见 [`docs/spec.md`](../../spec.md#12-closeout-与生命周期) — **Closeout 与生命周期**。

**与 `RUNTIME_REGISTRY.json` 的关系**：Execute **deep** 叙事**不**挂在 registry 的 `framework_commands` 块（退役的 autopilot 命令已删除）；`router-rs` 的 Execute live 塑形**不**在运行时读取 registry 中的 research 字段，真源为 [`core/runtime-core/src/cli/runtime_ops/body.rs`](../../../core/runtime-core/src/cli/runtime_ops/body.rs) 中 `build_live_execute_prompt` 的内嵌英文条款。改 deep 叙事时请同步该文件（`tests/policy_contracts.rs` 中有防漂移断言）。

**See also**: [lane-templates.md](lane-templates.md)（External research 深度模式）、[spec.md](../../spec.md#12-reasoning-depth-contract)、[spec.md](../../spec.md)。
