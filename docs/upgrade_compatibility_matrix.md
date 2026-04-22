# Upgrade Compatibility Matrix

This document is a secondary compatibility inventory / smoke view.
It is not framework truth, not the primary regression baseline, and not the
place where runtime or host authority gets redefined. Primary regression still
lives in parity snapshots and first-class shared contract artifacts.

| Adapter | Lifecycle | Host | Transport | Requires aionrs | Core runtime | Memory | Artifact | Orchestration | Notes |
|---|---|---|---|---:|---:|---:|---:|---:|---|
| `codex_common_adapter` | compatibility view | Codex shared contract | `host-neutral-contract` | No | Yes | Yes | Yes | Yes | Codex naming compatibility view over shared contract projection; does not replace `cli_common_adapter` as the canonical shared contract |
| `generic_host_adapter` | fallback baseline | generic | `inproc` | No | Yes | Yes | Yes | Yes | Portable fallback baseline; keeps the outer-framework runtime surface alive |
| `codex_desktop_adapter` | canonical desktop | Codex Desktop | `local-bridge` | No | Yes | Yes | Yes | Yes | Primary interactive desktop entrypoint and the desktop parity identity |
| `codex_desktop_host_adapter` | temporary alias | Codex Desktop | `local-bridge` | No | Yes | Yes | Yes | Yes | Compatibility bridge only; mirrors `codex_desktop_adapter`, stays opt-in for continuity lanes, is omitted from default artifact emission, and remains a retirement candidate |
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
8. Treat `cli_common_adapter` as the canonical CLI-family shared contract; `codex_common_adapter` remains a Codex compatibility naming view only.
9. `codex_desktop_host_adapter` may exist only as a mirror alias and must never receive host-only semantics that bypass `codex_desktop_adapter`.
10. Do not let `codexcli` become controller truth while shrinking legacy alias or companion surfaces.

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
- `cli_common_adapter`: canonical CLI-family shared contract projection is now implemented.
- `codex_common_adapter`: Codex compatibility naming view over the shared contract projection is now implemented.
- `codex_desktop_adapter`: canonical interactive desktop adapter is now implemented.
- `codex_desktop_host_adapter`: compatibility alias remains available only for explicit continuity / compatibility lanes and is not part of the default artifact emission surface.
- `codex_cli_adapter`: headless Codex entrypoint projection is now implemented.
- `aionrs_companion_adapter`: outer-framework companion projection is contract-scoped legacy debt.
- `AionUI host adapter`: outer-framework host projection is contract-scoped legacy debt.
- legacy Codex Desktop alias surface: contract-scoped compatibility debt only; not a first-class Desktop output target.
- `build_codex_dual_entry_parity_snapshot(...)`: Desktop / CLI shared-contract parity is emitted as the first-class dual-entry artifact.
- `build_cli_family_parity_snapshot(...)`: CLI-family shared-contract parity is emitted as the canonical CLI regression artifact.
- `codex_desktop_alias_inventory.json`: current repo-side alias references are inventoried only on explicit continuity runs.
- `codex_desktop_alias_retirement_status.json`: alias retirement gates are externalized as a contract only on the explicit continuity lane.
- `codex_agno_runtime.compatibility.build_upgrade_compatibility_matrix(...)`: the upgrade lane is anchored in the outer-framework contract, not host internals.
- `emit_framework_contract_artifacts(...)`: Python can now emit concrete bridge/contract artifacts for profile + default host adapters + dual-entry parity snapshot + the first-class control-plane contract artifacts `execution_controller_contract`, `delegation_contract`, and `supervisor_state_contract`; default outputs land under `default/`, fallback host artifacts under `fallback/`, legacy alias inventory/status under `continuity/`, and `upgrade_compatibility_matrix` itself is now an explicit compatibility-inventory output.
- default Python artifact emission is parity-first: `codex_desktop_host_adapter` no longer has a
  direct Python-emitted artifact, and its inventory/status artifacts stay behind explicit continuity opt-in.
- default Rust `--profile-artifacts-json` is now parity-first too: `codex_desktop_alias_retirement_status`
  stays behind explicit continuity opt-in together with the legacy alias artifact.
- default regression authority is parity-first: this matrix stays secondary
  inventory / smoke evidence and does not replace parity snapshots.
- default package export is parity-first: Python no longer exposes
  `compile_codex_desktop_host_adapter(...)`; the remaining compatibility surface stays limited to
  explicit inventory / retirement helpers.
- `router-rs --profile-json`: Rust lane validates and compiles the framework profile without touching `aionrs` or `AionUI`.
