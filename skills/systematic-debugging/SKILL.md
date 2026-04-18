---
name: systematic-debugging
description: |
  Gate: investigate bugs and failures at 每轮对话开始 / first-turn / conversation start when root
  cause is still unknown. Use for reproduction, evidence gathering, and root-cause isolation before
  fixing. Triggers: 为什么不工作、报错了、崩了、不对、失败了、帮我修（无 stack trace）、程序挂了、
  flaky test、build failure、prod issue. Do not use when root cause is already proven.
short_description: Investigate unknown failures before fixing
trigger_hints:
  - 为什么不工作
  - 帮我修
  - root cause
  - flaky test
  - prod issue
metadata:
  version: "2.5.0"
  platforms: [codex, antigravity]
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
session_start: required
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
- The task is project-wide error-handling design → use `$error-handling-patterns`
- The problem is clearly frontend-runtime-specific and already belongs to `$frontend-debugging`
- Covered by the **豁免条件** above

## Primary operating principle

This gate should behave like an **investigation controller**:

1. gather evidence before proposing fixes
2. keep hypotheses small and falsifiable
3. keep the main thread to observed signals, root-cause progress, and next experiment
4. if multiple independent evidence surfaces appear, preserve them as bounded investigation slices
5. if runtime policy blocks spawning, keep the same investigation matrix in local-supervisor mode

## Main-thread compression contract

The main thread should contain only:

- symptom summary
- observed evidence
- current hypothesis
- disconfirmed path if any
- next experiment or reroute

## Runtime-policy adaptation

If multiple non-blocking evidence slices can run independently and runtime policy permits:

- consult [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md) for bounded evidence collection

If runtime policy does **not** permit spawning:

- keep the same evidence slices in local-supervisor mode
- run them sequentially without abandoning the investigation structure

## Core workflow

1. Reproduce the problem, or say exactly why reproduction is blocked.
2. Gather evidence from the **real failure surface** (logs, stdout, stderr, stack trace). Never theorize without tools.
3. Trace the failure upstream — do not stop at the outermost symptom.
4. State one hypothesis at a time. Mark as inferred vs. observed.
5. Test minimally: change one variable, compare before/after.
6. Only after confirming root cause: fix inline or hand off to the right domain owner.

## Tool Selection Matrix

During investigation, choose the right evidence-gathering tool:

| Failure Surface | Primary Tool | Key Action |
|---|---|---|
| Crash / traceback | `run_command` | `cat log`, `grep -r error .` |
| Build failure | `run_command` | `npm run build 2>&1`, `cargo build 2>&1` |
| Test failure | `run_command` | `pytest -x -v`, `npm test -- --verbose` |
| Network / API | `mcp_browser-mcp_browser_get_network` | `sinceSeconds=30, resourceTypes=["fetch","xhr"]` |
| Frontend symptom | `mcp_browser-mcp_browser_screenshot` + `browser_get_state` | Visual evidence first |
| File state / config | `view_file`, `grep_search` | Inspect actual file contents |
| Sentry event | `$sentry` gate | Structured event intake, then route here |

**Evidence before hypothesis.** Do not propose a fix until one of the above tools has returned concrete output.

## Output defaults

```markdown
## Debugging Summary
- Symptom: ...
- Reproduction: confirmed / partial / blocked

## Evidence
- Source: [logs / Sentry / DevTools / manual repro]
- ...

## Likely Root Cause
- ...

## Next Step
- Route to: [domain skill] / [fix inline]
```

### Sentry evidence intake (when input comes from `$sentry`)

```markdown
## Evidence (from Sentry)
- Event ID: ...
- Exception type: ...
- Stack frame at fault: [file:line]
- Breadcrumbs (last 3): ...
- Regression? [yes / no / unknown]
```

## Hard constraints

- **No blind multi-fix patching.** Change one thing, verify, then proceed.
- **No symptom suppression** presented as root-cause resolution.
- **No passive finish**: never say "should work now" without showing stdout/stderr proof.
- **No context-begging**: run `grep`, `cat`, or `run_command` before asking the user.
- In this repository, use [`RTK.md`](/Users/joe/Documents/skill/RTK.md) only when the command is high-volume and compact output still preserves the needed evidence; prefer raw output when the exact failing line matters.
- If reproduction is unconfirmed, say so explicitly — never assume it can be reproduced.
- If three fix attempts fail, step back and challenge the premise or architecture.
- **Anti-laziness checkpoint**: before handing off to a domain owner, the debugging record must show: symptom + evidence source + observed (not inferred) root cause.

## Anti-laziness integration

This skill activates `$anti-laziness` enforcement when:
- Two or more fix attempts used the same approach without variation (Spinning Wheels).
- A "fix" is proposed based on theory before any tool output has been collected.
- Output contains `...` or partial code snippets instead of full diagnostic output.
- The user has already said: "不知道为什么"、"帮我找一下" and no grep/log tool has been called yet.

## Framework note

Emit a finding-like debugging record before handing execution to a fixer,
TDD workflow, or domain owner.

## Trigger examples

- "帮我修这个报错" (no root cause given)
- "为什么这个功能不工作了"
- "程序崩了/失败了，不知道为什么"
- "这里出错了: [error message without identified cause]"
- "Use $systematic-debugging to investigate this failure before patching."

## References

- [references/hypothesis-checklist.md](references/hypothesis-checklist.md) — Hypothesis formation rules, testing matrix, failure routing table, anti-spinning protocol
