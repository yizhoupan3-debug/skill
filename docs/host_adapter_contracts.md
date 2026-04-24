# Codex Profile Contract

## Goal

Codex is the only supported host. The runtime emits one Codex profile artifact:
`codex_adapter`. That JSON key remains for compatibility, but it is not a
multi-host adapter abstraction.

## Default Artifact

- `codex_adapter`: native Codex profile output for config paths, context files,
  MCP config, resume hints, and framework alias entrypoints.

## Removed From Default Surface

- No `cli_common_adapter`.
- No `codex_desktop_adapter`.
- No `codex_cli_adapter`.
- No `cli_family_parity_snapshot`.
- No non-Codex host projection.
- No compatibility inventory as a default artifact.

## Hard Rules

1. `framework_profile` stays Codex-pinned and Codex-private fields stay in
   `codex_adapter.host_adapter_payload`.
2. `workspace_bootstrap.bridges` is the only bridge default source.
3. `router-rs --profile-json` and `--profile-artifacts-json` emit Codex-only
   profile surfaces.
4. New default artifacts must not recreate multi-host, parity, alias, Python,
   or fallback host layers.
