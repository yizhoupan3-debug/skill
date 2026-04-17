---
name: npm-package-authoring
description: |
  Build, refactor, and publish npm packages and JavaScript/TypeScript libraries
  including `package.json`, `main/module/types/bin`, exports maps, ESM/CJS
  compatibility, type output, bundlers, versioning, registries, and package
  publish workflows. Use proactively when the user asks to turn code into an
  npm package, publish a library, fix `exports`, support ESM/CJS, ship a CLI,
  or phrases like "发 npm 包" "写 package.json" "库发布" "exports 怎么配".
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - npm
    - package
    - publishing
    - library
    - package-json
risk: medium
source: local
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
---
- **Dual-Dimension Audit (Pre: Package-JSON/Exports, Post: Publish-DryRun/Bundle-Size Results)** → `$execution-audit-codex` [Overlay]
# npm-package-authoring

This skill owns npm package and library authoring so packaging, exports, release shape, and publishability are handled as first-class concerns instead of an afterthought.

## When to use

- The user wants to turn code into an npm package or publish/update a library
- The task involves `package.json`, exports maps, package entry points, type declarations, bundling, or registry publishing
- The user wants to support ESM/CJS, choose a library build tool, or design package release/versioning strategy
- The task involves package metadata, public API surface, or package-consumer compatibility
- Best for requests like:
  - "帮我把这个工具发成 npm 包"
  - "这个 package.json / exports 怎么写"
  - "做一个同时支持 ESM/CJS 的库"
  - "设计 npm 包的构建和发布流程"

## Do not use

- The main task is monorepo/workspace structure rather than one package's packageability → use `$monorepo-tooling`
- The main task is GitHub Actions YAML authoring for CI/CD → use `$github-actions-authoring`
- The task is generic JavaScript/TypeScript app development rather than library/package concerns → use `$javascript-pro` or `$typescript-pro`
- The task is plain Node backend implementation rather than package publishing → use `$node-backend`

## Task ownership and boundaries

This skill owns:
- npm package structure and metadata
- entry points, exports maps, and module compatibility
- library bundling and package output shape
- package release/versioning/publish strategy
- consumer-facing package ergonomics and installability

This skill does not own:
- monorepo-wide package graph design
- CI workflow authoring itself
- generic application feature implementation
- backend service architecture

If the task shifts to adjacent skill territory, route to:
- `$monorepo-tooling`
- `$github-actions-authoring`
- `$javascript-pro`
- `$typescript-pro`
- `$node-backend`

## Required workflow

1. Confirm the task shape:
   - object: package, library, package config, build output, publish workflow
   - action: create, refactor, package, publish, debug compatibility
   - constraints: JS/TS, ESM/CJS, registry, versioning, consumer environment
   - deliverable: package config, build setup, publish plan, or implementation
2. Define the package's public API and target consumers first.
3. Make entry points, exports, and output formats explicit.
4. Validate install/build/consume behavior.
5. Call out release/publish prerequisites clearly.

## Core workflow

### 1. Intake
- Determine whether the package is application-internal, public npm, or private registry.
- Identify consumer expectations: Node, browser, bundler, TS types, CLI, or hybrid.
- Inspect current `package.json`, build tool, and output directory conventions.

### 2. Execution
- Define package name, entry points, files, exports, and type surface deliberately.
- Choose build tooling that matches the package complexity and output needs.
- Handle ESM/CJS compatibility explicitly instead of relying on accidental transpiler behavior.
- Keep publish-time metadata (`files`, `main`, `module`, `types`, `exports`, `bin`) coherent.

### 3. Validation / recheck
- Re-check package installability, import/require behavior, and type resolution where applicable.
- Validate that publish output excludes junk and includes required artifacts.
- If actual publish is blocked on credentials/registry state, say so explicitly.

## Output defaults

Default output should contain:
- package target and output model
- package/build/publish changes
- validation notes and publish prerequisites

Recommended structure:

````markdown
## Package Summary
- Package type: ...
- Consumers: ...

## Package / Build Changes
- `package.json`: ...
- Exports / output: ...
- Release flow: ...

## Validation / Publish Notes
- Checked: ...
- Needs credentials / tags / registry setup: ...
````

## Hard constraints

- Do not leave exports/entry points ambiguous when consumers depend on stable imports.
- Do not assume ESM/CJS compatibility without testing the actual package boundary.
- Do not publish incidental source junk if `files` or output filtering should be used.
- Do not design package metadata only for local dev; think about downstream consumers.
- **Superior Quality Audit**: For production-grade npm packages, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).
- If publishability depends on registry credentials or org settings, say so explicitly.

## Trigger examples

- "Use $npm-package-authoring to package and publish this library."
- "帮我把这个项目做成 npm 包并写好 package.json。"
- "这个 exports / ESM/CJS 兼容到底该怎么配？"
- "强制进行 NPM 包发布审计 / 检查 Exports 定义与打包体积结果。"
- "Use $execution-audit-codex to audit this npm package for consumer-compatibility idealism."
