# Shared Host Projection Contract

## Goal

Codex CLI and Claude Code CLI consume one Rust-owned framework core. Host
integration is a projection/install layer only; routing ownership, memory
policy, continuity state, and artifact contracts remain framework-root-native
and Rust-generated.

- `codex-cli` reads `AGENTS.md`.
- `claude-code-cli` reads `CLAUDE.md` and project-local `.claude/commands/*.md`
  projections when installed.

## Default Artifact

- `framework_core`: shared Rust-owned framework contract.
- `host_projections.codex-cli`: Codex projection metadata.
- `host_projections.claude-code-cli`: Claude Code projection metadata.
- `codex_profile`: native Codex projection output for Codex config paths,
  context files, MCP config, resume hints, and framework alias entrypoints.
- `claude_code_profile`: native Claude Code projection output for Claude
  command/settings/status surfaces.

## Default Surface Boundary

- The default artifact surface is the shared core plus the two explicit host
  projections.
- Host-facing entrypoints are generated projections for discovery and policy
  bootstrap only; `skills/` remains the only live skill source.
- Generated entrypoints and settings are not hand-authored truth. Regenerate
  them through the Rust host-entrypoint sync or `framework host-integration`
  paths.
- Default Claude Code install writes exactly one project-local root command,
  `.claude/commands/framework.md`, and leaves settings, hooks, statusLine,
  permissions, environment variables, `CLAUDE.md`, and user-scope files
  unmanaged unless explicitly enabled.
- Opt-in Claude settings writes are JSON merge/patch operations. They preserve
  unrelated keys and record framework-owned key paths in
  `.claude/.framework-projection.json`.
- Claude status reports `disableAllHooks` conflicts for hooks, but does not
  claim any statusLine interaction with `disableAllHooks` without native
  verification.
- Removal and cleanup may delete only files with framework ownership metadata
  or settings keys recorded in the projection manifest. User-authored command
  files and unrelated settings keys must be skipped.
- Compatibility aliases are machine-reported through
  `framework host-integration compatibility-aliases`; each retained alias must
  name its primary command, owner, reason, kept policy, removal condition, and
  `independent_behavior: false`.
- Checked-in generated artifacts are declared by
  `configs/framework/GENERATED_ARTIFACTS.json` and can be inspected through
  `framework host-integration generated-artifacts-status`; this manifest-backed
  drift gate regenerates declared artifacts in an isolated temporary root,
  validates schema `framework-generated-artifacts-manifest-v1`, byte-compares
  manifest-declared outputs, and reports undeclared generated framework artifacts
  across reverse-reference surfaces or forbidden expanded host-private paths.

## Hard Rules

1. `framework_profile` stays pinned to the shared Rust core; host-private fields
   stay under explicit host projection payloads.
2. `workspace_bootstrap.resources` is the only skills/memory default source.
3. `router-rs framework host-integration status/install/remove` is the primary
   host-neutral command path; `codex host-integration` is a compatibility alias
   only.
4. New default artifacts must stay within the shared core plus explicit
   `codex-cli` and `claude-code-cli` projection boundary.
5. Non-explicit host projections, generic adapters, CLI-common adapters, parity
   snapshots, Python runtime fallbacks, Node runtime fallbacks, and plugin
   runtime truth are regressions.
6. Generated host projections must not copy full skill bodies, routing tables,
   memory policy, or registry payloads into host-private directories.
