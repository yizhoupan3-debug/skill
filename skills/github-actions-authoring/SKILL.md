---
name: github-actions-authoring
description: |
  Produce GitHub Actions workflow YAML with minimal permissions, stable cache
  keys, and clear job dependencies. Delivers `.github/workflows/*.yml` files
  for CI/CD pipelines including test, build, release, and deployment jobs.
  Use when the user asks to write GitHub Actions, add CI/CD, set up test/build/
  release workflows, or phrases like "写 workflow", "搭 CI/CD", "GitHub
  Actions", "自动测试自动发布".
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - github-actions
    - ci
    - cd
    - workflow
    - release

routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 写 workflow
  - 搭 CI
  - CD
  - GitHub Actions
  - 自动测试自动发布
  - add CI
  - set up test
  - build
  - release workflows
  - ci
---

# github-actions-authoring

This skill owns GitHub Actions workflow authoring and pipeline structure: new workflows, refactors, job orchestration, caching, permissions, and release/deploy automation.

## When to use

- The user wants to create or modify GitHub Actions workflow files
- The task involves `.github/workflows/*.yml`, reusable workflows, matrix builds, artifacts, caches, release automation, or deployment jobs
- The user wants to add CI/CD for tests, lint, build, package, publish, or deploy steps
- The task involves workflow structure, trigger design, concurrency, permissions, or runtime efficiency
- Best for requests like:
  - "写一个 GitHub Actions workflow"
  - "帮我搭 CI/CD 流水线"
  - "给这个仓库加自动测试和发布"
  - "优化 workflow 的 cache 和 matrix"

## Do not use

- The main request is diagnosing an already failing PR check or GitHub Actions run → use `$gh-fix-ci`
- The task is local Git branching, commit, or push workflow → use `$gitx`
- The main deployment concern is provider-specific rollout rather than Actions authoring → use `$cloudflare-deploy`
- The task is general system architecture instead of CI/CD implementation

## Task ownership and boundaries

This skill owns:
- GitHub Actions workflow design and YAML authoring
- job decomposition, dependencies, and trigger strategy
- cache/artifact/concurrency/reusable-workflow design
- release and deployment pipeline structure inside GitHub Actions
- permissions and secret-handling patterns within workflow files

This skill does not own:
- retrospective CI failure triage on existing broken runs
- local Git operations
- provider-specific deployment details beyond the workflow integration
- general project architecture review

If the task shifts to adjacent skill territory, route to:
- `$gh-fix-ci` for retrospective CI failure triage
- `$gitx` for branch/commit/push operations
- `$cloudflare-deploy` for provider-specific deployment
- `$architect-review` for broader system design

> **Circular handoff with `$gh-fix-ci`**: When `$gh-fix-ci` identifies that a
> failure is caused by a workflow structure problem (bad trigger, missing cache
> key, wrong matrix config), it should hand back to this skill for the
> structural fix. After the fix, re-run CI and return to `$gh-fix-ci` for
> verification if needed.

## Required workflow

1. Confirm the task shape:
   - object: workflow file, job graph, release pipeline, deploy pipeline
   - action: create, refactor, optimize, harden, document
   - constraints: triggers, runners, secrets, cache strategy, target environments
   - deliverable: workflow YAML, pipeline plan, or refactor
2. Inspect existing workflows and required checks first.
3. Design jobs for clarity, cacheability, and minimal permissions.
4. Pin actions and handle secrets explicitly.
5. Validate workflow logic and prerequisites.

## Core workflow

### 1. Intake
- Determine triggers, job types, required runners, matrices, secrets, and environment protections.
- Check whether the repo already has workflows that should be reused or consolidated.

### 2. Execution
- Build the job graph with clear dependencies and concurrency rules.
- Add cache strategy only where it materially reduces repeat cost.
- Use artifacts for explicit handoff rather than hidden job coupling.
- Scope `permissions` narrowly and prefer pinned actions.

### 3. Validation / recheck
- Review trigger behavior and branch/tag matching.
- Check secret usage, permission scope, cache keys, and failure visibility.
- If local validation tools are unavailable, state the validation limit explicitly.

## Output defaults

Default output should contain:
- pipeline scope and trigger design
- workflow/job structure
- prerequisites, validation, and risks

Recommended structure:

````markdown
## Workflow Summary
- Trigger(s): ...
- Jobs: ...

## Authoring Notes
- Cache / artifacts / matrix: ...
- Permissions / secrets: ...

## Validation / Follow-up
- Checked: ...
- Needs repo secret(s): ...
- Risks: ...
````

## Hard constraints

- Do not grant broad default permissions when narrower permissions will work.
- Do not expose secrets in logs, env echoes, or commit history.
- Do not add caching without a stable invalidation key strategy.
- Do not create hidden coupling between jobs when artifacts or explicit dependencies are clearer.
- If workflow correctness depends on unavailable repository settings or secrets, say so explicitly.

## Trigger examples

- "Use $github-actions-authoring to add CI and release workflows."
- "帮我写 GitHub Actions，自动测试、构建并发布。"
- "重构这个 workflow，优化 matrix、cache 和权限。"

## References

Detailed pattern guides are in `references/`:

- [actions-patterns.md](references/actions-patterns.md) — reusable workflows, composite actions, OIDC auth, environments, caching, matrix, concurrency, self-hosted runners, workflow_dispatch, artifact handoff
