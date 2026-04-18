---
name: build-tooling
description: |
  Debug and design JS/TS/Python build tooling across package managers,
  lockfiles, bundlers, compilers, and CI pipelines.
  Use for install/build failures, module-resolution drift, ESM/CJS issues,
  and toolchain mismatches before app logic runs.
metadata:
  version: "1.0.1"
  platforms: [codex]
  tags:
    - build
    - bundler
    - package-manager
    - lockfile
    - vite
    - webpack
    - module-resolution
risk: medium
source: local
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
allowed_tools:
  - shell
  - git
  - python
  - node
approval_required_tools:
  - git push
---
- **Dual-Dimension Audit (Pre: Config/Lockfile, Post: Bundle-Success/Dep-Graph Results)** → `$execution-audit-codex` [Overlay]
# build-tooling

This skill owns dependency, compiler, bundler, and toolchain problems when the main blocker is build infrastructure rather than the app's runtime business logic.

## When to use

- The user is blocked by install, build, bundle, compile, or module-resolution failures
- The task involves npm/pnpm/Yarn/bun/uv/pip/poetry, lockfiles, version drift, environment injection, or build scripts
- The task involves Vite, Webpack, Rollup, esbuild, tsup, Babel, tsconfig/module alias resolution, or ESM/CJS boundary issues
- The user wants to rationalize or simplify local/CI build pipelines and toolchain choices
- Best for requests like:
  - "为什么 pnpm install / bun install / poetry install 失败"
  - "这个 Vite/Webpack/esbuild 构建报错怎么修"
  - "ESM/CJS、tsconfig path、alias、打包入口为什么有问题"
  - "帮我整理这个项目的 build/test/dev toolchain"

## Do not use

- The task is workspace topology and cross-package repo architecture → use `$monorepo-tooling`
- The task is publishing one npm package's public packaging contract → use `$npm-package-authoring`
- The main task is framework app implementation after the build is already healthy → use the relevant domain skill
- The task is CI workflow YAML authoring rather than the toolchain itself → use `$github-actions-authoring`
- The task is LaTeX-specific compile-speed, watch-loop, engine, or externalization tuning → use `$latex-compile-acceleration`

## Task ownership and boundaries

This skill owns:
- dependency/install/build pipeline diagnosis
- bundler/compiler/transpiler configuration and resolution behavior
- lockfile hygiene and package-manager drift
- ESM/CJS/module format boundaries and alias resolution
- environment variable injection and local-vs-CI build mismatches

This skill does not own:
- repo-wide monorepo boundary design
- package publishing strategy by itself
- one framework's runtime architecture
- generic server operations

If the task shifts to adjacent skill territory, route to:
- `$monorepo-tooling`
- `$npm-package-authoring`
- `$github-actions-authoring`
- `$typescript-pro`
- `$python-pro`

## Required workflow

1. Confirm the task shape:
   - object: install/build/dev/test pipeline, bundler config, lockfile, module graph
   - action: debug, simplify, migrate, stabilize, review
   - constraints: package manager, runtime, CI, module format, OS
   - deliverable: fixed config, diagnosis, migration plan, or stable commands
2. Reproduce the failing command exactly before changing config.
3. Separate dependency, resolution, transpilation, bundling, and environment layers.
4. Prefer the smallest config change that restores determinism.
5. Re-validate both local and affected automation paths.

## Core workflow

### 1. Intake
- Identify the failing command, working directory, runtime version, package manager, and exact error output.
- Inspect the relevant config files first: `package.json`, lockfiles, `tsconfig*`, bundler config, env-loading config, or Python build metadata.
- Determine whether the issue is install-time, compile-time, bundle-time, start-up, or CI-only.

### 2. Execution
- Verify version compatibility among runtime, package manager, plugins, loaders, and framework.
- Check lockfile freshness, duplicate package-manager use, and node_modules/cache assumptions.
- Trace module format and resolution boundaries: ESM/CJS, exports maps, aliases, path mapping, file extensions, and condition ordering.
- Audit bundler/compiler config for unnecessary complexity, conflicting plugins, and environment leaks.
- When migrating tooling, preserve the minimal set of contracts the repo actually relies on.

### 3. Validation / recheck
- Re-run the failing command and one adjacent command likely to regress.
- Confirm whether the fix is deterministic from a clean install/cache state.
- Call out any required version pinning, lockfile regeneration, or CI follow-up.
- If the issue is only mitigated, say so explicitly.

## Output defaults

Default output should contain:
- failing layer and root cause summary
- config/package/tooling changes
- validation results and remaining drift risks

Recommended structure:

````markdown
## Build Tooling Summary
- Failing command/layer: ...
- Root cause: ...

## Changes / Recommendations
- ...

## Validation / Follow-up
- Re-ran: ...
- Remaining risks: ...
````

## Hard constraints

- Do not change multiple tooling layers at once without isolating the failing layer.
- Do not delete lockfiles or caches as a "fix" unless the underlying inconsistency is identified.
- Do not mix package managers casually in one project without an explicit reason.
- Always preserve the exact failing command and environment in the diagnosis.
- **Superior Quality Audit**: For stable build infrastructure and toolchains, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).
- If a build succeeds only because of a local cache or undeclared global dependency, call that out.

## Trigger examples

- "Use $build-tooling to fix this Vite/Webpack/module-resolution build failure."
- "帮我排查 pnpm/bun/poetry 依赖安装和 lockfile 漂移问题。"
- "为什么这个 ESM/CJS、alias、tsconfig、bundler 配置总是打不通？"
- "强制进行构建工具深度审计 / 检查依赖图谱与打包结果一致性。"
- "Use $execution-audit-codex to audit this build toolchain for dependency-integrity idealism."
