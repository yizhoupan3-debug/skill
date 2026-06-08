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
  version: "2.0.0"
  platforms: [supported]
  tags: [code-review, security, correctness, delegation, adversarial-review, codegraph]
allowed_tools:
  - mcp__mcp-codegraph__codegraph_search
  - mcp__mcp-codegraph__codegraph_callers
  - mcp__mcp-codegraph__codegraph_callees
  - mcp__mcp-codegraph__codegraph_impact
  - mcp__mcp-codegraph__codegraph_node
  - mcp__mcp-codegraph__codegraph_status
framework_roles:
  - detector
  - planner
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: true
risk: medium
source: local
---

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

- **Task / subagents**: **omit** `model` (inherit parent session). Fail with `Model not available` → retry without model or fall back to main-thread.

### REVIEW_GATE clearance

**单一清门规则（Claude canonical · 2026-06）**

| 条件 | 行为 |
|------|------|
| Armed | review 信号且非 My 执行区入口，且未 narrow-skip / override |
| 清门 | `independent_reviewer_seen`（registry `reviewer_lanes` + explicit `fork_context=false` via PostTool/subagent）**或** `review_override` |
| Stop 出站 | **advisory-only**——hook 可 nudge，**不**硬拦 Stop；`my-light` / `ROUTER_RS_REVIEW_GATE_DISABLE` suppress nudge 链 |
| 非清门条件 | phase≥3、compact-only bump、Cursor multiset unsettled、Codex `subagent_start_count` — **遥测/提示 only** |

**Hook hosts**：Claude Code 为参考实现；Cursor/Codex 仅 transport 差异（nudge 文案前缀、`rg_clear`/`reject_reason` 粘贴面、multiset/`subagent_start_count` 遥测）。实现：`core-policy::review_gate_satisfied`；见 [`docs/host_adapter_contract.md`](../../docs/host_adapter_contract.md) §0.1。

**MCP hosts（Antigravity / OpenCode）**：review 缺口经 MCP **advisory**（`ADVISORY`）；`closeout_gate` / `goal_state_manage complete` 可在证据未满足时 **hard-block**（与 review 分层）。见 [`host_adapter_contract.md`](../../docs/host_adapter_contract.md) §0.1、`ROUTER_RS_CLOSEOUT_ENFORCEMENT`。

**Host countable evidence**: subagent lane（normalize 后）须在 `RUNTIME_REGISTRY.json` → `review_gate.reviewer_lanes`。`explore`、`ci-investigator`、`cursor-guide` 与自定义 lane **不计**——即使 `fork_context=false`。

Lane outputs must cite **locations** (paths + anchors / symbols).

## CodeGraph MCP（可选 · CG-5）

宿主已注册独立 `mcp-codegraph` 进程（`configs/framework/RUNTIME_REGISTRY.json` → `managed_mcp_servers.mcp-codegraph`）。**只读**核对传播范围与调用链，再写 findings；review-only  posture 不变。

| 审稿场景 | 何时用 | 首选工具 |
|----------|--------|----------|
| 变更集 / PR | findings 前 blast radius | `codegraph_impact` |
| 单点缺陷 | 符号定义与引用锚点 | `codegraph_node` |
| 安全 lens | 可达调用链、横向传播 | `codegraph_callers` / `codegraph_callees` |
| 广度审稿 | 模块入口与 owner | `codegraph_search` |

**Fallback**：MCP 不可用时 reviewer lane 用 `Grep` / `Read`；在首条 finding 或 `Caveat:` 注明索引未校验。**禁止**因 codegraph 失败转入 implement / 改代码。

## External / network research lane

Use when the user allows network/tools or scope touches third-party crates/services or known vulnerability classes.

**In compact mode**: external material appears only as indented bullets under the specific `[P*]` / `Caveat:` line they support, or as plain continuation after the last finding and before the one-line verdict — no standalone section headers, no Markdown tables.

**Full report profile** (or explicit preamble): produce **Claims** with citations (CVE, changelog URL, Advisory ID), **Contradiction sweep**, **Unknowns**, **Retrieval_trace**. Aligns with [`docs/references/rfv-loop/reasoning-depth-contract.md`](../../docs/references/rfv-loop/reasoning-depth-contract.md) section A-B.

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

## Integration / boundaries

- Repo closeout Git operations: `$gitx` owns staging history; reuse this lane for diff critique only.
- Screenshots/UI decks: `$visual-review` complements but does not replace correctness/security lanes.
- Paper/manuscript judgment or PR comment triage: prefer narrower owners (`paper-workbench`, `gh-address-comments`) when routing applies.
- **Framework-repo optional evidence** (this harness repo only): local checklists or `router-rs framework maint` audit commands as read-only evidence — never as a dependency for other codebases.
