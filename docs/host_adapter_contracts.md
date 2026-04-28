# Shared Host Entrypoint Contract

## Goal

Codex CLI and Codex App share one repo system policy. The runtime emits the
Codex-native `codex_profile` artifact for Codex configuration, and host
entrypoint sync is Codex-only. The supported generated host entrypoints are:

- `codex-cli` reads `AGENTS.md`.
- `codex-app` reads `AGENTS.md`.

## Default Artifact

- `codex_profile`: native Codex profile output for config paths, context files,
  MCP config, resume hints, and framework alias entrypoints.

## Default Surface Boundary

- The default artifact surface is `codex_profile`, generated Codex entrypoints,
  and the sync manifest.
- Host-facing entrypoints are generated projections for discovery and policy
  bootstrap only; `skills/` remains the only live skill source.
- Generated entrypoints and settings are not hand-authored truth. Regenerate
  them through the Rust host-entrypoint sync or host-integration paths.

## Hard Rules

1. `framework_profile` stays Codex-pinned and Codex-private fields stay in
   `codex_profile.codex_host_payload`.
2. `workspace_bootstrap.resources` is the only skills/memory default source.
3. `router-rs codex sync` materializes `AGENTS.md` and
   `.codex/host_entrypoints_sync_manifest.json` for the supported host
   entrypoints.
4. New default artifacts must stay within the single profile plus generated
   host-entrypoint boundary.
5. Non-Codex host integrations are not part of the active repository contract.
