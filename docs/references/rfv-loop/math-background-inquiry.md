---
last_verified: "2026-06-05"
depends_on:
  - math-reasoning-harness.md
  - external-research-harness.md
  - lane-templates.md
  - ../../../skills/research-discovery/references/academic-sources.md
---

# 数学背景深挖 harness（未知性质 → 理论地图）

> **status: aspirational** — RFV 多轮 loop 在 `my-light` profile 下很少使用；本文件描述的数学背景深挖 harness 为计划中的深度推理模式。

面向 **「还不清楚该问题属于哪类数学、有哪些定理/框架可能相关、跨领域类比是否成立」**。与 [math-reasoning-harness.md](math-reasoning-harness.md) §G 配套；**PASS/FAIL 仍只认** `falsification_tests` + `verify_commands` + `EVIDENCE_INDEX`——背景地图 **不能** 顶替验证。

## 外部实践对照（可迁移部分）

| 来源 | 可迁移机制 | 本 harness 落点 |
|------|------------|-----------------|
| [AI Mathematician (AIM)](https://ar5iv.labs.arxiv.org/html/2505.22451) | explorer / verifier / refiner 双环；研究题拉长探索路径 | RFV 单轮内 Phase A 多路只读 + 多轮 `continue`；**不**复刻 NL「PRV」为通过标准 |
| [Yanasse / Deep Vision 类比](https://arxiv.org/html/2604.17229) | **语义适配**证明/ tactic 模式，**非**符号替换；显式 `breaks_when` | `analogy_candidates` + `theorem_applicability.fails_when` |
| [Axiom-Based Atlas](https://arxiv.org/html/2504.00063v1) | 定理按逻辑依赖/公理向量做 **结构邻近** | `standard_objects` + `cross_domain_bridges` |
| [LeanAgent](https://openreview.net/forum?id=Uo4EHT4ZZ8) | 跨库检索已知定理片段（Mathlib 类） | External strict `claims` + `retrieval_trace` 扇出 |

**不迁移**：全自动 Mathlib 证明搜索、几何合成闭包、仅凭 embedding 相似就宣称「可证」。

## 操作员 workflow（默认）

1. **路由**：[`$research-discovery`](../../../skills/research-discovery/SKILL.md) → lane `math_background_inquiry`（勿与纯「证不等式」混用 `$math-derivation`）。
2. **RFV start**：`allow_external_research=true`，`external_research_strict=true`（除非快查）。
3. **Phase A（≤3 路只读）**：优先 `External(external_mode=math_background)`；复杂时可加 `STEM_THEORY_BACKGROUND` 或 `Reviewer`。
4. **检索扇出**（硬性）：`retrieval_trace.queries_used` **≥3** 且覆盖 **≥2 类来源**（见 [academic-sources.md](../../../skills/research-discovery/references/academic-sources.md) — 至少 arXiv/OpenAlex/CrossRef 中二者）；`math.*` / `stat.*` 分类按问题选。
5. **Supervisor 背景合并**（非 subagent）：把 `theory_background` 与 `claims` 对照；**每条类比必须有 `breaks_when`**；适用定理写入 `theorem_applicability`；仍未知 → `open_mathematical_gaps` 或 §D `conjecture_list`。
6. **检验升格**：对 **可形式化** 的缺口起草 `falsification_tests`（SymPy 化简、小反例枚举、已知定理假设核对脚本）→ STEM 三 lane → Verifier → `append_round`。

## `theory_background` 增强字段（结构化）

除 §G 基础块外，深度探讨 **应** 填（见 `RFV_EXTERNAL_RESEARCH.schema.json`）：

| 字段 | 用途 |
|------|------|
| `theorem_applicability` | 命名定理：**`applies_when` / `fails_when`** + `sources` |
| `cross_domain_bridges` | 跨领域桥接思路 + **何时桥接断裂** |
| `proof_strategy_hints` | 证明 **模式**（非完整证明）：来源领域、目标用法、局限 |
| `retrieval_fanout_plan` | 计划检索式（与 `retrieval_trace.queries_used` 对齐） |

## 反模式

- 只列定理名、不写 **假设** 与 **失效条件**。
- 类比不做 `breaks_when`（Yanasse 强调：禁止符号替换式假类比）。
- 用背景综述顶替 `contradiction_sweep` 或 verifier。
- 单源博客链替代 arXiv/OpenAlex/CrossRef 扇出。

## 与 §D 分工

| 产出 | 字段 |
|------|------|
| 「该用什么理论、已知什么」 | `theory_background` + `claims` |
| 「可能的新结构/引理」 | `conjecture_list` |
| 「这条背景是否站得住」 | `falsification_tests` + `EVIDENCE_INDEX` |
