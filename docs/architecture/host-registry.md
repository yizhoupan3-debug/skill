# Host Registry Architecture

> **Source of truth**: `configs/framework/RUNTIME_REGISTRY.json`
> **Schema version**: `framework-runtime-registry-v2`

## Overview

The host registry defines all supported AI coding hosts (Cursor, Claude, OpenCode, Codex)
and their configuration in a single JSON file. Adding or removing a host requires changes to
this file plus a Rust provider module — no other Rust code needs modification.

## Schema v2 Structure

```jsonc
{
  "schema_version": "framework-runtime-registry-v2",
  "framework_core": {
    "authority": "rust",
    "host_policy": "closed-set-explicit-projections"
  },
  "host_targets": {
    "supported": ["cursor", "claude-code", "opencode", "codex"],
    "metadata": {
      "<host_id>": {
        // ── Existing fields (v1) ──
        "install_tool": "string",
        "projection_status": "implemented | experimental",
        "installable": true,
        "default_framework_command": "implementx",
        "host_entrypoints": "string | string[]",

        // ── New in v2 ──
        "display_name": "Human-readable name",
        "transport_type": "cursor-agent | anthropic-claude-code | native-opencode | native-codex",
        "config_format": "json | toml | mdc",
        "config_path": ".<host>/settings.json",
        "cli_aliases": ["alias1", "alias2"],
        "home_env_var": "HOST_HOME",
        "default_home_dir": ".<host>"
      }
    },
    "host_providers": { /* Rust module bindings */ }
  },
  "host_projections": { /* Runtime configuration per host */ }
}
```

## Host Transport Types

| Host | Transport | Hook Mechanism |
|------|-----------|---------------|
| Cursor | `cursor-agent` | Shell hook → `router-rs cursor hook` |
| Claude | `anthropic-claude-code` | Shell hook → `router-rs claude hook` |
| OpenCode | `native-opencode` | Shell hook → `router-rs opencode hook` |
| Codex | `native-codex` | Shell hook → `router-rs codex hook` |

All four hosts use the same Rust `HostHookDispatcher` trait implementation.
Hook launchers live in `configs/framework/<host>-router-rs-hook.sh`.

## Adding a New Host

1. Add entry to `RUNTIME_REGISTRY.json`:
   - `host_targets.supported` array
   - `host_targets.metadata.<host_id>` with all v2 fields
   - `host_targets.host_providers.<host_id>` (Rust module bindings)
   - `host_projections.<host_id>` (runtime config)

2. Create Rust provider: `core/host-projection/src/hosts/<host>_provider.rs`
   - Implement `HostLifecycle`, `HostTelemetry`, `HostProvider` traits

3. Create Rust hook dispatcher: `core/host-projection/src/hosts/<host>_hooks.rs`
   - Implement `HostHookDispatcher` trait (7 event handlers)

4. Create hook launcher: `configs/framework/<host>-router-rs-hook.sh`

5. Register in `core/host-projection/src/hosts/mod.rs`

6. Add CLI subcommand in `core/router-rs/src/`

No changes to shared infrastructure (`hook_dispatch.rs`, `core-policy`, `mcp_stdio_harness`).

## Key Invariants

- `host_targets.supported` length == number of `metadata` entries == number of `host_providers` entries
- Every host in `supported` must have `has_native_hook: true` (enforced by `host_provider.rs` test)
- `transport_type` in metadata must match `transport` in `host_projections`
- Schema version is validated strictly at load time (hard error on mismatch)
