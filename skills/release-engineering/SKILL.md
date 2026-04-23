---
name: release-engineering
description: |
  Build release pipelines from commit to published artifact.
  Use for versioning strategy, changelog automation, and automated tag→build→publish flows.
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - release
    - versioning
    - semver
    - changelog
    - semantic-release
    - changesets
    - release-please
    - github-releases
    - publish
    - tag
risk: medium
source: local
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - release
  - versioning
  - semver
  - changelog
  - semantic release
  - changesets
---

# release-engineering

This skill owns the end-to-end release lifecycle: from version strategy design through changelog generation, tagging, release note creation, artifact publishing, and release pipeline orchestration.

## When to use

- The user wants to design or implement a versioning strategy (semver, calver, custom)
- The task involves changelog generation (conventional commits, changesets, release-please, semantic-release)
- The user wants to set up GitHub Releases with assets and release notes
- The task involves release pipeline design: tag → build → test → publish → deploy
- The user wants to publish to registries (npm, PyPI, crates.io, Docker Hub, GHCR) as part of a release flow
- The task is about release automation, release orchestration, or release tooling selection
- Best for requests like:
  - "帮我搭一套自动发布流程"
  - "设计版本号策略，该用 semver 还是 calver"
  - "帮我配 semantic-release / release-please / changesets"
  - "自动生成 changelog 和 release notes"
  - "从 tag 到 publish 的完整 release pipeline"

## Do not use

- The task is Git operations (branch, commit, push, rebase) → use `$gitx`
- The task is writing GitHub Actions workflow YAML → use `$github-actions-authoring`
- The task is npm package structure (`package.json`, exports, ESM/CJS) → use `$npm-package-authoring`
- The task is fixing broken CI checks → use `$gh-fix-ci`
- The task is Cloudflare-specific deployment → use `$cloudflare-deploy`
- The task is deployment strategy design (blue-green, canary) beyond release creation

## Task ownership and boundaries

This skill owns:
- versioning strategy design (semver, calver, pre-release, build metadata)
- conventional commits enforcement and tooling
- changelog generation tooling selection and configuration
- release note authoring and automation
- GitHub Releases creation and asset management
- tag management and release branching strategy
- release pipeline orchestration (tag → build → test → publish → notify)
- multi-registry publish coordination
- release tooling comparison and selection

This skill does not own:
- Git operations execution (branching, committing, pushing) — delegate to `$gitx`
- CI/CD workflow YAML authoring — delegate to `$github-actions-authoring`
- npm package structure and exports — delegate to `$npm-package-authoring`
- Docker image build/push details — delegate to `$docker`
- deployment infrastructure (servers, Cloudflare, VMs)

If the task shifts to adjacent skill territory, route to:
- `$gitx` for tag/branch Git operations
- `$github-actions-authoring` for workflow file creation
- `$npm-package-authoring` for package structure
- `$docker` for image build/push
- `$cloudflare-deploy` for Cloudflare rollout

## Required workflow

1. Understand the project type, registry targets, and current release maturity.
2. Recommend a versioning strategy based on project needs.
3. Select release tooling based on ecosystem and team size.
4. Design the release pipeline end-to-end.
5. Implement configuration and workflow integration.
6. Validate with a dry-run release or documentation.

## Core workflow

### 1. Intake

- Identify project type: library, CLI, web app, monorepo, multi-package
- Identify target registries: npm, PyPI, crates.io, Docker Hub, GHCR, GitHub Releases
- Check existing version management: manual, `npm version`, tags, pre-existing tooling
- Check existing CI/CD: GitHub Actions, GitLab CI, none
- Identify commit message conventions: conventional commits, free-form, mixed

### 2. Design versioning strategy

- **Semver** (most libraries/packages): `MAJOR.MINOR.PATCH`
  - Breaking changes → major; new features → minor; fixes → patch
  - Pre-release: `1.0.0-beta.1`, `1.0.0-rc.1`
- **Calver** (continuously deployed apps): `YYYY.MM.DD` or `YYYY.MM.PATCH`
- **Monorepo**: independent versioning per package vs synced versions
- Document the strategy in README or CONTRIBUTING

### 3. Select release tooling

| Tool | Best for | Ecosystem |
|------|----------|-----------|
| `semantic-release` | Full automation, single package | npm, GitHub |
| `release-please` | Google-style, monorepo support | Any, GitHub Actions |
| `changesets` | Monorepo, human-curated changelogs | npm, pnpm workspaces |
| `standard-version` | Simple semver bump + changelog | npm (deprecated, use alternatives) |
| `cargo release` | Rust crates | Cargo, crates.io |
| Manual | Small projects, learning | Any |

### 4. Configure release pipeline

Typical pipeline stages:
1. **Commit** — enforce conventional commits (commitlint, commitizen)
2. **Version bump** — automated by release tool based on commit types
3. **Changelog** — generated from commits, grouped by type
4. **Tag** — created by release tool or CI
5. **Build** — compile, bundle, create artifacts
6. **Test** — run full test suite against release build
7. **Publish** — push to registries
8. **GitHub Release** — create release with notes and assets
9. **Notify** — Slack, Discord, email, or webhook

### 5. Validation / recheck

- Do a dry-run release if the tool supports it
- Verify changelog format and completeness
- Check that version bump follows the expected pattern
- Verify registry credentials and permissions are configured
- Confirm tag and release creation works end-to-end

## Output defaults

Default output should contain:
- version strategy recommendation
- tooling selection with rationale
- release pipeline design
- configuration files or changes
- validation notes

Recommended structure:

````markdown
## Release Strategy
- Project type: ...
- Versioning: ...
- Tooling: ...

## Pipeline Design
1. Commit → Version → Changelog → Tag → Build → Publish
- Triggers: ...
- Registries: ...

## Configuration
- Files changed: ...
- Secrets needed: ...

## Validation / Follow-up
- Dry-run: ...
- Remaining setup: ...
````

## Hard constraints

- Do not recommend a release tool without explaining why it fits the project size and ecosystem.
- Do not skip changelog generation in a release pipeline.
- Do not hardcode version numbers when automated bumping is available.
- Do not create a publish step without verifying registry credentials are configured.
- Do not mix release pipeline design with unrelated CI/CD concerns.
- If the project is a monorepo, address per-package vs synced versioning explicitly.

## Trigger examples

- "Use $release-engineering to set up automated releases for this npm package."
- "帮我搭一套自动发布流程，从 commit 到 npm publish。"
- "该用 semantic-release 还是 changesets？帮我选并配好。"
- "设计版本号策略，这个项目该用 semver 还是 calver？"
- "帮我生成 changelog 并创建 GitHub Release。"

## References

Detailed guides for specific release tools are in `references/`:

- [semantic-release-guide.md](references/semantic-release-guide.md) — setup, plugins, monorepo config
- [changesets-guide.md](references/changesets-guide.md) — monorepo workflow with changesets
- [conventional-commits.md](references/conventional-commits.md) — commit convention and enforcement
