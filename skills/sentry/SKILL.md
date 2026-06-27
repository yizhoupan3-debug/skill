---

allowed_tools:
- shell
- python
- web
approval_required_tools:
- authenticated web access
description: Inspect Sentry production errors and issue evidence read-only.
metadata:
  platforms:
  - supported
  tags:
  - sentry
  - production-errors
  - triage
  - error-monitoring
  - incident-debugging
  version: '2.1.0'
name: sentry
scene: code_review
network_access: required
risk: medium
routing_gate: source
routing_layer: L0
routing_owner: gate
routing_priority: P2
session_start: required
source: project
trigger_hints:
- Sentry evidence
- error monitoring
- incident debugging
- production errors
- sentry
- triage
---
# sentry

This skill is the **source gate for Sentry-grounded production triage**.
It gathers and ranks evidence from Sentry before deeper debugging or implementation.

## When to use

- The user asks to inspect Sentry issues, events, releases, or recent prod errors
- The task needs Sentry-backed prioritization of online failures
- The goal is to connect Sentry evidence to likely code paths or debugging next steps

## Do not use

- Local debugging without Sentry evidence → 直接在当前上下文做系统化调试
- Sentry configuration or alert setup changes
- Code fixing as the main task
- No Sentry access is available

## Core workflow

1. Confirm access and scope.
2. Start from issue-level triage.
3. Drill into only the highest-value issues/events.
4. Rank by impact, recency, frequency, and regression risk.
5. Convert evidence into actionable debugging next steps.

## 调试后续步骤

After extracting Sentry evidence, if the root cause is still not confirmed:
- **Must perform systematic root-cause debugging** in the current context before domain fix（参见 `systematic-debugging/SKILL.md`）.
- Pass along the extracted stack trace and Sentry event metadata as the evidence block.
- Do not jump directly to a "fix" without root-cause isolation when the Sentry trace is ambiguous.

If the stack trace clearly points to a specific line/module and the cause is self-evident, you may proceed directly to the relevant implementation owner.

## Output defaults

```markdown
## Sentry Triage Summary
- Scope: ...
- Time window: ...

## Top Issues
- ...

## Recommended Next Steps
- ...

## Risks / Gaps
- ...
```

## Hard constraints

- Never echo `SENTRY_AUTH_TOKEN`.
- Stay read-only unless the user explicitly requests a supported write action.
- Label inference vs direct evidence clearly.
- Do not dump raw noise when a ranked summary is enough.
