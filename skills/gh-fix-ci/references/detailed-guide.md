# gh-fix-ci — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Task ownership and boundaries

This skill owns:
- resolving the PR and failing GitHub Actions checks
- fetching run/job logs
- classifying failures by type
- producing a likely-root-cause summary
- drafting a fix plan

This skill does not own:
- non-GitHub-Actions CI debugging beyond reporting URLs
- human PR review thread management
- generic git publishing workflow

If the task shifts to adjacent skill territory, route to:
- `$gh-address-comments` for review comments
- `$checklist-writting` if the user only wants a broader checklist or plan without CI inspection
- `$github-actions-authoring` if the root cause is a workflow structure issue (trigger, cache, matrix, permissions)

> **Circular handoff with `$github-actions-authoring`**: When triage reveals the
> failure is due to workflow YAML problems (wrong trigger, stale cache key,
> missing permission, bad matrix), hand off to `$github-actions-authoring` for
> the structural fix. After the fix, this skill should re-verify the PR checks.

## Required workflow

1. Confirm `gh` authentication and PR scope.
2. Inspect failing checks with the local helper first.
3. Classify failures by type and confidence.
4. Summarize evidence and propose a fix plan.
5. Implement only after explicit user approval.
6. Recheck relevant status after changes.

## Core workflow

### 1. Intake

- Verify authentication:
  - `gh auth status`
- Resolve the target PR:
  - current branch PR by default
  - user-provided PR number or URL if given
- Preferred helper:
  - `cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/gh_source_gate_rs/Cargo.toml --bin gh-source-gate -- inspect-pr-checks --repo . --json`

### 2. Inspect failures

- Use the helper first to fetch:
  - failing checks
  - run URLs
  - log snippets
  - machine-readable JSON when useful
- Manual fallback:
  - `gh pr checks`
  - `gh run view`
  - `gh api .../actions/jobs/.../logs`

### 3. Classify the failure

Classify each failure as one of:
- `lint`
- `typecheck`
- `unit-test`
- `integration-test`
- `build`
- `missing-secret-or-env`
- `infra-or-flake`
- `external-provider`

For each item, record:
- failing check name
- run URL
- strongest evidence snippet
- likely root cause
- confidence level

### 4. Plan before fixing

- Do not jump straight into code changes.
- Summarize:
  - what failed
  - why it probably failed
  - what should be changed
  - what verification to run after the fix
- Ask for approval before implementation.

### 5. Recheck after changes

- Re-run the smallest relevant local verification first.
- Suggest re-running or rechecking the impacted PR checks.

## Output defaults

Default output should contain:
- failing checks summary
- likely causes
- fix plan

Recommended structure:

````markdown
## CI Summary
- PR: ...
- Failing checks: ...

## Failure Breakdown
1. Check: ...
   - Type: ...
   - Evidence: ...
   - Likely cause: ...
   - Confidence: ...

## Proposed Fix Plan
- ...

## Verification
- ...
````

## Hard constraints

- Do not implement fixes before user approval.
- Do not treat non-GitHub-Actions providers as if their logs are locally inspectable here.
- Do not hide missing logs or low confidence.
- Prefer the bundled helper before ad hoc manual commands.
- Keep evidence concise and actionable.

## Trigger examples

- "Use $gh-fix-ci to inspect why this PR's GitHub Actions checks are failing."
- "Summarize the failing checks and propose a fix plan before changing code."
- "看这个 PR 的 CI 日志，告诉我最可能的根因。"

## Optional supporting assets

- Rust CLI: `/Users/joe/Documents/skill/rust_tools/gh_source_gate_rs`
- `assets/`
- `agents/`
