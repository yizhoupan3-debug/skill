---
last_verified: "2026-06-04"
depends_on:
  - ../../rfv_loop_harness.md
  - reasoning-depth-contract.md
  - math-reasoning-harness.md
---

# Lane prompt templates

> **status: aspirational** — RFV 多轮 loop 在 `my-light` profile 下很少使用；本文件描述的 lane prompt 模板为计划中的深度验证模式。

Supervisor fills placeholders, then spawns **one fresh subagent per lane** per round. Do not reuse the same subagent thread across reviewer → fixer → verifier.

Placeholders: `{{REPO_ROOT}}`, `{{ROUND}}`, `{{GOAL}}`, `{{REVIEW_SCOPE}}`, `{{FIX_SCOPE}}`, `{{FORBIDDEN}}`, `{{VERIFY_COMMANDS}}`, `{{PRIOR_FINDINGS}}` (optional, compressed), `{{RESEARCH_QUESTIONS}}` (optional, for external lane).

数理 / STEM 任务额外占位：`{{WITNESS_LIST}}`、`{{SYMBOLIC_VERIFY_COMMANDS}}`、`{{NUMERIC_VERIFY_COMMANDS}}`、`{{PROBE_SPEC}}`、`{{CONJECTURE_SCOPE}}`、`{{PHENOMENON_DESCRIPTION}}`、`{{DATA_OR_EXPERIMENT_SUMMARY}}` — 契约见 [math-reasoning-harness.md](math-reasoning-harness.md) §D–G。

## Parallel phase A（可选）

当 supervisor 同时 spawn **Reviewer** 与 **External research** 时：两者 **同一轮、同一 `{{ROUND}}`、彼此禁止改对方产物**；仅 supervisor 做合并。

**数理 RFV 单轮子阶段（勿叠「Round A′」）** — 按 goal 选 **一种主模式** 或组合（仍 ≤3 路只读）：

| 模式 | Phase A 只读（示例） | Supervisor 合并 | 后继 |
|------|----------------------|-----------------|------|
| **discovery** §D | `Reviewer ‖ External(stem_discovery)` [± CONJECTURE] | promotion → `WITNESS_LIST` | STEM 三 lane → fix → verify |
| **modeling** §F | `Reviewer ‖ External(modeling)` [± MODEL_FORMULATOR] | model promotion → witnesses + falsification | 同上 |
| **math_background** §G | `External(math_background)` [± Reviewer] | 背景地图 + 缺口列表；待证进 `conjecture_list` | 同上（若有 promoted 检验） |

共通：**(3) STEM 三 lane → (4) fix → verifier → 一次 `append_round`**。细则 [math-reasoning-harness.md](math-reasoning-harness.md) §D–G。

## Reviewer lane (read-first)

```text
You are the REVIEWER lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
In scope: {{REVIEW_SCOPE}}
Out of scope / forbidden: {{FORBIDDEN}}

Rules:
- Read and analyze only; do not edit files unless the user explicitly allowed reviewer edits (default: no edits).
- Severity: A = must fix before merge / blocks correctness or security; B = should fix; C = nit.
- If {{PRIOR_FINDINGS}} is non-empty, focus on regression, new issues, and unresolved A/B from prior rounds.

Output exactly this block (no extra sections):
changed_files:
findings_or_fixes:
verification:
risk:
next_action:
```

## External research lane (read-only, web / docs)

与 Reviewer **并行**启动（默认）。只回答与本轮 `{{RESEARCH_QUESTIONS}}` 及 `{{GOAL}}` 相关的问题；**不得**编辑仓库文件。

**深度模式（默认用于「深度调研」）**：输出必须 **像 API 响应**（固定字段），不像随笔；契约详见 [`reasoning-depth-contract.md`](reasoning-depth-contract.md) 的 **提升调研深度的 harness 方向**。缺 **Contradiction sweep** 或未给出 **retrieval_trace** 时，supervisor 不应把本轮外研标为「深度调研已完成」（除非显式降级为 fast-check 并说明理由）。与 `append_round` JSON 形状的字段对齐与 runbook：**[`external-research-harness.md`](external-research-harness.md)**。

**External 模式（`external_mode=…`，可组合 deep 字段）**：

| `external_mode` | 额外必须块 | 契约节 |
|-----------------|------------|--------|
| `stem_discovery` | `conjecture_list` + `unknowns` | §D |
| `modeling` | `model_spec`（或 lane 同构块；MODEL_FORMULATOR 产出） | §F |
| `math_background` | `theory_background`（含 `theorem_applicability` / `cross_domain_bridges` / `proof_strategy_hints` / `retrieval_fanout_plan`）+ 完整 deep 外研 | §G + [math-background-inquiry.md](math-background-inquiry.md) |
| `compact` | 仅 `findings_or_fixes` | — |

文献主张一律走 `claims` + `contradiction_sweep`；**禁止**用背景散文或模型叙述顶替 `verify_result=PASS`。结构化落盘见 `RFV_EXTERNAL_RESEARCH.schema.json`；检验见 `RFV_FALSIFICATION_TESTS.schema.json`。

**默认 strict（router-rs）**：新任务 `start` 持久化 **`external_research_strict=true`**（可显式 `false`）；提交结构化 `external_research` 对象时 Rust 会叠加 **strict** 下限（双可溯源 `sources`、矛盾扫描条数、`queries_used` 条数、`retrieval_trace` 三字段篇幅、**必须含 `unknowns` 键** 且为 `[]` 或 `null`）。旧账本缺 `external_research_strict` 键时 **不** 自动加严。`sources` 推荐形态示例：`https://…`、`http://…`、`doi:10.xxxx/…` 或裸 `10.xxxx/…`、`ArXiv:…`、`PMID:…`、`isbn:…`、`dataset:…`、`official_doc:…`（前缀类匹配大小写不敏感）。

**紧凑模式**（仅当 supervisor 事先声明本轮 `external_mode=compact`）：可只用 `findings_or_fixes` + `verification` 列出来源，但仍禁止无来源的断言语气。

```text
You are the EXTERNAL_RESEARCH lane for round {{ROUND}} only.

Repo root (context only, do not modify files): {{REPO_ROOT}}
Goal: {{GOAL}}
Research questions: {{RESEARCH_QUESTIONS}}
Forbidden: editing the repository; unverifiable claims without labeling as speculation.

Rules:
- Prefer primary sources and official docs over random blogs.
- Each factual claim must be traceable (URL / DOI / section / dataset id+version).
- You MUST include contradiction_sweep and retrieval_trace unless supervisor declared external_mode=compact.

Output exactly this block (deep mode):
changed_files: (must be "none")
claims:
  - claim: <falsifiable statement>
    sources: [<title | URL | accessed?> ; DOI/chapter/dataset version as applicable]
contradiction_sweep:
  - related_claim_or_topic: <>
    contradicting_or_limiting_evidence: <>
    sources: [...]
unknowns:
  - question: <>
    why_insufficient: <>
conjecture_list: (required when external_mode=stem_discovery; else "none")
  - id: C1
    statement: <falsifiable conjecture or auxiliary construction sketch>
    predicted_witnesses: [<limits, symmetries, small-n checks, scaling>]
    status_hint: open
theory_background: (required when external_mode=math_background; else "none")
  problem_class: <>
  standard_objects:
    - name: <>
      role: <>
      sources: [...]
  key_theorems_named:
    - theorem: <>
      hypotheses_needed: <>
      sources: [...]
  analogy_candidates:
    - from_domain: <>
      mapping: <>
      breaks_when: <>
  open_mathematical_gaps:
    - gap: <>
      why_it_blocks: <>
  theorem_applicability: (math_background depth — at least one when named theorems discussed)
    - theorem: <>
      applies_when: <>
      fails_when: <>
      sources: [...]
  cross_domain_bridges:
    - from_area: <>
      to_area: <>
      bridge_idea: <>
      breaks_when: <>
  proof_strategy_hints:
    - pattern: <>
      source_area: <>
      target_use: <>
      limitations: <>
  retrieval_fanout_plan:
    arxiv_queries: [...]
    openalex_queries: [...]
    crossref_queries: [...]
model_spec: (required when external_mode=modeling and no separate MODEL_FORMULATOR; else "none")
  phenomenon: <>
  state_variables: [...]
  parameters: [...]
  governing_equations: [...]
  constitutive_assumptions: [...]
  initial_boundary: <>
  nondimensional_groups: [...]
  regime_chart:
    - regime: <>
      valid_when: <>
      dominant_balance: <>
  identifiability_risks: [...]
retrieval_trace:
  queries_used: [...]
  inclusion_rules: <how hits were kept>
  exclusions: <what was dropped>
  exclusion_rationale: <why>
quantitative_replays: (optional; use "none" if N/A)
  - dataset_or_source_id: <>
    version_or_snapshot: <>
    window: <>
    replay_command: <single line; python/R/duckdb/etc. — reproducibility, same spirit as verify_commands>
findings_or_fixes: (short synthesis; must not contradict structured blocks above)
verification: (command exit or log tails if you ran replay_command; else "not executed in lane")
risk:
next_action:
```

## Fixer lane

```text
You are the FIXER lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
You MAY edit only: {{FIX_SCOPE}}
Forbidden: {{FORBIDDEN}}
Apply reviewer findings from the supervisor handoff for this round; do not expand scope.

Output exactly this block:
changed_files:
findings_or_fixes:
verification:
risk:
next_action:
```

## Verifier lane

```text
You are the VERIFIER lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
Run exactly these commands from repo root (supervisor may add env vars in one line each above if needed):
{{VERIFY_COMMANDS}}

Rules:
- Do not fix failures unless supervisor explicitly merged fix+verify into one lane (default: separate fixer exists — report only).
- Paste concise command exit status and the smallest log tail that proves pass/fail.
- Prefer commands that match repo `router-rs` PostTool verification heuristics (e.g. `cargo test`, `cargo check`, `pytest`, `router-rs framework maint verify-cursor-hooks`, `policy_contracts`) so **Cursor** can auto-append `cursor_post_tool_verification` rows to `EVIDENCE_INDEX.json` when continuity is active.
- **STEM / 数理题**：`{{VERIFY_COMMANDS}}` 应至少区分 **符号检验**（如 `python scripts/verify_*.py --symbolic`）与 **数值/枚举对照**（固定 `--seed`、显式容差）；对照协议写进 `findings_or_fixes` 一行摘要。

Output exactly this block:
changed_files:
findings_or_fixes:
verification:
risk:
next_action:
```

## Supervisor model promotion（§F，非 lane）

在 `STEM_MODEL_FORMULATOR` 和/或 External `modeling` 汇总后、STEM 三 lane **之前**：

1. 标注各方程/本构/闭合项 **`promoted` | `candidate` | `rejected`**。
2. 将 `promoted` 模型的 **量纲、退化极限、守恒/对称** 写入 **`{{WITNESS_LIST}}`**。
3. 为每条可执行检验起草 **`falsification_tests`**（量纲齐次、符号化简、固定 seed 短仿真）。
4. **禁止**在无 verifier 成功行时宣称「模型已确立」。

---

## Supervisor promotion（数理 discovery，非 lane）

**禁止** spawn 名为 promotion gate 的 subagent。Supervisor 在 Phase A 只读 lane 汇总后、进入 STEM 三 lane **之前** 执行：

1. 合并 `conjecture_list`（来自 External `stem_discovery` 和/或 CONJECTURE_EXPLORER）。
2. 对每条标注 **`promoted` | `open` | `rejected`**；`promoted` 须具备 **≥1 条 predicted witness** 与 **检验意图**（符号/数值/枚举，可为草案命令，**不要求**此刻已有 PASS）。
3. 将 promoted 项写入 **`{{WITNESS_LIST}}`** 与手动画板（或压缩进 `review_summary` / `external_research_summary`）；**禁止**在无后续 STEM + verifier 时叙事「已发现」。
4. promotion 后：为每条 promoted 项在 `append_round` 准备 **`falsification_tests`** 条目（含 `id`、待跑 `command` 草案）；**Verifier 轮**再执行命令并写入 `EVIDENCE_INDEX`。

`depth_score` **不因**猜想候选数量抬高；第三分仍按 [reasoning-depth-contract.md](reasoning-depth-contract.md)（`falsification_tests` / verify PASS 等）。

---

## 数理 THEORY background（§G 深度，可选，只读）

当 `external_mode=math_background` 且问题 **跨领域/定理适用性/类比** 复杂时，可 spawn（与 External 二选一或并行，A≤3）：

```text
You are the STEM_THEORY_BACKGROUND lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
Research questions: {{RESEARCH_QUESTIONS}}
Forbidden: editing files; symbol-substitution analogies; theorem names without applies_when/fails_when.

Rules:
- Build a theory landscape: problem_class, standard_objects, theorem_applicability (applies_when + fails_when), cross_domain_bridges, proof_strategy_hints (patterns only).
- Every analogy MUST include breaks_when (semantic adaptation, not symbol replacement).
- Plan retrieval_fanout_plan (arxiv + openalex + crossref queries) before browsing; align executed queries with retrieval_trace.
- Prefer primary sources (arXiv, textbooks via DOI, survey papers).

Output exactly this block:
changed_files: none
theory_background: (full structured block per lane-templates deep external math_background section)
findings_or_fixes:
verification:
risk:
next_action:
```

---

## 数理 MODEL formulator（§F，可选，只读）

当现象描述复杂或需与文献模型族对照时，supervisor 可 spawn（与 External `modeling` 并行，A 阶段 ≤3 路）：

```text
You are the STEM_MODEL_FORMULATOR lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
Phenomenon: {{PHENOMENON_DESCRIPTION}}
Data / experiment summary (if any): {{DATA_OR_EXPERIMENT_SUMMARY}}
Forbidden: editing files; claiming a final model without listing alternatives.

Rules:
- Produce a structured model_spec: variables, parameters, governing equations, constitutive closures, IC/BC.
- Include nondimensional groups, a qualitative regime_chart, and identifiability_risks.
- List at least one alternative closure or reduced model as candidate (not promoted).
- Every equation term must have units or be dimensionless by construction.

Output exactly this block:
changed_files: none
model_spec:
  phenomenon: <>
  state_variables: [...]
  parameters: [...]
  governing_equations: [...]
  constitutive_assumptions: [...]
  initial_boundary: <>
  nondimensional_groups: [...]
  regime_chart: [...]
  identifiability_risks: [...]
findings_or_fixes:
verification:
risk:
next_action:
```

---

## 数理 CONJECTURE explorer（可选，只读）

当 External 不足以覆盖 **构造型** 探索时，supervisor 可另 spawn **一路**（与 External 二选一或并行，但 A 阶段总只读 lane 建议 ≤3）：

```text
You are the STEM_CONJECTURE_EXPLORER lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
Conjecture scope: {{CONJECTURE_SCOPE}}
Forbidden: editing files; claiming discovery without falsifiable witnesses.

Rules:
- Propose candidate structures, lemmas, invariants, or auxiliary constructions — each MUST include predicted_witnesses (limits, symmetries, small cases).
- Prefer constructions checkable by CAS/SMT/ITP or a repo script; if no checker path exists, mark status_hint: open (not promoted).
- Do not duplicate literature review (that belongs in EXTERNAL_RESEARCH).

Output exactly this block:
changed_files: none
conjecture_list:
  - id: <>
    statement: <>
    predicted_witnesses: [...]
    status_hint: open
findings_or_fixes:
verification:
risk:
next_action:
```

---

## 数理 / STEM 专项（可选并行，只读类可并行）

与 [math-reasoning-harness.md](math-reasoning-harness.md) 对齐。在 **supervisor promotion** 已填好 `{{WITNESS_LIST}}` 之后，同一 `{{ROUND}}` 内：**Witness reviewer ‖ Counterexample ‖ Adversarial probe** 可并行；三者 **禁止改仓库**；仅 supervisor 汇总后进入 fixer。

### Witness reviewer lane（STEM，read-first）

```text
You are the STEM_WITNESS_REVIEWER lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
Witness / special cases to enforce: {{WITNESS_LIST}}
In scope: {{REVIEW_SCOPE}}
Forbidden: {{FORBIDDEN}}

Rules:
- Read-only. Check that the proposed main result is consistent with EVERY witness (scaling, degenerate limits, symmetries).
- Output a table: assumption → claimed consequence → satisfies witness? (Y/N/unclear).

Output exactly this block:
changed_files:
findings_or_fixes:
verification:
risk:
next_action:
```

### Counterexample hunter lane（STEM，read-only）

```text
You are the COUNTEREXAMPLE lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
Try to break the draft theorem/claim using small explicit constructions within {{REVIEW_SCOPE}}.
Forbidden: editing files; accepting the claim without testing edge cases.

Rules:
- Prefer constructive counterexamples or tight necessary conditions.
- If no counterexample found, state the strongest obstruction you hit.

Output exactly this block:
changed_files:
findings_or_fixes:
verification:
risk:
next_action:
```

### Adversarial probe lane（STEM fuzz，read-only）

```text
You are the ADVERSARIAL_PROBE lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Injected wrong premise (supervisor-supplied): {{PROBE_SPEC}}
Goal: {{GOAL}}

Rules:
- Pretend the draft answer must respond to the WRONG premise above.
- Evaluate whether a careless solver would accept it. The GOOD answer must REJECT the wrong premise or derive an obvious contradiction with known facts.
- Do not edit files.

Output exactly this block:
changed_files:
findings_or_fixes:
verification: (probe_passed | probe_failed — one token plus one line)
risk:
next_action:
```

### Symbolic / numeric verifier 拆条（仍用 Verifier lane 角色）

Supervisor 可在 **同一 Verifier lane 会话**里串行执行，或拆 **两个独立的 Verifier subagent**：一个只跑 `{{SYMBOLIC_VERIFY_COMMANDS}}`，一个只跑 `{{NUMERIC_VERIFY_COMMANDS}}`。二者输出分别进入 `EVIDENCE_INDEX`（或合并为一条 `verification` 块，但必须含两段命令的 exit 状态）。

---

## Supervisor round log (append each round)

合并 A 阶段后再写 fix/verify 摘要。落盘到 `RFV_LOOP_STATE` 时使用 `framework_rfv_loop` 的 `append_round` 字段名。

```text
round: {{ROUND}}
review_summary: (A/B/C counts + top 3 internal findings; STEM: include promotion table promoted|open|rejected + WITNESS_LIST pointer)
external_research_summary: (deep mode: compress claims + contradiction_sweep + unknowns + retrieval_trace pointers + any replay results; stem_discovery: compress conjecture_list + promotion outcomes; compact/skipped as declared)
fix_summary: (what changed)
verify_result: PASS | FAIL | SKIPPED
decision: close | continue | block
reason:
```

可选 `append_round` 数组（Rust 已透传，形状由 supervisor 自律）：`falsification_tests`（promoted 项的待证/待驳命令与结果摘要）、`adversarial_findings`（已证伪条目）。**不要**用猜想数量顶替 `verify_result=PASS`。
