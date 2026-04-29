---
name: sentry
description: Inspect Sentry production errors and issue evidence read-only.
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags:
    - sentry
    - production-errors
    - triage
    - error-monitoring
    - incident-debugging
risk: medium
source: local
routing_layer: L0
routing_owner: gate
routing_gate: source
session_start: required
trigger_hints:
  - Sentry evidence
  - sentry
  - production errors
  - triage
  - error monitoring
  - incident debugging
allowed_tools:
  - shell
  - python
  - web
approval_required_tools:
  - authenticated web access
filesystem_scope:
  - repo
  - artifacts
network_access: required
artifact_outputs:
  - sentry_triage.md
  - EVIDENCE_INDEX.json
  - TRACE_METADATA.json

---

# sentry

This skill is the **source gate for Sentry-grounded production triage**.
It gathers and ranks evidence from Sentry before deeper debugging or implementation.

## When to use

- The user asks to inspect Sentry issues, events, releases, or recent prod errors
- The task needs Sentry-backed prioritization of online failures
- The goal is to connect Sentry evidence to likely code paths or debugging next steps

## Do not use

- Local debugging without Sentry evidence → use `$systematic-debugging`
- Sentry configuration or alert setup changes
- Code fixing as the main task
- No Sentry access is available

## Core workflow

1. Confirm access and scope.
2. Start from issue-level triage.
3. Drill into only the highest-value issues/events.
4. Rank by impact, recency, frequency, and regression risk.
5. Convert evidence into actionable debugging next steps.

## Handoff to systematic-debugging

After extracting Sentry evidence, if the root cause is still not confirmed:
- **Must route to `$systematic-debugging`** before domain fix.
- Pass along the extracted stack trace and Sentry event metadata as the evidence block.
- Do not jump directly to a "fix" without root-cause isolation when the Sentry trace is ambiguous.

If the stack trace clearly points to a specific line/module and the cause is self-evident, you may proceed directly to the domain owner (e.g., `$frontend-debugging`, `$node-backend`).

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
