# Upgrade Compatibility Matrix

This file is historical smoke documentation, not framework truth.

## Current Default Profile Artifact

| Artifact key | Lifecycle | Host | Transport | Default artifact |
|---|---|---|---|---:|
| `codex_adapter` | canonical compatibility key | Codex | `native-codex` | Yes |

## Removed Or Retired

| Surface | Status | Replacement |
|---|---|---|
| `cli_common_adapter` | Removed | `codex_adapter` |
| `codex_desktop_adapter` | Removed | `codex_adapter` |
| `codex_cli_adapter` | Removed | `codex_adapter` |
| `cli_family_parity_snapshot` | Removed | no parity layer |
| Non-Codex host projections | Retired | none |

## Upgrade Policy

1. Upgrade `framework_profile` and `codex_adapter` only.
2. Do not add host-specific branches to framework truth.
3. Do not reintroduce compatibility inventory or alias artifacts as default outputs.
