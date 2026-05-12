---
name: code-review-deep
description: |
  Deep adversarial-style code review (review-only verdict first). Default depth for short prompts like "review" / 代码审查 when no narrower owner applies.
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
  - 深度 code review
  - 深度代码审查
  - 严苛代码评审
  - security code review
  - security-focused code review
  - threat model review
  - adversarial code review
  - 只允许审不改
  - review-only 代码审查
  - CVE 审查
  - dependency audit PR
  - supply chain review
  - 供应链安全
metadata:
  version: "1.1.0"
  platforms: [codex, cursor]
  tags: [code-review, security, correctness, delegation, adversarial-review]
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

Judgment-focused review for code and change sets **without** rewriting by default. Portable across repositories: do **not** assume framework-specific files or audit commands exist unless the workspace is this skill/harness repo and the user’s scope includes it.

## Default posture

- Assume a **hostile but fair** reviewer: maximize plausible failure under real abuse, regressions,
  flaky ops, closest prior API expectations, dependency churn, or incomplete tests.
- **Verdict-first**: summarize overall risk and top blockers before long findings lists.
- **Lens catalog, not a fixed runway**: choose lenses from [`references/review-dimensions.md`](references/review-dimensions.md). **Do not** treat every review as “must run every row.” **Do** systematically exhaust findings **within each lens you selected**.
- When the user explicitly asks to **cover all dimensions** / **exhaust every lens** / **全维度**, apply the full catalog (still grouped by lens, still evidence-gated for severities).

## Lane selection protocol (start of output)

Begin with a short block (about 3–8 lines):

- **Scope**: what is being reviewed (paths, PR slice, subsystem).
- **Lenses**: which dimensions from the catalog you are using and why they fit.
- **Omitted**: important dimensions you are **not** using this round and one-line reasons (token budget, out of scope, user constraint, or needs different owner).

Then deliver the verdict and findings **grouped by chosen lenses**.

## Lane contracts

For broad/deep/PR-level review, start at least one read-only reviewer subagent with **fork_context=false** before the main-thread synthesis. Narrow single-file review may stay local unless the user asks for deep/adversarial coverage. When additional subagents are admitted, keep them read-only and **artifact-disjoint**. Split subagents by **your selected lenses**, not by a hard-coded global list. Do **not** have multiple lanes silently edit shared files mid-review.

Lane outputs must cite **locations** (paths + anchors / symbols where possible).

**Framework-repo optional evidence** (only when this workspace is this harness/skill framework repository and scope touches it): you may cite local checklists or `router-rs framework maint` audit-style commands as **read-only** evidence—never as a dependency for reviews of other codebases.

## External / network research lane (optional but recommended)

Use only when the user allows network/tools or the scope touches third-party crates/services or known vulnerability classes:

- Produce **Claims** backed by citations (changelog URL, GitHub Advisory ID, CVE, release notes DOI/issue).
- **Contradiction sweep**: cite evidence that contradicts or limits each high-confidence Claim.
- **Unknowns**: what still cannot be asserted from reachable evidence alone.
- **Retrieval_trace** (minimal): queries / sources scanned, inclusion/exclusion heuristic, stale assumptions rejected.

Structured output expectations align with
[`docs/references/rfv-loop/reasoning-depth-contract.md`](../../docs/references/rfv-loop/reasoning-depth-contract.md) §A–B (same headings whenever you mark work as “deep external,” even outside an RFV ledger).

## Severity evidence gate

- **P0/P1 requires evidence**: include at least one of a concrete call chain, a repro path, a checked test gap, or a cited external advisory/source. Without that, downgrade to P2, caveat, or open question.
- **No hollow findings**: every finding must include path + symbol/line anchor, user or operational impact, and the smallest verification or missing test that would confirm it.
- **Testing honesty**: if tests were not run, say so in the review and name the residual risk.
- **Security claims**: state exploitability or blast radius; speculative abuse without a reachable path is a caveat/open question, not a blocker.

## Deliverable shape (default)

0. **Scope / Lenses / Omitted** (see above).
1. `verdict`: one line risk posture (`ship with caveats / revise before merge / blocked`).
2. Findings grouped by **applied lens** (not a single undifferentiated list); include **P0–P2** within each group where relevant.
3. `test / repro gap`: smallest missing command or scenario that would catch the top issues.
4. `external calibration` (if used): link-backed claims + contradiction sweep bullets.
5. `next move`: review-only handoff (what an implementer lane should change), not silent patches.

## Integration / boundaries

- If the task is repo closeout Git operations, `$gitx` still owns staging history; reuse this lane for substantive diff critique only.
- If the artifact is screenshots or rendered UI decks, `$visual-review` complements but does not replace correctness/security lanes.
- If the user needs **paper/manuscript** judgment or **GitHub PR comment triage** as the primary task, prefer the narrower owners (`paper-workbench`, `gh-address-comments`, etc.) when routing applies.
