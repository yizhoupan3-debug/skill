---
name: code-review-deep
description: |
  Deep adversarial-style code review (review-only verdict first). Use for whole-PR/repo-slice/security/deps
  review when the bar is hostile-but-fair: correctness, abuse cases, API/compat, observability,
  dependency/supply-chain, and reproducible reasoning. Prefer parallel disjoint read-only subagent lanes plus
  an external research lane when the user allows tools/network. Does not silently rewrite implementation unless
  the user explicitly exits review-only posture.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: preferred
user-invocable: true
disable-model-invocation: false
trigger_hints:
  - $code-review-deep
  - code-review-deep
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
  version: "1.0.0"
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

Judgment-focused review lane for code changes **without** rewriting by default.

## Default posture

- Assume a **hostile but fair** reviewer: maximize plausible failure under real abuse, regressions,
  flaky ops, closest prior API expectations, dependency churn, or incomplete tests.
- **Verdict-first**: summarize overall risk and top blockers before long findings lists.
- **Parallel read-only lanes** over monologue: schedule disjoint subagents for distinct lenses once scope is clear.
  See [`references/review-dimensions.md`](references/review-dimensions.md).

## Parallel lane contracts

Run these as **fork_context=false**, read-only, **artifact-disjoint**. Do **not** have multiple lanes silently edit shared files mid-review.

Suggested default split (adapt to repo shape):

| Lane | Focus |
| --- | --- |
| correctness | logic, concurrency, resilience, retries, resource lifetime |
| security | authZ/session, injections, deserialization, FFI/`unsafe`, secret handling |
| api_abi_compat | public surface, ABI-stable contracts, versioning, rollout/migration traps |
| deps_supply_chain | semver/changelog/advisories/CVE reachability; pins and transitive exposure |
| observability | logs/metrics/traces, on-call cues, flaky tests masking defects |

Lane outputs must cite **locations** (paths + anchors / symbols where possible).

## External / network research lane (optional but recommended)

When scope touches third-party crates/services or known vulnerability classes:

- Produce **Claims** backed by citations (changelog URL, GitHub Advisory ID, CVE, release notes DOI/issue).
- **Contradiction sweep**: cite evidence that contradicts or limits each high-confidence Claim.
- **Unknowns**: what still cannot be asserted from reachable evidence alone.
- **Retrieval_trace** (minimal): queries / sources scanned, inclusion/exclusion heuristic, stale assumptions rejected.

Structured output expectations align with
[`docs/references/rfv-loop/reasoning-depth-contract.md`](../../docs/references/rfv-loop/reasoning-depth-contract.md) §A–B (same headings whenever you mark work as “deep external,” even outside an RFV ledger).

## Deliverable shape (default)

1. `verdict`: one line risk posture (`ship with caveats / revise before merge / blocked`).
2. `P0–P2 blockers`: cite file/symbol; state exploitability or blast radius when security.
3. `test / repro gap`: smallest missing command or scenario that would catch the top issues.
4. `external calibration` (if used): link-backed claims + contradiction sweep bullets.
5. `next move`: review-only handoff (what an implementer lane should change), not silent patches.

## Integration / boundaries

- If the task is repo closeout Git operations, `$gitx` still owns staging history; reuse this lane for substantive diff critique only.
- If the artifact is screenshots or rendered UI decks, `$visual-review` complements but does not replace correctness/security lanes.
