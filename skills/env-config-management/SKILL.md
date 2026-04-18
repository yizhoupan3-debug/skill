---
name: env-config-management
description: |
  Design, audit, debug, and implement app configuration across env vars,
  `.env`, secrets, validation, feature flags, and multi-environment switching.
  Use for config layering, dotenv / vault / envalid / zod-joi schemas, drift
  between dev/staging/prod, and requests like “管理环境变量”, “排查 .env 问题”,
  “多环境切换”, or “config validation”. Best for config-layer work, not build
  tooling or platform deployment.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 管理环境变量
  - 排查 .env 问题
  - 多环境切换
  - config validation
  - config
  - env
  - dotenv
  - secrets
  - feature flags
  - twelve factor
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - config
    - env
    - dotenv
    - secrets
    - feature-flags
    - twelve-factor
    - config-validation
    - multi-environment
risk: low
source: local
---

# env-config-management

This skill owns application configuration management when the primary task is
designing, debugging, auditing, or implementing how an application loads,
validates, layers, and distributes configuration values across environments.

## When to use

- The user needs to design or audit a config management strategy (env vars, config files, secrets, feature flags)
- The task involves `.env` files, `dotenv`, `cross-env`, config layering, or environment variable injection
- The user wants config validation using schema tools (zod, joi, envalid, pydantic Settings, convict, etc.)
- The task involves multi-environment config switching (dev / staging / prod / test)
- The user needs to set up or audit secret management (vault, AWS Secrets Manager, GCP Secret Manager, 1Password CLI, SOPS, age)
- The user wants to design or implement feature flags or runtime config hot-reload
- The task involves 12-factor app config principles or config drift detection
- Best for requests like:
  - "帮我设计这个项目的环境变量管理方案"
  - "为什么 production 的配置和 staging 不一样"
  - ".env 文件怎么管理才安全"
  - "帮我加 config validation，启动时校验环境变量"
  - "帮我设计 feature flag 系统"
  - "怎么让 secret 不提交到 git"
  - "12-factor config best practices"

## Do not use

- The task is build toolchain / bundler / module resolution → use `$build-tooling`
- The task is Dockerfile / compose environment setup → use `$docker`
- The task is CI/CD workflow YAML with secrets → use `$github-actions-authoring`
- The task is deployment platform specifics (Cloudflare, Vercel) → use the relevant deploy skill
- The task is security vulnerability scanning → use `$security-audit`
- The task is server-level OS environment or systemd config → use `$linux-server-ops`
- The task is runtime cache/queue/datastore config → use `$datastore-cache-queue`

## Task ownership and boundaries

This skill owns:
- Environment variable design, naming conventions, and documentation
- `.env` file management, `.env.example` templates, and gitignore patterns
- Config schema definition and startup-time validation
- Config layering strategy (defaults → env → file → CLI override)
- Multi-environment config switching and drift detection
- Secret management integration (vault, cloud secret managers, SOPS, age)
- Feature flag design and runtime config patterns
- 12-factor app config compliance auditing

This skill does not own:
- Build pipeline or bundler configuration
- Container orchestration environment
- CI/CD secret injection mechanics
- Database connection pooling or ORM configuration (runtime behavior)
- Application deployment process

If the task shifts to adjacent territory, route to:
- `$build-tooling` for bundler env injection (e.g., `define` in Vite, `DefinePlugin` in Webpack)
- `$docker` for Docker Compose env_file and container env
- `$github-actions-authoring` for GitHub Actions secrets and vars
- `$security-audit` for secret leakage scanning
- `$linux-server-ops` for systemd environment directives

## Core workflow

### 1. Intake

- Identify the config surface: what values are configurable, where they come from, who consumes them
- Map current config sources: env vars, `.env` files, config files (JSON/YAML/TOML), CLI args, hardcoded defaults
- Identify environments: local dev, CI, staging, production, preview
- Assess sensitivity: which values are secrets, which are public, which are feature flags

### 2. Execution

- Design or refactor the config loading chain with clear precedence rules
- Add config schema validation at application startup (fail fast on missing/invalid config)
- Ensure `.env.example` is maintained and `.env` is gitignored
- Separate public config from secrets; secrets should never appear in source control
- Implement feature flags with clear on/off semantics and safe defaults
- For multi-environment setups, prefer environment-based injection over file switching
- Document every config key: purpose, type, default, required/optional, sensitivity level

### 3. Validation

- Verify that the app starts correctly with only `.env.example` values filled in
- Verify that missing required config causes a clear startup error
- Verify that secrets are not committed, logged, or exposed in client bundles
- Spot-check config in each environment for drift or unexpected overrides
- Confirm feature flags have safe defaults and can be toggled without redeployment

## Output defaults

```markdown
## Config Management Summary
- Config sources: [env vars, .env, config files, secrets manager, ...]
- Environments: [dev, staging, prod, ...]
- Sensitive keys: [count, management method]

## Changes / Recommendations
1. ...

## Validation Results
- Startup validation: PASS / FAIL
- Secret exposure check: PASS / FAIL
- Config drift: [findings]

## Config Key Reference
| Key | Type | Default | Required | Sensitive | Description |
|-----|------|---------|----------|-----------|-------------|
| ... | ...  | ...     | ...      | ...       | ...         |
```

## Hard constraints

- Never commit actual secret values to source control; always use `.env.example` with placeholder values
- Always validate config at startup; do not let invalid config cause runtime failures later
- Always document the precedence order when multiple config sources exist
- Prefer typed config objects over raw `process.env` / `os.environ` access scattered throughout code
- If a config key is required, fail loudly at startup rather than silently falling back to an empty string
- Feature flags must always have a safe default (usually `false` / disabled)

## Trigger examples

- "Use $env-config-management to design a config validation schema for this project."
- "帮我设计多环境配置切换方案，dev/staging/prod 分离。"
- "排查为什么 prod 环境的某个配置和 staging 不一样。"
- "帮我把 secret 从 .env 迁移到 vault / AWS Secrets Manager。"
