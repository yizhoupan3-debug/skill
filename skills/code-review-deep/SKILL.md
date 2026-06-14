---
name: code-review-deep
description: |
  Deep adversarial-style code review (review-only). Default visible output is a compact, severity-sorted findings list; narrative sections only when explicitly requested.
  Model selects lenses from an extensible catalog (core + optional: first principles/subtraction, dead-code signals, stale docs); exhaustive within chosen lenses.
  Broad/deep/PR-level work authorizes read-only independent reviewer subagents (fork_context=false) before main-thread synthesis. Does not silently rewrite implementation
  unless the user explicitly exits review-only posture.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: preferred
user-invocable: true
disable-model-invocation: false
trigger_hints:
  - $code-review-deep
  - code-review-deep
  - review
  - code review
  - 代码审查
  - 帮我 review
  - deep code review
  - 深度代码审查
  - 严苛代码评审
  - security code review
  - adversarial code review
  - 只允许审不改
  - CVE 审查
  - 供应链安全
metadata:
  version: "2.1.0"
  platforms: [supported]
  tags: [code-review, security, correctness, delegation, adversarial-review]
framework_roles:
  - detector
  - planner
  - verifier
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: true
risk: medium
source: local
allowed_tools:
  - Read
  - Bash
  - Agent
  - mcp__mcp-codegraph__codegraph_search
  - mcp__mcp-codegraph__codegraph_callers
  - mcp__mcp-codegraph__codegraph_callees
  - mcp__mcp-codegraph__codegraph_impact
  - mcp__mcp-codegraph__codegraph_node
  - mcp__mcp-codegraph__codegraph_status
---

## Quick Ref
- **Purpose**: 深度对抗式代码审查（review-only），默认输出 severity-sorted findings 紧凑列表
- **Key Rules**: 默认只审不改；hostile-but-fair 立场；P0/P1 须有 evidence；lens 可扩展目录选型；broad review ≥2 spawned reviewer lane
- **Trigger**: "review"、"代码审查"、"帮我 review"、"deep code review"、"$code-review-deep"
<!-- full content below; load on demand -->

# Code review (deep owner)

Judgment-focused review for code and change sets **without** rewriting by default. Portable across repositories: do **not** assume framework-specific files or audit commands exist unless the workspace is this skill/harness repo.

## Default posture

- **Findings-only by default (hard stop)**: On a review request, **do not** edit files, add tests, run fix commits, open PRs, or continue into implement / `/implementx` / gitx / loop unless the user **explicitly** exits review-only (e.g. "fix these findings", "implement", "merge", "commit"). End with findings (+ optional one-line verdict), not execution.
- Assume a **hostile but fair** reviewer: maximize plausible failure under real abuse, regressions, flaky ops, closest prior API expectations, dependency churn, or incomplete tests.
- **Compact default output = less prose, not shallower reasoning.** Analysis standard is unchanged: choose lenses internally, exhaust findings **within each selected lens**, apply the severity evidence gate.
- **Lens catalog, not a fixed runway**: choose lenses from [`references/review-dimensions.md`](references/review-dimensions.md). When the user asks to **cover all dimensions** / **全维度**, apply the full catalog **and** use the **full report profile**.

## Output format — compact (default)

Everything the host/user sees in chat under default compact. Lens reasoning stays implicit unless the user asks for grouping or full report.

### Envelope rules

- **Severity prefixes**: every finding line starts with **`[P0]`**, **`[P1]`**, or **`[P2]`**. Caveats/open questions use **`[P2]`** with downgrade note, or one line starting **`Caveat:`**.
- **Prefix block** (before first finding): at most **one** `Scope:` line + optionally **one** `Out of scope:` line (only if `Scope:` used). The **very next** line must be the first finding. No tables, no summary headings, no scene-setting prose between prefix and findings.
- **Without `Scope:`**: the first host-visible line **must** be the first finding. Do **not** use standalone `Out of scope:` — fold it into the first finding or a `Scope:` line.
- **Verdict**: at most **one line**, **only after** the complete findings list. Optional `test/repro gap` stays one line after verdict or folded into residual-risk.
- **No grouping by lens** unless the user asks.
- **Each finding**: one tight line + optional indented evidence: **`[Pn] path:anchor`** — issue — impact — smallest verification.

### Full report profile (explicit triggers only)

Use **only** when user asks for **`Scope/Lenses/Omitted`**, lens-by-lens sections, **PR/述职叙事**, categorical deliverables, **exhaust every lens**, audit-style report. Vague 「有什么问题」「全面review」**alone stays compact**.

Then: preamble (Scope / Lenses / Omitted), verdict, findings grouped by lens, test/repro gap, external calibration (if used), next move — same rigor, richer packaging.

## Lane contracts

### Spawn-first pairing

For broad/deep/PR-level review, spawn **at least one** parallel read-only reviewer (`fork_context=false`, lane in `reviewer_lanes`; Cursor 可选 `Task` + `subagent_type=deep-reviewer`). Explore lanes **do not count** as review evidence. For breadth/PR/cross-module prompts, prefer **>=2** lanes split by disjoint lens bundles, before main-thread compact synthesis.

**Narrow scope** (single-file, `small_task`, or explicit「不用子代理」): no multi-lane requirement; hosts skip arming `review_required`.

**I7 heterogeneous requirement** (when `ROUTER_RS_HETEROGENEOUS_ADVERSARIAL_REVIEW=1`): at least one spawned reviewer must use a different model family than the primary session. The framework injects a `heterogeneous_review_hint_for_lane()` nudge naming the primary family; the spawn orchestration should select a cross-family model. Same-family self-review findings do **not** satisfy the I7 adversarial contract.

- **Task / subagents**: **omit** `model` (inherit parent session). Fail with `Model not available` → retry without model or fall back to main-thread.

### REVIEW_GATE clearance

**Cursor**: countable reviewer evidence per wave-2 (`start_count>=1`, multiset drained, no compact-alone forgery). `lifecycle_profile: my-light` does **not** hard-block Stop.

**Claude Code**: `PostToolUse` observes `claude_reviewer_lanes` (registry `review_gate.claude_reviewer_lanes`) with `fork_context` parsed as logical `false`. Stop hard-blocks before `independent_reviewer_seen`. `my-light` / `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE=1` disables hard-block.

**Host countable evidence**: the subagent lane (after normalization) must be in `RUNTIME_REGISTRY.json` -> `review_gate.reviewer_lanes`. `explore`, `ci-investigator`, `cursor-guide`, and custom lane names **do not count** on Cursor — even with `fork_context=false`.

Lane outputs must cite **locations** (paths + anchors / symbols).

## Factcheck gate（source-level ground truth）

Deep review workflow（`claude-chain-deep-review`、`deep-review-template`）在 Merge 和 Verify 之间插入独立 **Factcheck 阶段**，专门拦截幻觉 finding：

- **职责边界**：Factcheck agent **只核查事实**（代码是否存在、evidence 是否原文引用、行号是否准确、行为描述是否与代码一致），**不做判断**（是否是 bug、severity 如何）。
- **幻觉分类**：`code_not_exist`（代码不存在）、`evidence_fabricated`（evidence 为捏造/复述）、`wrong_line`（行号偏差）、`behavior_misrepresented`（行为描述有误）、`partial_hallucination`（部分准确部分幻觉）、`none`（全部准确）。
- **拦截规则**：`code_exists=false` 或 `evidence_accurate=false` 的 finding 标记为 hallucinated，**不进入 Verify 阶段**。`behavior_misrepresented` 附带修正描述后可选进入 Verify。
- **独立性**：Factcheck agent 与 Scan agent 必须是不同的 agent 实例（pipeline 自动保证）。Factcheck 不复用 Scan 的上下文，避免循环确认。
- **输出 schema**：`FACTCHECK_VERDICT_SCHEMA`（定义在 `workflow-helpers.js`），包含 `code_exists`、`evidence_accurate`、`line_accurate`、`behavior_accurate`、`hallucination_type`、`actual_code`、`actual_behavior`。
- **Skill 层 spawn**：非 workflow 上下文（如主会话直接 spawn review subagent）可使用 `factcheck-verifier` agent 定义（`.claude/agents/factcheck-verifier.md`），工具限制为 Read + Bash（只读）。

## I7: heterogeneous adversarial review (model-family diversity)

When the environment flag `ROUTER_RS_HETEROGENEOUS_ADVERSARIAL_REVIEW=1` is set **and** the prompt qualifies as broad/deep review (not narrow/single-file), the framework enforces model-family diversity:

- **Primary model family** is detected from `ROUTER_RS_MODEL_FAMILY` (or host-injected `CLAUDE_MODEL` / `OPENAI_MODEL`). At least **one** reviewer subagent must use a **different** model family (e.g., primary=`claude` requires a `gpt`/`gemini`/`llama` reviewer).
- The `heterogeneous_review_hint_for_lane()` nudge is injected into the reviewer prompt automatically by the host hooks (Claude/Cursor/Codex). This hint names the primary model family so the spawn orchestration can select a cross-family reviewer.
- **`reviewer_lanes`** countable evidence: a reviewer lane that satisfies the heterogeneous requirement **and** has `fork_context=false` (or inferred false) counts toward REVIEW_GATE clearance. Same-family self-review does **not** satisfy the I7 requirement.
- In the RFV loop, the `metadata.heterogeneous_adversarial_review` block records `primary_model_family` alongside the config so round-to-round auditing is possible.

**Operator**: set `ROUTER_RS_MODEL_FAMILY=gpt-4o` (or equivalent) to declare the primary session's model family when the host does not inject it automatically.

## External / network research lane

Use when the user allows network/tools or scope touches third-party crates/services or known vulnerability classes.

**In compact mode**: external material appears only as indented bullets under the specific `[P*]` / `Caveat:` line they support, or as plain continuation after the last finding and before the one-line verdict — no standalone section headers, no Markdown tables.

**Full report profile** (or explicit preamble): produce **Claims** with citations (CVE, changelog URL, Advisory ID), **Contradiction sweep**, **Unknowns**, **Retrieval_trace**. Aligns with [`docs/spec.md) section A-B.

## Severity evidence gate

- **P0/P1 requires evidence**: at least one concrete call chain, repro path, checked test gap, or cited external advisory. Without evidence, downgrade to P2 / caveat / open question.
- **No hollow findings**: every finding must include path + symbol/line anchor, user/operational impact, and the smallest verification or missing test.
- **Testing honesty**: if tests were not run, say so once and name the residual risk.
- **Security claims**: state exploitability or blast radius; speculative abuse without a reachable path is a caveat, not a blocker.

## Security audit dimensions

参考 Trail of Bits 安全审计方法论。以下为选中 lens 时的检查点：

- **注入**: 用户输入是否参数化/转义 (SQL/NoSQL/OS/LDAP)
- **认证缺陷**: session 管理、密码存储、MFA 绕过
- **敏感数据暴露**: 日志中 secrets/API keys/tokens、硬编码 credentials、.env 是否在 .gitignore
- **访问控制**: IDOR、权限提升、水平越权
- **安全配置**: 默认密码、debug 模式、CORS 策略、XXE 禁用
- **XSS / 反序列化**: 输出编码、CSP、untrusted data 处理
- **依赖安全**: 已知漏洞、lockfile 完整性、范围版本 vs 锁定版本
- **日志监控**: 安全事件是否被记录

安全发现按 Critical / High / Medium / Low / Info 分级，与代码质量发现分开报告。每个发现包含：位置、描述、影响、修复建议。

## Deliverable shape summary

**Compact (default)**: optional prefix (0-2 lines) -> Findings list (P0->P1->P2->caveats, evidence-gated) -> optional one-line verdict -> optional one-line test/repro gap.

**Full report (explicit only)**: Scope/Lenses/Omitted -> verdict -> findings by lens (P0-P2) -> test/repro gap -> external calibration -> next move.

## CodeGraph 场景

Review lane **只读**；图谱用于定位与 call-chain 证据，不替代 Read/Grep。

> 工具与场景表：见 [`codegraph-scenarios.md`](../shared-references/codegraph-scenarios.md)。
> 何时：宽范围 review 前确认就绪（stale 时在 finding 中加 caveat）；消歧 FQN、核实 path:anchor；P0/P1 需 concrete call chain 时优先于手工 rg；API/行为变更的 blast radius 与测试缺口。

## Integration / boundaries

- Repo closeout Git operations: `$gitx` owns staging history; reuse this lane for diff critique only.
- Screenshots/UI decks: `$visual-review` complements but does not replace correctness/security lanes.
- Paper/manuscript judgment or PR comment triage: prefer narrower owners (`paper-workbench`, `gh-address-comments`) when routing applies.
- **Framework-repo optional evidence** (this harness repo only): local checklists or `router-rs framework maint` audit commands as read-only evidence — never as a dependency for other codebases.
