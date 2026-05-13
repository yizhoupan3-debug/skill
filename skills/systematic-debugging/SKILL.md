---
name: systematic-debugging
description: |
  Explicit diagnostic lane for root-cause investigations that need reusable reproduction,
  evidence capture, flake isolation, incident triage, or failure-mode playbooks. Runtime owns
  the generic "unknown cause -> gather evidence before fixing" rule; load this skill only for
  explicit diagnostic work or precise failure-mode requests.
short_description: Investigate unknown failures before fixing
trigger_hints:
  - systematic-debugging
  - root-cause analysis
  - flake isolation
  - incident triage
  - diagnostic playbook
metadata:
  version: "2.5.0"
  platforms: [supported]
  tags:
    - debugging
    - root-cause-analysis
    - reproduction
    - hypothesis-testing
    - incident-triage
framework_roles:
  - gate
  - detector
framework_phase: 2
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false
risk: low
source: local
routing_layer: L0
routing_owner: gate
routing_gate: evidence
routing_priority: P1
session_start: n/a
user-invocable: false
disable-model-invocation: true
allowed_tools:
  - shell
  - browser
  - python
approval_required_tools:
  - gui automation
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - runtime_evidence.md
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once

---

# Systematic Debugging

This skill owns **investigation before repair**. When root cause is still
unknown, do not jump straight to implementation.

## When to use

- A bug, failing test, flaky behavior, build failure, or prod issue is being investigated
- The failure mechanism is still unknown
- Multiple blind fixes have already failed
- The user explicitly wants root-cause analysis before patching
- User describes symptoms without providing a root cause ("为什么不工作", "报错了", "崩了", "不对", "失败了")
- User says "帮我修…" without attaching a stack trace or identified fault line
- Any request where the cause is inferred but not yet confirmed by evidence

## Do not gate (豁免条件)

- Root cause is already confirmed by a stack trace pointing to a specific line
- User explicitly says "我知道是 X 问题，帮我修" with X specified
- Pure feature request with no failure involved

## Do not use

- Root cause is already proven and the user only wants the fix
- The task is pure feature work (no failure involved)
- The task is project-wide error-handling design -> use the current architecture or implementation context
- The problem is clearly frontend-runtime-specific and already belongs to a selected frontend implementation owner
- Covered by the **豁免条件** above

## Primary operating principle

This gate should behave like an **investigation controller**:

1. gather evidence before proposing fixes
2. keep hypotheses small and falsifiable
3. keep the main thread to observed signals, root-cause progress, and next experiment
4. if multiple independent evidence surfaces appear, preserve them as bounded investigation slices
5. if runtime policy blocks spawning, keep the same investigation matrix in local-supervisor mode

## Main-thread Compression

The main thread should contain only:

- symptom summary
- observed evidence
- current hypothesis
- disconfirmed path if any
- next experiment or reroute

## Core workflow

1. Reproduce the problem, or say exactly why reproduction is blocked.
2. Gather evidence from the **real failure surface** (logs, stdout, stderr, stack trace). Never theorize without tools.
3. Trace the failure upstream — do not stop at the outermost symptom.
4. State one hypothesis at a time. Mark as inferred vs. observed.
5. Test minimally: change one variable, compare before/after.
6. Only after confirming root cause: fix inline or hand off to the right domain owner.

Evidence before hypothesis. Do not propose a fix until a real command, log,
trace, screenshot, or source-gate result has returned concrete output. Detailed
tool matrices and output templates live in
[`references/hypothesis-checklist.md`](references/hypothesis-checklist.md).

## Hard constraints

- **No blind multi-fix patching.** Change one thing, verify, then proceed.
- **No symptom suppression** presented as root-cause resolution.
- **No passive finish**: never say "should work now" without showing stdout/stderr proof.
- **No context-begging**: run `grep`, `cat`, or `run_command` before asking the user.
- In this repository, use [`RTK.md`](/Users/joe/Documents/skill/RTK.md) only when the command is high-volume and compact output still preserves the needed evidence; prefer raw output when the exact failing line matters.
- If reproduction is unconfirmed, say so explicitly — never assume it can be reproduced.
- If three fix attempts fail, step back and challenge the premise or architecture.
- **Anti-laziness checkpoint**: before handing off to a domain owner, the debugging record must show: symptom + evidence source + observed (not inferred) root cause.

## Framework note

Emit a finding-like debugging record before handing execution to a fixer,
TDD workflow, or domain owner.

## References

- [references/hypothesis-checklist.md](references/hypothesis-checklist.md) — Hypothesis formation rules, testing matrix, failure routing table, anti-spinning protocol
