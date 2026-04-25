# Codex Profile Contract

## Goal

Codex is the only supported host. The runtime emits one Codex profile artifact:
`codex_profile`. It is the canonical Codex-only profile surface.

## Default Artifact

- `codex_profile`: native Codex profile output for config paths, context files,
  MCP config, resume hints, and framework alias entrypoints.

## Removed From Default Surface

- No generic CLI common adapter artifact.
- No Codex Desktop adapter artifact.
- No Codex CLI adapter artifact.
- No `cli_family_parity_snapshot`.
- No non-Codex host projection.

## Hard Rules

1. `framework_profile` stays Codex-pinned and Codex-private fields stay in
   `codex_profile.codex_host_payload`.
2. `workspace_bootstrap.resources` is the only skills/memory default source.
3. `router-rs profile emit` and `profile artifacts` emit Codex-only
   profile surfaces.
4. New default artifacts must not recreate multi-host, parity, alias, Python,
   or fallback host layers.
