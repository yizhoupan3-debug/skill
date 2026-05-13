# External research harness（结构化 `external_research`）

**Rust 校验真源**：`scripts/router-rs/src/rfv_loop.rs`。`append_round` 在传入 **`external_research`** 且为 **JSON 对象**（非 `null`）时：**先** `validate_external_research_structured`（字段存在、非空字符串等基线）；**若** 当前任务 `RFV_LOOP_STATE.external_research_strict == true`（`start` 默认写入；磁盘缺键或非布尔则视为 `false` 以兼容旧账本），**再** `validate_external_research_strict`（可追溯来源、矛盾扫描体量、检索轨迹长度、`unknowns` 键等）。任一阶段失败 → **`Err`，不写盘**。机读草稿 schema：`configs/framework/RFV_EXTERNAL_RESEARCH.schema.json`（字段说明含 strict 期望；**数值下限以 Rust 为准**，避免 schema 与执行双真源）。

**Runbook**：外研有两种叙述形态——**compact**（由 supervisor 在 `external_research_summary` 压缩 prose）与 **structured**（本页的 JSON 负载，便于审计与 rollup）。结构化块**不负责**顶替 **verifier**：**PASS/FAIL** 仍以 **`verify_commands` 执行记录**、`EVIDENCE_INDEX`（及 `verify_result`）为准；定量复算同属「可执行验证」精神与 `lane-templates` 中的 `replay_command`。与 **STEM** **`adversarial_findings` / `falsification_tests`** 正交——后者管数理推翻面，本节管可追溯外研/API 形状。

| 字段 | 必填 | 形状（摘要） | strict 附加（当任务 `external_research_strict=true`） |
|------|------|----------------|-------------------------------------------|
| `claims` | 是 | 非空数组；每项含 `claim`（非空字符串）、`sources`（非空字符串数组） | 每项 `sources` **≥2**；每条字符串须通过 `source_traceable_heuristic`（`http(s)://`、`doi:10.` / `10…/`、不区分大小写前缀 `arxiv:`/`pmid:`/`isbn:`/`dataset:`/`official_doc:`） |
| `contradiction_sweep` | 是 | 非空数组；每项含 `related_claim_or_topic`、`contradicting_or_limiting_evidence`、`sources`（非空数组） | 数组长度 **≥ max(2, claims.len())**；每项 `sources` 仍 **≥1**（基线），且每条通过同一启发式 |
| `retrieval_trace` | 是 | 对象：`queries_used`（非空字符串数组）、`inclusion_rules`、`exclusions`、`exclusion_rationale`（皆非空字符串） | `queries_used` **≥3**；三 prose 字段 **trim 后长度各 ≥40**（常量 `EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN`） |
| `unknowns` | 否（基线） | `null` 或对象数组；每项 `question`、`why_insufficient` | **必须出现 `unknowns` 键**；值为 **`[]` 或 `null`**（禁止省略键） |
| `quantitative_replays` | 否 | 省略 / `null` / 字符串 `"none"`（大小写不敏感），或非空对象数组（`dataset_or_source_id`、`version_or_snapshot`、`window`、`replay_command`） | 不变 |

**宿主提示（可选）**：在 RFV active、**`allow_external_research`**、**`prefer_structured_external_research=true`**（`start` 持久化）且上一轮缺 `external_research` 时，`RFV_LOOP_CONTINUE` 只追加短提示和 schema 路径；**`ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT=0`** 关闭。RFV 也会在外研开启时追加检索 / `retrieval_trace` 短句；长版 strict 说明保留在本文件和 schema 中。深度合规 rollup：`task_state` 的 **`rfv_external_deep_structured_round_count`**（有对象即计数）与 **`rfv_external_strict_ok_round_count`**（仅当任务 `external_research_strict` 为真且该轮 blob 通过 strict 校验时递增）。**账本式外研路径**与 Execute `research_mode=deep`/Plan `plan_profile` 的职责分工（不自动合并）见 [`docs/harness_architecture.md`](../../harness_architecture.md) — **Closeout 与深度** → **深度调研：三轨对齐**。

**与 `RUNTIME_REGISTRY.json` 的关系**：`framework_commands.autopilot.research_contract`（含 `deep` 字段）为宿主/文档侧**叙事契约**；`router-rs` 的 Execute live 塑形**不**在运行时读取该 JSON，真源为 [`scripts/router-rs/src/cli/runtime_ops.inc`](../../../scripts/router-rs/src/cli/runtime_ops.inc) 中 `build_live_execute_prompt` 的内嵌英文条款。改 deep 叙事时请同步该文件（`tests/policy_contracts.rs` 中有防漂移断言）。

**See also**: [lane-templates.md](lane-templates.md)（External research 深度模式）、[reasoning-depth-contract.md](reasoning-depth-contract.md)、[rfv_loop_harness.md](../../rfv_loop_harness.md)。
