---
name: dependency-migration
description: |
  Manage, audit, upgrade, and migrate project dependencies across npm, pip,
  Cargo, and Go. Covers vulnerability scanning, lockfile hygiene, license
  compliance, major version migration (React 18→19), codemod execution,
  and cross-version compatibility.
  Use when asked to 升级依赖, 扫描漏洞, 解决 lockfile 冲突, 做大版本迁移,
  upgrading frameworks, or planning migration paths.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - dependency
  - npm audit
  - pip audit
  - cargo audit
  - lockfile
  - license
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - dependency
    - npm-audit
    - pip-audit
    - cargo-audit
    - lockfile
    - license
    - vulnerability
    - upgrade
    - migration
    - breaking-change
    - codemod
    - version-migration
    - schema-migration
risk: medium
source: local

---

# dependency-migration

This skill owns **dependency lifecycle management and version migration
engineering**: vulnerability scanning, upgrade planning, lockfile hygiene,
license compliance, major version migration, codemod execution, and
cross-version compatibility verification.

> Merged from `dependency-management` + `migration-upgrade`.

## When to use

- The user needs to audit dependencies for known vulnerabilities (CVEs, advisories)
- The task involves planning or executing a dependency version upgrade (minor, major, or cross-ecosystem)
- The user wants to resolve lockfile conflicts, prune unused dependencies, or analyze the dependency graph
- The task involves license compliance checking across project dependencies
- The user needs to upgrade a framework, library, or runtime to a new major version
- The task involves planning a safe migration path with incremental steps
- The user wants to inventory breaking changes from a version upgrade
- The task involves running or writing codemods for automated migration
- The user needs cross-version compatibility verification
- The task involves database schema migration strategy or API version migration
- Best for requests like:
  - "帮我跑一下依赖安全扫描，看有没有漏洞"
  - "这个项目有哪些依赖可以升级"
  - "lockfile 有冲突，帮我解决"
  - "检查一下所有依赖的 license 是否合规"
  - "帮我从 React 18 迁移到 React 19"
  - "Next.js 14 升 15 有哪些 breaking changes"
  - "规划一下 Python 3.11 到 3.13 的升级路径"
  - "帮我跑 codemod 自动迁移"

## Do not use

- The task is build/bundler/module-resolution failures → use `$build-tooling`
- The task is publishing an npm package → use `$npm-package-authoring`
- The task is using the framework after migration is done → use the framework skill (`$react`, `$nextjs`, etc.)
- The task is ORM/cache/queue runtime behavior → use `$datastore-cache-queue`
- The task is code restructuring without a version change → use `$refactoring`
- The task is monorepo workspace dependency topology → use `$monorepo-tooling`

## Core workflow

### 1. Inventory & Assess

- List all direct and transitive dependencies with current versions
- Identify the package manager(s) and lockfile(s) in use
- Run vulnerability scans: `npm audit`, `pip-audit`, `cargo audit`, or equivalent
- Identify outdated dependencies: `npm outdated`, `pip list --outdated`, `cargo outdated`
- Analyze the dependency graph for unused, duplicate, or excessively heavy packages
- Check license compatibility against project requirements
- For major upgrades: read official migration guides, changelogs, and breaking change lists
- Inventory all usage of deprecated or changed APIs in the codebase

### 2. Plan

- Prioritize: critical vulnerabilities > major version upgrades > minor/patch > cleanup
- For each major upgrade, inventory breaking changes from changelogs/migration guides
- Sequence upgrades to minimize cascading breakage
- Determine migration strategy: big-bang vs incremental vs strangler fig
- Identify available codemods or transformation tools
- Plan compatibility shims for gradual transition if needed
- Define verification criteria for each migration step
- Plan rollback strategy
- Identify test coverage needed to verify each upgrade safely

### 3. Execute

- Apply upgrades incrementally, not all at once
- Run available codemods first for mechanical transformations
- Apply manual changes for cases codemods cannot handle
- Update configuration files, type definitions, and build settings
- Run tests after each upgrade step
- Update lockfiles deterministically
- Commit each migration step separately with clear descriptions
- Document any workarounds, patches, or held-back versions with rationale

### 4. Verify

- Re-run vulnerability scans to confirm fixes
- Re-run the full test suite
- Verify no regressions in build, startup, or runtime behavior
- Test edge cases and deprecated API replacements specifically
- Confirm backward compatibility if running in mixed-version mode
- Document the final dependency state and any remaining known issues

## Output defaults

```markdown
## Dependency & Migration Summary
- Ecosystem: [npm / pip / cargo / mixed]
- Total dependencies: [direct / transitive]
- Migration: [from version] → [to version] (if applicable)
- Strategy: [big-bang / incremental / strangler fig]

## Vulnerabilities Found
| Package | Severity | CVE | Fix Version | Status |
|---|---|---|---|---|

## Breaking Changes Inventory (if migration)
| Area | Change | Impact | Migration Action |
|---|---|---|---|

## Upgrade / Migration Steps
1. [step] — [status: done / pending / deferred]

## License Check
- [status: PASS / issues found]

## Verification Results
- Tests: PASS / FAIL
- Deferred items: ...

## Rollback Plan
- ...
```

## Hard constraints

- Never upgrade a major version without checking the changelog for breaking changes
- Always run vulnerability scans before and after upgrades
- Prefer incremental upgrades over batch upgrades
- Document any version holds with a clear rationale
- Do not remove lockfiles as a fix; regenerate them properly
- If a vulnerability has no fix available, document the risk and mitigation
- Always read the official migration guide before starting a major migration
- Run tests after each migration step, not just at the end
- If codemods exist, use them before manual changes
- Keep a rollback plan until migration is fully verified
