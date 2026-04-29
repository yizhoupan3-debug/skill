---
name: monorepo-tooling
description: |
  Design clean package boundaries and task orchestration for multi-package repositories.
  Use when the user asks to set up a monorepo, split apps and shared packages, fix
  workspace resolution, debug cross-package imports, or phrases like 'pnpm workspace',
  'Turborepo 结构', 'monorepo 怎么组织', '共享包引用有问题'.
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - monorepo
    - workspace
    - turborepo
    - nx
    - pnpm
risk: medium
source: local
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - pnpm workspace
  - Turborepo 结构
  - monorepo 怎么组织
  - 共享包引用有问题
  - split apps
  - shared packages
  - fix workspace resolution
  - debug cross-package imports
  - monorepo
  - workspace

---

- **Dual-Dimension Audit (Pre: Workspace-Topo/Boundary, Post: DAG-Execution/Cache-Hits Results)** → runtime verification gate
# monorepo-tooling

This skill owns repository-level workspace structure and tooling orchestration when the problem is package boundaries, task graphs, and shared tooling rather than one app in isolation.

## When to use

- The user wants to create, reorganize, or debug a monorepo/workspace
- The task involves pnpm workspaces, npm workspaces, Yarn workspaces, Turborepo, Nx, or multi-package repo structure
- The task involves shared packages, shared config, task pipelines, workspace scripts, or incremental builds
- The user wants to split one repo into apps/packages or consolidate multiple packages under one workspace
- Best for requests like:
  - "帮我搭一个 pnpm workspace / monorepo"
  - "Turborepo 结构怎么设计"
  - "共享 package 和 config 怎么组织"
  - "为什么 workspace script / task graph 跑不通"

## Do not use

- The task is npm package authoring/publishing for one package rather than repo-wide workspace tooling → use `$npm-package-authoring`
- The main task is one application's framework implementation → use the relevant domain skill
- The task is CI workflow authoring rather than workspace structure → use `$github-actions-authoring`
- The task is generic Git organization rather than monorepo tooling → use `$gitx`

## Task ownership and boundaries

This skill owns:
- monorepo/workspace layout
- package boundaries and shared-config strategy
- task orchestration and workspace scripts
- build/test/lint graph design and cache-aware tooling
- cross-package dependency hygiene

This skill does not own:
- individual package publishing policy by itself
- one framework's internal implementation details
- CI workflow YAML authoring
- generic Git operations

If the task shifts to adjacent skill territory, route to:
- `$npm-package-authoring`
- `$github-actions-authoring`
- `$gitx`
- `$typescript-pro`
- `$node-backend`

## Required workflow

1. Confirm the task shape:
   - object: apps/packages/workspace graph, shared config, build/test pipeline
   - action: create, split, merge, refactor, debug, optimize
   - constraints: package manager, runner, cache tooling, publish model
   - deliverable: repo structure, config changes, script fixes, or plan
2. Identify package boundaries and dependency directions before moving files.
3. Prefer explicit workspace contracts over ad hoc cross-imports.
4. Keep tooling choices aligned with repo scale and team needs.
5. Validate package resolution, scripts, and task graph behavior.

## Core workflow

### 1. Intake
- Determine package manager and workspace tooling.
- Identify current pain: structure confusion, duplicated config, slow tasks, broken resolution, or unclear ownership.
- Inspect existing `package.json`, workspace config, and directory layout before proposing changes.

### 2. Execution
- Define app/package boundaries first.
- Centralize shared config only where it reduces duplication without obscuring intent.
- Keep dependency direction clean and avoid circular package relationships.
- Use task graphs/caches intentionally; do not add heavy orchestration without payoff.

### 3. Validation / recheck
- Re-check workspace install, package resolution, scripts, and local task execution.
- Verify that imports, build outputs, and shared package references resolve correctly.
- Call out any required follow-up for CI, publishing, or versioning.

## Output defaults

Default output should contain:
- workspace structure and tool choices
- config/package changes
- validation notes and remaining repo-wide risks

Recommended structure:

````markdown
## Monorepo Summary
- Tooling: ...
- Package boundaries: ...

## Structure / Changes
- ...

## Validation / Follow-up
- Verified: ...
- Remaining work: ...
````

## Hard constraints

- Do not create cross-package imports that bypass declared workspace boundaries.
- Do not add monorepo orchestration complexity without a clear payoff.
- Do not centralize config so aggressively that per-package behavior becomes opaque.
- If publishing/versioning strategy is affected, call it out explicitly.
- Preserve a clear dependency direction between apps and shared packages.
- **Superior Quality Audit**: For high-efficiency monorepo architectures, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples

- "Use $monorepo-tooling to set up a pnpm workspace with apps and packages."
- "帮我设计 monorepo 结构和 workspace scripts。"
- "Turborepo/Nx 这里为什么 task graph 和共享包有问题？"
- "强制进行 Monorepo 深度审计 / 检查 DAG 拓扑与缓存执行结果。"
- "Use the runtime verification gate to audit this monorepo for workspace-topo idealism."
