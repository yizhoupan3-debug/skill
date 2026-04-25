# Codex Profile Contract

## Goal

Codex is the only supported host. The runtime emits one Codex profile artifact:
`codex_profile`. It is the canonical Codex-only profile surface.

## Default Artifact

- `codex_profile`: native Codex profile output for config paths, context files,
  MCP config, resume hints, and framework alias entrypoints.

## Removed From Default Surface

- No `cli_common_adapter`.
- No `codex_desktop_adapter`.
- No `codex_cli_adapter`.
- No `cli_family_parity_snapshot`.
- No non-Codex host projection.

## Hard Rules

1. `framework_profile` stays Codex-pinned and Codex-private fields stay in
   `codex_profile.codex_host_payload`.
2. `workspace_bootstrap.resources` is the only skills/memory default source.
3. `router-rs --profile-json` and `--profile-artifacts-json` emit Codex-only
   profile surfaces.
4. New default artifacts must not recreate multi-host, parity, alias, Python,
   or fallback host layers.
