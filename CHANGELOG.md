# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [v7.0] — 2026-06-14

### Added
- Registry Schema v2: per-host metadata (display_name, transport_type, config_format, cli_aliases, home_env_var, default_home_dir)
- OpenCode Rust native hook: unified with cursor/claude/codex via `HostHookDispatcher` trait
- `configs/framework/opencode-router-rs-hook.sh`: hook launcher script
- `clippy.toml`: too-many-arguments-threshold=8, cognitive-complexity-threshold=30
- 10 `#[tokio::test]` async tests for MCP dispatch paths

### Changed
- OpenCode `transport_type`: `"opencode-plugin"` → `"native-opencode"`
- `OPENCODE_HOOKS_PATH`: `.opencode/plugins/` → `.opencode/hooks.json`
- `schema_version`: `"framework-runtime-registry-v1"` → `"framework-runtime-registry-v2"`
- runtime-core: removed crate-level `#![allow(unused_variables, unused_mut)]`

### Removed
- `McpCloseoutGateVerdict` dead fields (all_clear, checkpoint_only, hard_block)
- OpenCode `hooks_manifest_path` override (now uses default, matching Claude)

## [v6.5] — 2026-06-12

Baseline for v7. See `artifacts/current/system-evolution-roadmap-v6.md` for v6.x history.
