---
name: code-review
description: |
  Review code with structured findings and optional quality scoring.
  Use when the user asks for code review, PR review, code-quality scoring, or an iterative
  review→fix→re-score loop. Also use when the request is phrased as 代码 review、代码审核、
  实现质量 review、回归风险检查, or findings / 严重程度排序. Best for structured review, not coding-standard enforcement,
  security audit, or architecture review.
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - code-review
    - pull-request
    - review-checklist
    - score
    - overlay
framework_roles:
  - detector
  - planner
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
risk: low
source: local
routing_layer: L2
routing_owner: overlay
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - code review
  - 代码 review
  - 代码审核
  - PR review
  - 代码审查
  - 实现质量
  - 回归风险
  - findings
  - 严重程度
  - code-quality scoring
  - review→fix→re-score
  - an iterative review→fix→re-score loop
  - pull request
  - review checklist
  - score
  - overlay
allowed_tools:
  - shell
  - git
  - python
approval_required_tools: []

---

# code-review

This skill owns **structured implementation review**: qualitative findings,
prioritized review comments, and optional scoring.

## When to use

- The user asks to review code, a PR, or a patch set
- The user wants structured feedback instead of ad-hoc comments
- The user wants a score or iterative review→fix→re-score loop
- The user asks for findings ordered by severity, bug risk, or regression risk
- Review rigor is needed on top of a domain owner skill

## Do not use

- Coding-standard enforcement → use `$coding-standards`
- Security-focused audit → use `$security-audit`
- Architecture-level review → use `$architect-review`
- PR comment handling workflow → use `$gh-address-comments`

## Review modes

1. **Checklist review** — default
2. **Scorecard review** — when the user explicitly wants scoring
3. **Iterative convergence** — when the user wants repeated review/fix loops

## Core workflow

1. Confirm scope and intent.
2. Review for:
   - correctness
   - robustness / edge cases
   - readability
   - testability
   - performance
   - security surface
   - documentation impact
3. Classify findings as must-fix / should-fix / nit.
4. Add score only if requested or materially useful.
5. Cite concrete file/line evidence.

## Hard constraints

- Do not say “looks good” without checking the change systematically.
- Do not mix nits with blockers.
- Do not score without evidence.
- Acknowledge good patterns, not only defects.
