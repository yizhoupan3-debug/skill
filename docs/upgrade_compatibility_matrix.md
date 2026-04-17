# Upgrade Compatibility Matrix

| Adapter | Lifecycle | Host | Transport | Requires aionrs | Core runtime | Memory | Artifact | Orchestration | Notes |
|---|---|---|---|---:|---:|---:|---:|---:|---|
| `codex_common_adapter` | shared canonical | Codex shared contract | `host-neutral-contract` | No | Yes | Yes | Yes | Yes | Shared projection layer for Desktop / CLI; keeps framework core as the only controller |
| `generic_host_adapter` | fallback baseline | generic | `inproc` | No | Yes | Yes | Yes | Yes | Portable fallback baseline; keeps the outer-framework runtime surface alive |
| `codex_desktop_adapter` | canonical desktop | Codex Desktop | `local-bridge` | No | Yes | Yes | Yes | Yes | Primary interactive desktop entrypoint and the desktop parity identity |
| `codex_desktop_host_adapter` | temporary alias | Codex Desktop | `local-bridge` | No | Yes | Yes | Yes | Yes | Compatibility bridge only; mirrors `codex_desktop_adapter`, stays opt-in for continuity lanes, and remains a retirement candidate |
| `codex_cli_adapter` | canonical headless | codexcli | `headless-exec` | No | Yes | Yes | Yes | Yes | Formal headless entrypoint for batch / cron / CI without becoming framework truth |
| `aionui_host_adapter` | legacy debt | AionUI host shell | `bridge-contract` | No | Yes | Yes | Yes | Yes | Upstream-facing compatibility surface only; not a forward roadmap anchor |
| `aionrs_companion_adapter` | legacy debt | aionrs companion sidecar | `stdio-jsonl` | Optional companion | Yes | Optional | Yes | Optional | Companion integration only; deep adaptation, not deep fork |

## Upgrade Policy

1. Upgrade framework contract first.
2. Re-run adapter compatibility validation against all hosts.
3. Keep Codex common / Desktop / CLI and the parity snapshot green before enabling any legacy companion path.
4. Treat `aionrs` and `AionUI` as upstream dependencies, not writable framework internals.
5. Any new host must prove portability against `framework_profile` before gaining host-specific extensions.
6. Resolve `host_capability_requirements` from `framework_profile` before introducing host-specific branching.
7. Use Desktop / CLI parity snapshots as the primary dual-entry regression artifact; treat this matrix as a secondary inventory view.
8. `codex_desktop_host_adapter` may exist only as a mirror alias and must never receive host-only semantics that bypass `codex_desktop_adapter`.
9. Do not let `codexcli` become controller truth while shrinking legacy alias or companion surfaces.

## Alias Exit Gates

`codex_desktop_host_adapter` can only move toward retirement when all of the
following are true:

1. Downstream callers, docs, and emitted artifact references have migrated to
   `codex_desktop_adapter` as the canonical desktop identity.
2. `codex_dual_entry_parity_snapshot` remains green without alias-specific
   semantics.
3. Rust and Python artifact emitters can drop the alias without reintroducing
   `aionrs` / `AionUI` mainline assumptions.
4. Any last-mile translation shim is edge-local compatibility code, not a new
   framework controller or contract truth source.

## Current Status

- `framework_profile` contract: implemented in outer framework.
- `codex_common_adapter`: shared Codex contract projection is now implemented.
- `codex_desktop_adapter`: canonical interactive desktop adapter is now implemented.
- `codex_desktop_host_adapter`: compatibility alias remains available only for explicit continuity / compatibility lanes.
- `codex_cli_adapter`: headless Codex entrypoint projection is now implemented.
- `aionrs_companion_adapter`: outer-framework companion projection is contract-scoped legacy debt.
- `AionUI host adapter`: outer-framework host projection is contract-scoped legacy debt.
- legacy Codex Desktop alias surface: contract-scoped compatibility debt only; not a first-class Desktop output target.
- `build_codex_dual_entry_parity_snapshot(...)`: Desktop / CLI shared-contract parity is emitted as the first-class dual-entry artifact.
- `codex_desktop_alias_inventory.json`: current repo-side alias references are inventoried so retirement stops depending on ad-hoc grep.
- `codex_desktop_alias_retirement_status.json`: alias retirement gates are externalized as a contract instead of staying implicit in docs only.
- `build_upgrade_compatibility_matrix(...)`: the upgrade lane is anchored in the outer-framework contract, not host internals.
- `emit_framework_contract_artifacts(...)`: Python can now emit concrete bridge/contract artifacts for profile + adapters + matrix + dual-entry parity snapshot + the first-class control-plane contract artifacts `execution_controller_contract`, `delegation_contract`, and `supervisor_state_contract`.
- default artifact emission is parity-first: `codex_desktop_host_adapter` is now
  legacy opt-in output.
- default package export is parity-first: `compile_codex_desktop_host_adapter(...)`
  no longer lives on `codex_agno_runtime` root and is only exposed from
  `codex_agno_runtime.compatibility`.
- `router-rs --profile-json`: Rust lane validates and compiles the framework profile without touching `aionrs` or `AionUI`.
