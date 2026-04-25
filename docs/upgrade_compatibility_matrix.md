# Upgrade Compatibility Matrix

This file is historical smoke documentation, not framework truth.

## Current Default Profile Artifact

| Artifact key | Lifecycle | Host | Transport | Default artifact |
|---|---|---|---|---:|
| `codex_profile` | canonical compatibility key | Codex | `native-codex` | Yes |

## Removed Or Retired

| Surface | Status | Replacement |
|---|---|---|
| Generic CLI common adapter artifact | Removed | `codex_profile` |
| Codex Desktop adapter artifact | Removed | `codex_profile` |
| Codex CLI adapter artifact | Removed | `codex_profile` |
| `cli_family_parity_snapshot` | Removed | no parity layer |
| Non-Codex host projections | Retired | none |

## Upgrade Policy

1. Upgrade `framework_profile` and `codex_profile` only.
2. Do not add host-specific branches to framework truth.
3. Do not reintroduce compatibility inventory or alias artifacts as default outputs.
4. Legacy top-level router flags have moved to canonical subcommands and now fail with migration guidance when used as live entrypoints.
