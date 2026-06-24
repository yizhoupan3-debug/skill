---
allowed_tools:
- shell
- browser
- python
approval_required_tools:
- gui automation
description: 'Root-cause investigation lane: hypothesis-driven debugging, reproduction capture, flake isolation, incident triage.'
metadata:
  platforms:
  - supported
  tags:
  - debugging
  - root-cause-analysis
  - reproduction
  - hypothesis-testing
  - incident-triage
  version: '2.5.0'
name: systematic-debugging
network_access: conditional
risk: low
routing_gate: evidence
routing_layer: L0
routing_owner: gate
routing_priority: P2
session_start: n/a
short_description: Investigate unknown failures before fixing
source: local
trigger_hints:
- blind fix
- debug
- flake
- incident triage
- root cause
- systematic-debugging
- 为什么失败
- 为什么报错
- 偶发失败
- 失败了
- 崩了
- 无法复现
- 根因分析
- 调试
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
- If reproduction is unconfirmed, say so explicitly — never assume it can be reproduced.
- If three fix attempts fail, step back and challenge the premise or architecture.
- **Anti-laziness checkpoint**: before handing off to a domain owner, the debugging record must show: symptom + evidence source + observed (not inferred) root cause.

## Framework note

Emit a finding-like debugging record before handing execution to a fixer,
TDD workflow, or domain owner.

## References

- [references/hypothesis-checklist.md](references/hypothesis-checklist.md) — Hypothesis formation rules, testing matrix, failure routing table, anti-spinning protocol
