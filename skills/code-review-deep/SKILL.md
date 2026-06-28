---

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
- mcp__mcp-codegraph__codegraph_dead_code
- mcp__mcp-codegraph__codegraph_goto_definition
description: Deep adversarial-style code review (review-only). Default visible output is a compact, severity-sorted findings list; narrative sections only when explicitly requested. Model selects lenses from an ex
metadata:
  platforms:
  - supported
  tags:
  - code-review
  - security
  - correctness
  - delegation
  - adversarial-review
  version: '2.2.0'
name: code-review-deep
scene: code_review
risk: medium
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P1
session_start: preferred
source: project
trigger_hints:
- $code-review-deep
- adversarial code review
- code review
- code-review-deep
- deep code review
- review
- review-only 代码审查
- security code review
- security-focused code review
- threat model review
- 严苛代码评审
- 代码审查
- 只允许审不改
- 帮我 review
- 深度 code review
- 深度代码审查
---
## Quick Ref
- **Purpose**: 深度对抗式代码审查（review-only），默认输出 severity-sorted findings 紧凑列表
- **Key Rules**: 默认只审不改；hostile-but-fair 立场；pipeline 五阶段（Scope 评估 → 静态分析预扫描 → Review lanes → Factcheck → 对抗性验证）；P0/P1 须有 evidence；lens 可扩展目录选型；broad review 可 spawn 多 reviewer lane；对抗验证并发 ≤3
- **Trigger**: "review"、"代码审查"、"帮我 review"、"deep code review"、"$code-review-deep"
<!-- full content below; load on demand -->

# Code review (deep owner)

Judgment-focused review for code and change sets **without** rewriting by default. Portable across repositories: do **not** assume framework-specific files or audit commands exist unless the workspace is this skill/harness repo.

## Default posture

- **Findings-only by default (hard stop)**: On a review request, **do not** edit files, add tests, run fix commits, open PRs, or continue into implement / gitx / loop unless the user **explicitly** exits review-only (e.g. "fix these findings", "implement", "merge", "commit"). End with findings (+ optional one-line verdict), not execution.
- Assume a **hostile but fair** reviewer: maximize plausible failure under real abuse, regressions, flaky ops, closest prior API expectations, dependency churn, or incomplete tests.
- **Compact default output = less prose, not shallower reasoning.** Analysis standard is unchanged: choose lenses internally, exhaust findings **within each selected lens**, apply the severity evidence gate.
- **Lens catalog, not a fixed runway**: choose lenses from [`references/review-dimensions.md`](references/review-dimensions.md). When the user asks to **cover all dimensions** / **全维度**, apply the full catalog **and** use the **full report profile**.

## Review pipeline (phases)

Deep review runs these phases in order. Earlier phases inform later ones; the output is the consolidated result of all phases.

### Phase 1: Scope assessment & adaptive depth

Diff scope determines review depth. Assess before spawning any reviewer:

| Dimension | Narrow | Moderate | Broad |
|-----------|--------|----------|-------|
| **Scope** | Single-file，<50 lines changed, `small_task` | 2-5 files，同模块内跨文件 | PR-level，>5 files，跨模块/跨 crate |
| **Lenses** | 1-2 core，surface scan | 3-4 core lenses | All core + optional |
| **Factcheck** | Recommended | Required | Required |
| **Adversarial verify** | Optional | Recommended | Required |

**Depth escalation** (any trigger → escalate one tier, e.g. narrow→moderate, moderate→broad):
- `unsafe` blocks / FFI / raw pointer manipulation
- Network I/O / file system / process spawning
- Public API signature changes or trait additions
- Dependency additions or version bumps
- Security-sensitive domains (auth, crypto, secrets, access control)
- Configuration / credential handling changes

Also consider **diff entropy**: a small diff touching security infrastructure or `unsafe` blocks is _not_ narrow.

### Phase 2: Static analysis pre-scan

Before review lanes run, gather objective signal channels. Results become context for reviewer lanes — not a replacement for judgment.

- **`cargo check` / `cargo clippy`** (or language equivalent): emit warnings as objective signals. Each clippy warning in a diff-touched area → reviewer increases scrutiny there. Warnings outside the diff → note as `Caveat:` for the project's tech debt, not a finding against the diff.
- **`cargo deny advisories`** (or `cargo audit`): flag known-vulnerability dependencies. Results feed into the Deps/Supply-chain lens.
- **`cargo miri`** (when diff contains `unsafe` blocks): detect undefined behavior. If miri passes, unsafe findings from review require stronger evidence.
- **Non-Rust projects**: use analogous tooling (`pylint`/`mypy` for Python, `eslint` for JS/TS, `gosec` for Go, etc.).

Static analysis output is **advisory**, not authoritative. A clean clippy run does not mean the diff is correct; a clippy warning does not always indicate a bug. Reviewer lanes make the final call.

### Phase 3: Regression-aware git context

Before review lanes, inspect git history for churn signals:

- For each modified file, check `git log --oneline -10 <file>` — frequent recent edits suggest unstable code.
- For suspicious regions, `git blame <file> -L <start>,<end>` to see if the same lines were recently modified for related reasons.
- If the diff reverts or bypasses a prior fix (detect via `git log -S <symbol> -- <file>`), flag as P0/P1 with the prior fix commit cited.
- For broad reviews, `git diff --stat <base>...HEAD` establishes total change surface area.

Regression signals are **context** for reviewer lanes, not standalone findings (unless a confirmed revert of a prior fix, which is P0).

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

For broad/deep/PR-level review, LLM 根据 scope 自行决定串行或并行 spawn reviewer lane（`fork_context=false`, lane in `reviewer_lanes`; Cursor 可选 `Task` + `subagent_type=deep-reviewer`）。Explore lanes **do not count** as review evidence。跨 domain 的宽范围时优先并行减少延迟，同区域多透镜需避免冲突时串行。LLM 自主权衡。

**Narrow scope** (single-file, `small_task`, or explicit「不用子代理」): no multi-lane requirement; hosts skip arming `review_required`.

**I7 heterogeneous requirement**: at least one spawned reviewer must use a different model family than the primary session. The orchestrator injects a `heterogeneous_review_hint_for_lane()` nudge naming the primary family; the spawn orchestration should select a cross-family model. Same-family self-review findings do **not** satisfy the I7 adversarial contract.

- **Task / subagents**: **omit** `model` (inherit parent session). Fail with `Model not available` → retry without model or fall back to main-thread.

### REVIEW_GATE clearance

Countable reviewer evidence判定：subagent lane 的 review 产出须包含具体**位置引用**（路径 + 锚点/符号）。`explore`、`ci-investigator` 及自定义 lane 不计入 evidence，即使 `fork_context=false`。

## Factcheck gate（source-level ground truth）

Deep review 在 Merge 和 Verify 之间插入独立 **Factcheck 阶段**，专门拦截幻觉 finding：

- **职责边界**：Factcheck agent **只核查事实**（代码是否存在、evidence 是否原文引用、行号是否准确、行为描述是否与代码一致），**不做判断**（是否是 bug、severity 如何）。
- **幻觉分类**：`code_not_exist`（代码不存在）、`evidence_fabricated`（evidence 为捏造/复述）、`wrong_line`（行号偏差）、`behavior_misrepresented`（行为描述有误）、`partial_hallucination`（部分准确部分幻觉）、`none`（全部准确）。
- **拦截规则**：`code_exists=false` 或 `evidence_accurate=false` 的 finding 标记为 hallucinated，**不进入 Verify 阶段**。`behavior_misrepresented` 附带修正描述后可选进入 Verify。
- **独立性**：Factcheck agent 与 Scan agent 必须是不同的 agent 实例（pipeline 自动保证）。Factcheck 不复用 Scan 的上下文，避免循环确认。
- **Schema**：`FACTCHECK_VERDICT_SCHEMA`（`{ is_accurate: bool, errors: [{ quote, correction, severity }], reasoning: string }`）。
- **Skill 层 spawn**：主会话直接 spawn review subagent 时可使用 `factcheck-verifier` agent，工具限制为 Read + Bash（只读）。Agent 定义在 skill 内：
  ```json
  { "name": "factcheck-verifier", "tools": ["Read", "Bash"], "prompt": "仅核查事实——代码是否存在、evidence 是否原文引用、行号是否准确、行为描述是否与代码一致。不做判断（是否是 bug、severity 如何）。" }
  ```

## Adversarial verification

Deep review 在 Factcheck 之后插入 **对抗性验证（Adversarial verification）**阶段，不可跳过。目标：对整合后的 findings 进行敌意反驳，只有 survived 的 finding 进入最终输出。

### 流程

```
Review lanes → Factcheck → [Findings consolidation] → Adversarial verification → Output
```

### 整合（Consolidation）

所有 reviewer lane 产出的 finding（已通过 Factcheck）先**整合去重**：
- 同一位置同一类发现 → 合并为一条 finding，保留最严重 severity
- 跨 lane 矛盾发现（如 Correctness 说安全、Security 说危险）→ 标记为争议项
- 去重后形成 **FindingsSet**（每个 finding 含：severity、位置、根因、影响、证据链、来自哪些 lane）

### 对抗验证（Adversarial verification）

整合后的 FindingsSet 逐条 spawn 验证 agent，**目标是反驳**——尝试证明该 finding 不可触发、不可重现、有缓解因素、或基于错误前提。

- **每轮最多 3 个并发验证 agent**（硬约束：`max_concurrent=3`）。若 FindingsSet 超过 3 条，分 batch 执行，每 batch ≤3，全部完成后进入下一 batch。
- 验证 agent 使用**不同模型族**（利用 I7 多样性），避免同族自洽确认。
- 验证 agent 只读（工具限制 Read + Bash + CodeGraph query），不允许修改代码。
- 验证 agent 的 prompt 模板：
  > 「你的任务：**反驳**以下代码审查 finding。尝试证明它不可触发、有缓解因素、或基于对代码的误读。如果无法反驳，说明为什么维持原判。给出结论：REFUTE、DOWNGRADE（降到P2/caveat）或 SUSTAIN。」
- **Schema**：`{ verdict: "REFUTE" | "DOWNGRADE" | "SUSTAIN", reasoning: string, counterevidence: [{ location, description }] }`

### 结果处置

| Verdict | 处置 |
|---------|------|
| `SUSTAIN` (n个验证 agent 中 ≥多数) | 保留原 severity |
| `DOWNGRADE` (n个验证 agent 中 ≥多数) | P0/P1 → 降为 P2/caveat；P2 → 移除或转为 Open question |
| `REFUTE` (n个验证 agent 中 ≥多数) | **从输出中移除**，不进最终 finding 列表 |

**争议处理**：3 agent 各执一词（无多数）→ 保留 finding 但标注 `[adversarial:split]`，用户自行裁决。

### 适用范围

- **Broad/moderate scope**：强制执行。验证 agent 可 spawn，数量受 max_concurrent=3 约束。
- **Narrow scope**：推荐但不强制。如果执行，同样受 max_concurrent=3 约束。
- **用户显式要求 /fast / 快速 review**：可跳过（在发现列表末尾标注 `[adversarial:skipped]`）。

## CodeGraph 增强分析

在代码审查中，以下场景使用 codegraph 获取深层上下文：

- **死代码审查**：调 `codegraph_dead_code[min_lines=5]` 得到候选列表；对候选调 `codegraph_callers` 验证是否真为 orphan。
- **数据流追溯**：对 diff 中可疑符号，调 `codegraph_callers[depth=8]` 完整追溯上下游调用链。
- **PR 影响评估**：PR 删除公共函数/接口时，调 `codegraph_impact[depth=3]` 评估下游破坏。
- **符号定位**：diff 中符号名不在当前文件时，调 `codegraph_search` 定位定义位置。

> 详细场景与参数示例见 [`codegraph-scenarios.md`](../shared-references/codegraph-scenarios.md)。

## I7: heterogeneous adversarial review (model-family diversity)

When the prompt qualifies as broad/deep review (not narrow/single-file), the orchestrator enforces model-family diversity:

- **Primary model family** is detected from host-injected `CLAUDE_MODEL` / `OPENAI_MODEL`. At least **one** reviewer subagent must use a **different** model family (e.g., primary=`claude` requires a `gpt`/`gemini`/`llama` reviewer).
- The `heterogeneous_review_hint_for_lane()` nudge is injected into the reviewer prompt. This hint names the primary model family so the spawn orchestration can select a cross-family reviewer.
- A reviewer lane that satisfies the heterogeneous requirement counts toward REVIEW_GATE clearance. Same-family self-review does **not** satisfy the I7 requirement.
- In multi-round review sessions, the `metadata.heterogeneous_adversarial_review` block records `primary_model_family` alongside the config so round-to-round auditing is possible.

**Operator**: set `primary_model_family=gpt-4o` (or equivalent) to declare the primary session's model family when the host does not inject it automatically.

## External / network research lane

Use when the user allows network/tools or scope touches third-party crates/services or known vulnerability classes.

**In compact mode**: external material appears only as indented bullets under the specific `[P*]` / `Caveat:` line they support, or as plain continuation after the last finding and before the one-line verdict — no standalone section headers, no Markdown tables.

**Full report profile** (or explicit preamble): produce **Claims** with citations (CVE, changelog URL, Advisory ID), **Contradiction sweep**, **Unknowns**, **Retrieval_trace**. Aligns with [docs/architecture.md](../../docs/routing/architecture.md) section A-B.

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
