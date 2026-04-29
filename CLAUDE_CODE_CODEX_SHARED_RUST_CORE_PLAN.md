# Claude Code + Codex Shared Rust Core Remaining Checklist

Last reviewed: 2026-04-29 (closeout verified)

## Goal

Make **Claude Code CLI** and **Codex** consume the same repo-native, Rust-owned framework/runtime core across all consuming directories/repositories, not only this framework checkout.

Target steady state:

- one shared Rust framework core;
- two explicit, closed-set host projections:
  - `codex-cli`
  - `claude-code-cli`
- install/status/remove/projection flows distinguish:
  - `framework_root`
  - `project_root`
  - `artifact_root`
  - Claude/Codex host homes
- host-private directories (`~/.codex/**`, `~/.claude/**`, project `.codex/**`, project `.claude/**`) are disposable projection/install targets only;
- framework truth stays in framework-root-native sources and Rust-generated artifacts.

## Non-goals that must stay enforced

Do not introduce or revive:

- `generic-host-adapter`;
- `cli-common-adapter`;
- host-family parity snapshots;
- Python routing/runtime/artifact/host-integration code;
- Node/TypeScript live runtime fallback;
- plugin runtime semantics as framework source of truth;
- `~/.claude/**` or `~/.codex/**` as shared routing, memory, skill, or artifact truth;
- assumptions that `/Users/joe/Documents/skill` is the active project root for every consuming repository.

## Current completed baseline

These items were observed in the current working tree and verified by focused tests.

- [x] `RUNTIME_REGISTRY.json` exposes `framework_core`.
- [x] `RUNTIME_REGISTRY.json` exposes `host_projections`.
- [x] Supported projections are the closed set `codex-cli` and `claude-code-cli`.
- [x] Registry includes Claude Code projection metadata.
- [x] Framework command entrypoints include both Codex dollar entrypoints and Claude slash entrypoints.
- [x] Profile runtime surface uses `host_projection` instead of `codex_host_payload` as the shared-core axis.
- [x] Profile generation builds explicit Codex and Claude Code host payloads.
- [x] Profile bundle emits `codex_profile`, `claude_code_profile`, and `host_payloads`.
- [x] Host integration supports `codex` and `claude-code` as install/status/remove targets.
- [x] Primary host-neutral dispatch exists under `framework host-integration ...`.
- [x] `codex host-integration ...` is retained as a compatibility alias.
- [x] Root resolution supports separate framework, project, artifact, Claude home, and Codex home inputs.
- [x] Claude Code project projection installs a single default `/framework` command-file entrypoint.
- [x] Default Claude Code projection does not manage settings, hooks, or statusLine.
- [x] Claude settings/hooks/statusLine are represented as opt-in capabilities.
- [x] Projection manifests record framework-owned files and managed settings key paths.
- [x] Basic remove skips user-authored projection-like files.
- [x] Retired plugin wrapper files are removed from the working tree.
- [x] Policy tests assert removed plugin/runtime wrappers stay removed.
- [x] Compiler tests assert framework command rows track registry commands.
- [x] Focused tests pass:
  - `cargo test --manifest-path Cargo.toml --test host_integration`
  - `cargo test --manifest-path Cargo.toml --test policy_contracts`
  - `cargo test --manifest-path Cargo.toml --test documentation_contracts`
  - `cargo test --manifest-path scripts/router-rs/Cargo.toml`
  - `cargo test --manifest-path scripts/skill-compiler-rs/Cargo.toml`

## Remaining checklist

### 1. External consuming repository portability

- [x] Add an integration test matrix where `framework_root != project_root`.
- [x] Seed at least:
  - `temp/framework-root`
  - `temp/project-a`
  - `temp/project-b`
  - `temp/home/.claude`
  - `temp/home/.codex`
- [x] Verify Claude Code install into project A writes only under project A and selected artifact root.
- [x] Verify Codex install into project B writes only under project B and selected artifact root.
- [x] Verify status works from a cwd outside both roots when explicit roots are supplied.
- [x] Verify generated project A command resolves the supplied framework root.
- [x] Verify stale/missing framework root fails closed with repair guidance.
- [x] Verify user-scope Claude command resolves `project_root` from invocation context or `SKILL_PROJECT_ROOT`, never from `framework_root`.
- [x] Verify project-scope Claude command moved to another repo fails closed or reports stale project metadata.

### 2. Root discovery conflict hardening

- [x] Add tests for each root fallback chain:
  - explicit CLI flag;
  - environment variable;
  - documented discovery/default;
  - fail-closed missing-root error.
- [x] Verify project markers never resolve `framework_root` unless the candidate is explicitly the framework checkout.
- [x] Verify framework markers never resolve `project_root` unless the checkout is explicitly both framework and project.
- [x] Verify a candidate project containing framework markers but not matching the resolved `framework_root` is treated as ambiguous unless both roots are supplied.
- [x] Verify `--claude-home` and `--codex-home` override `CLAUDE_HOME` and `CODEX_HOME`.
- [x] Verify resolved-root status output always prints `framework_root`, `project_root`, `artifact_root`, and host homes.

### 3. Claude Code native contract fixtures

- [x] Add a native Claude Code command/skill contract fixture for supported project command directories.
- [x] Add a fixture for supported user command directories.
- [x] Add a fixture for supported project/user skill directories if skill-backed entrypoints remain possible.
- [x] Verify `.claude/commands/framework.md` loadability and expected frontmatter shape.
- [x] Define and enforce a generated Claude frontmatter allowlist.
- [x] Reject unknown generated frontmatter keys.
- [x] Verify whether `argument-hint` or any argument field is native-supported before keeping it in generated Claude command files.
- [x] Verify precedence behavior before ever generating both command-file and skill-backed representations for the same logical entrypoint.
- [x] Verify generated Claude files contain only thin bootstrap pointers, not copied `SKILL.md` bodies, routing tables, memory policy, or large registry payloads.

### 4. Claude Code hooks/statusLine safety

- [x] Add a native Claude Code settings contract fixture for hook event names, matcher semantics, blocking behavior, and statusLine behavior.
- [x] Verify `PreToolUse` use against documented/native Claude Code behavior.
- [x] Do not claim prompt/slash-command pre-expansion enforcement unless a native prompt hook event is documented and tested.
- [x] If no native pre-expansion hook exists, report that enforcement as unmanaged instead of inventing an event.
- [x] Verify blocking hooks use supported blocking behavior, including documented exit code or JSON decision semantics.
- [x] Verify `PostToolUse` is never used as a prevention mechanism.
- [x] Verify generated Claude hooks are written only through native settings JSON schema.
- [x] Verify Claude hook install never creates `.codex/hooks.json`.
- [x] Verify `disableAllHooks` effects are reported separately from statusLine effects.
- [x] Verify statusLine effective behavior before claiming any interaction with `disableAllHooks`.
- [x] Reject broad generated `allowed-tools` grants unless explicitly allowlisted by policy and tests.

### 5. Compatibility alias inventory and equivalence

- [x] Keep a machine-readable inventory for every retained compatibility alias:
  - `codex host-integration ...`;
  - `framework host-integration install-skills`;
  - `--repo-root`;
  - any old command/slash/skill aliases retained for migration.
- [x] For each retained alias, record:
  - primary command;
  - compatibility reason;
  - owner;
  - removal condition or explicit kept-indefinitely policy.
- [x] Add normalized-output equivalence tests for `framework host-integration ...` vs `codex host-integration ...`.
- [x] Add normalized-output equivalence tests for `install-skills` vs `install` where retained.
- [x] Verify aliases have no independent parser, root-resolution, install, remove, or status behavior.
- [x] Delete aliases without an explicit compatibility reason.

### 6. Generated artifact drift gate

- [x] Define the canonical regeneration command for checked-in generated artifacts.
- [x] Ensure `configs/framework/GENERATED_ARTIFACTS.json` lists every checked-in generated artifact that should be compared.
- [x] Implement a drift gate that regenerates into a temporary `artifact_root`.
- [x] Compare only manifest-declared checked-in generated artifacts.
- [x] Fail with artifact path and regeneration command when drift is detected.
- [x] Fail on undeclared generated framework artifacts in checked-in paths.
- [x] Ignore concrete install reports unless explicitly listed as fixtures.
- [x] Verify generated host projections do not contain copied full skill bodies or registry payloads beyond allowed bootstrap metadata.
- [x] Verify generated artifacts contain no expanded host-private paths such as `/Users/joe/.claude` or `/Users/joe/.codex` unless the artifact is a concrete install report.
- [x] Verify no generated shared artifact assumes `/Users/joe/Documents/skill` as a consuming `project_root`.

### 7. Projection cleanup subsystem

- [x] Extend remove/cleanup behavior beyond basic scoped remove to cover stale framework-owned projections.
- [x] Cleanup must remove only files with valid framework ownership proof:
  - sidecar projection manifest;
  - generated file metadata;
  - versioned legacy-projection allowlist.
- [x] Cleanup must skip files with framework-like names but no valid ownership marker.
- [x] Cleanup must preserve user-authored files next to framework projections.
- [x] Cleanup must remove obsolete per-command Claude aliases unless current registry/profile policy enables them.
- [x] Cleanup must remove obsolete Codex projections unless explicitly retained as compatibility projections.
- [x] Settings cleanup must remove only exact JSON key paths recorded as framework-owned.
- [x] Cleanup must never delete a whole settings file solely because it contains framework-managed keys.
- [x] Hooks/statusLine cleanup must remove only entries matching framework-owned metadata or manifest paths.
- [x] Cleanup must report every removed path and skipped user-owned path.
- [x] Cleanup must support `--to codex`, `--to claude-code`, `--scope project`, `--scope user`, `--project-root`, `--claude-home`, and `--codex-home`.
- [x] Default cleanup scope must be project-only.
- [x] User-scope cleanup must require explicit `--scope user` and explicit host-home resolution.
- [x] Cleanup must support non-mutating dry-run/report mode.
- [x] Cleanup must be idempotent.

### 8. Codex compatibility projection tightening

- [x] Inventory retained Codex-era generated outputs, especially `codex_profile.codex_host_payload`.
- [x] Prove every retained Codex-era output is derived from shared core/projection state, not independently authored truth.
- [x] Verify default Codex projection creates or preserves exactly one primary root entrypoint such as `$framework` or `$skill`.
- [x] Treat per-command `$gitx`, `$team`, `$deepinterview`, and `$autopilot` as explicit opt-in or compatibility projections.
- [x] Verify Codex hooks remain disabled by default.
- [x] Verify Codex install does not mutate Claude Code files.
- [x] Verify Claude Code install does not mutate Codex files.
- [x] Verify removing one host does not remove the other host.

### 9. Documentation and active-policy cleanup

- [x] Remove or archive any active docs that still describe Codex-only runtime policy as current truth.
- [x] Update generated help/bootstrap text to use `framework host-integration ...` as primary.
- [x] Ensure examples install one host at a time by default, not `--to all`.
- [x] Keep `--to all` documented only as explicit advanced behavior that reports all mutations before applying.
- [x] Ensure active docs say Claude settings are merge-patched, never whole-file overwritten.
- [x] Ensure active docs say Claude hooks/statusLine are native settings-managed opt-in features.
- [x] Ensure active docs say `CLAUDE.md` is context/bootstrap only, not enforcement.
- [x] Ensure active docs say `~/.claude/projects/**/memory` is never framework truth.
- [x] Ensure active docs forbid Python/Node/plugin runtime revival.

### 10. Final verification before acceptance

- [x] Re-run focused tests:
  - `cargo test --manifest-path Cargo.toml --test host_integration`
  - `cargo test --manifest-path Cargo.toml --test policy_contracts`
  - `cargo test --manifest-path Cargo.toml --test documentation_contracts`
  - `cargo test --manifest-path scripts/router-rs/Cargo.toml`
  - `cargo test --manifest-path scripts/skill-compiler-rs/Cargo.toml`
- [x] Run broader Rust test suite if different from the focused commands.
- [x] Run external consuming repo matrix tests.
- [x] Verify no plugin runtime/package path is revived.
- [x] Verify no generic adapter, CLI-common adapter, parity snapshot, Python runtime, or Node runtime appears in active runtime surfaces.
- [x] Verify default Claude Code projection creates exactly one project-local root entrypoint and no per-command aliases.
- [x] Verify default Claude Code install does not modify settings, hooks, statusLine, permissions, env vars, `CLAUDE.md`, or `${CLAUDE_HOME}/projects/**/memory`.
- [x] Verify user-scope Claude Code writes require explicit opt-in and never embed a consuming `project_root`.
- [x] Verify cleanup/install/status cycle is idempotent and preserves user-authored host files.
- [x] Verify no undeclared generated artifacts or temporary install reports are required for a clean install/status/remove cycle.

## Current risk summary

Closeout verification now covers the four prior risk areas with focused host-integration, policy, documentation, router, and compiler tests:

1. external-repository portability when `framework_root != project_root`;
2. Claude Code native command/settings behavior fixtures for command files, hooks, and statusLine reporting;
3. compatibility alias inventory/equivalence and explicit alias lifecycle policy;
4. manifest-owned cleanup plus generated-artifact drift gating.

Remaining risk is limited to future upstream Claude Code native behavior changes; this repo now treats those surfaces as tested native settings/command contracts and reports unmanaged behavior instead of inventing unsupported semantics.
