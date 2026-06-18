# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [v7.0] — 2026-06-18

### Added (2026-06-18 v7 收尾)
- K13: insta snapshot 从 5 扩展到 33 个，覆盖 6 个 crate（core-policy, runtime-core, host-projection, runtime-storage, routing-engine, framework-kernel）
- K13: CI coverage 门控（30% threshold，bash lcov 解析 fallback）
- K6: framework_runtime/mod.rs 提取 5 子模块（snapshot, contract_summary, evidence, closeout, util）从 2K+ 行瘦身到 180 行
- K6: paper hook 类型耦合消除（PaperProseHookHost 改为直接引用 host_projection::hooks）
- K8: codegraph-rs 4 个警告清零（prev_leading, frontmatter_closed, skill_path, make_keyword_id）
- docs/hosts/mimo.md: 从 69 行扩充到 113 行，补齐 6 个板块
- paper_adversarial_hook: 依赖从 `crate::paper_prose_hook::PaperProseHookHost` 改为 `host_projection::hooks::PaperProseHookHost`

### Changed (2026-06-18 v7 收尾)
- framework_runtime/mod.rs: 提取 snapshot/contract_summary/evidence/closeout/util 5 个独立子模块
- roadmap-v7.md: 进度更新至 ~85%（K6 35%→70%, K11 25%→75%, K13 30%→55%, K8 100%）

### Removed (2026-06-18 v7 收尾)
- codegraph-rs: dead 函数 `make_keyword_id` 移除
- research-log-rs: `use std::fmt::Write as FmtWrite` 歧义别名清理

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
