# Upgrade Compatibility Matrix

This document is a secondary compatibility inventory / smoke view.
It is not framework truth, not the primary regression baseline, and not the
place where runtime or host authority gets redefined. Primary regression still
lives in parity snapshots and first-class shared contract artifacts.

| Adapter | Lifecycle | Host | Transport | Requires aionrs | Core runtime | Memory | Artifact | Orchestration | Notes |
|---|---|---|---|---:|---:|---:|---:|---:|---|
| `codex_common_adapter` | compatibility view | Codex shared contract | `host-neutral-contract` | No | Yes | Yes | Yes | Yes | Codex naming compatibility view over shared contract projection; does not replace `cli_common_adapter` as the canonical shared contract |
| `generic_host_adapter` | retired fallback baseline | generic | `inproc` | No | Yes | Yes | Yes | Yes | Retired fallback surface; not emitted by Python artifact generation |
| `codex_desktop_adapter` | canonical desktop | Codex Desktop | `local-bridge` | No | Yes | Yes | Yes | Yes | Primary interactive desktop entrypoint and the desktop parity identity |
| `codex_desktop_host_adapter` | temporary alias | Codex Desktop | `local-bridge` | No | Yes | Yes | Yes | Yes | Compatibility bridge only; mirrors `codex_desktop_adapter`, stays opt-in for continuity lanes, is omitted from default artifact emission, and remains a retirement candidate |
| `codex_cli_adapter` | canonical headless | codexcli | `headless-exec` | No | Yes | Yes | Yes | Yes | Formal headless entrypoint for batch / cron / CI without becoming framework truth |
| `aionui_host_adapter` | retired legacy debt | AionUI host shell | `bridge-contract` | No | Yes | Yes | Yes | Yes | Compatibility inventory row only; not emitted as a fallback artifact |
| `aionrs_companion_adapter` | retired legacy debt | aionrs companion sidecar | `stdio-jsonl` | Optional companion | Yes | Optional | Yes | Optional | Compatibility inventory row only; not emitted as a fallback artifact |

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
3. Rust artifact emission can drop the alias without reintroducing `aionrs` /
   `AionUI` mainline assumptions; Python must not keep a second emitter alive.
4. Any last-mile translation shim is edge-local compatibility code, not a new
   framework controller or contract truth source.

## Current Status

- `framework_profile` contract: implemented in outer framework.
- `cli_common_adapter`: canonical CLI-family shared contract projection is now implemented.
- `codex_common_adapter`: Codex compatibility naming view over the shared contract projection is now implemented.
- `codex_desktop_adapter`: canonical interactive desktop adapter is now implemented.
- `codex_desktop_host_adapter`: compatibility alias remains available only for explicit continuity / compatibility lanes and is not part of the default artifact emission surface.
- `codex_cli_adapter`: headless Codex entrypoint projection is now implemented.
- `aionrs_companion_adapter`: retired compatibility inventory row only.
- `AionUI host adapter`: retired compatibility inventory row only.
- legacy Codex Desktop alias surface: contract-scoped compatibility debt only; not a first-class Desktop output target.
- `build_codex_dual_entry_parity_snapshot(...)`: Desktop / CLI shared-contract parity is emitted as the first-class dual-entry artifact.
- `build_cli_family_parity_snapshot(...)`: CLI-family shared-contract parity is emitted as the canonical CLI regression artifact.
- `codex_desktop_alias_inventory.json`: retired; no longer emitted by Python artifact generation.
- `codex_desktop_alias_retirement_status.json`: alias retirement gates are externalized as a contract only on the explicit continuity lane.
- `upgrade_compatibility_matrix`: compiled by Rust `--profile-artifacts-json`; Python no longer owns the matrix truth.
- `emit_framework_contract_artifacts(...)`: calls Rust and writes Rust-owned bridge/contract artifacts. It no longer emits fallback host artifacts, no longer emits alias inventory, and no longer writes a Python/Rust parity report.
- default Python artifact emission is Rust-first: `codex_desktop_host_adapter` no longer has a direct Python-emitted artifact, and alias retirement status stays behind explicit Rust continuity opt-in.
- default Rust `--profile-artifacts-json` is now parity-first too: `codex_desktop_alias_retirement_status`
  stays behind explicit continuity opt-in together with the legacy alias artifact.
- default regression authority is parity-first: this matrix stays secondary
  inventory / smoke evidence and does not replace parity snapshots.
- default package export is parity-first: Python no longer exposes
  `compile_codex_desktop_host_adapter(...)`; the remaining compatibility surface stays limited to
  explicit inventory / retirement helpers.
- the old retired-package compatibility shim is gone; internal artifact emitters now call the
  canonical runtime modules directly instead of bouncing through an extra escape hatch.
- `router-rs --profile-json`: Rust lane validates and compiles the framework profile without touching `aionrs` or `AionUI`.
