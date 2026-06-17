# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [v7.0] — 2026-06-15

### Added
- Registry Schema v2: per-host metadata (display_name, transport_type, config_format, cli_aliases, home_env_var, default_home_dir) for all 5 hosts
- `ROUTER_RS_MIMO_REVIEW_GATE_DISABLE` env var for MiMo review gate control
- `HookObservationHost` and `PaperProseHookHost` enums: added OpenCode + MiMo variants
- `insta` snapshot testing framework: first snapshot test for routing scoring output
- `host-mimo` feature in router-rs Cargo.toml and CI feature matrix
- `tracing` + `tracing-subscriber` workspace dependencies: structured logging infrastructure
- `tracing::instrument` on `review_gate_armed`, `dispatch_hook_command`, `dispatch_agent_command`, `dispatch_host_command` + debug spans on routing scoring
- Dispatch table pattern: `dispatch_hook_command`/`dispatch_agent_command`/`verify`/`install`/`status`/`remove` all converted from match to const TABLE lookup
- `FrameworkError` enum in `core-policy::error` (11 variants: Io, Json, Config, Registry, Hook, Mcp, Session, Path, Validation, NotFound, Unsupported)
- `thiserror` 2.0 workspace dependency
- host-projection tracing: `hook_dispatch::dispatch()` + `file_state_lock` load/save debug spans
- runtime-core tracing: `telemetry_emit` route decision + `closeout_enforcement` evaluate + `execution_contract` bundle + `session_supervisor` operation + `hook_timing` emit + `framework_runtime` snapshot/contract spans
- Final deep review: clippy 0 errors, 1966 tests pass, 0 backup files, 0 stale references

### Changed
- OpenCode `transport_type`: `"opencode-plugin"` → `"native-opencode"`
- `OPENCODE_HOOKS_PATH`: `.opencode/plugins/` → `.opencode/hooks.json`
- `schema_version`: `"framework-runtime-registry-v1"` → `"framework-runtime-registry-v2"`
- runtime-core: removed crate-level `#![allow(unused_variables, unused_mut)]`
- `json_payload.rs` merged into `json_value.rs` (json_payload deleted, functions deduplicated)
- Documentation: "四宿主" → "五宿主" across README, docs, specs, skills
- Documentation: all `last_verified` dates refreshed to 2026-06-15
- Documentation: removed stale `roadmap-v5-exec` references from b10/b11 docs
- CI: feature matrix now includes `host-mimo`

### Removed
- `McpCloseoutGateVerdict` dead fields (all_clear, checkpoint_only, hard_block)
- OpenCode `hooks_manifest_path` override (now uses default, matching Claude)
- Ghost reference to `docs/hosts/claude-desktop.md` in specs
- `framework_runtime/json_payload.rs` (merged into json_value.rs)
- `cursor_hooks/tests_review_gate.rs` 5254-line monolith (split into 3 sub-modules)

## [v6.5] — 2026-06-12

Baseline for v7. See `artifacts/current/system-evolution-roadmap-v6.md` for v6.x history.
