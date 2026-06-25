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

## [v7.1] — 2026-06-25

### Architecture
- 六层→八层运行时模型重构 (L0-L7)
- `framework-runtime` 拆分为 `fr-utils`/`fr-contracts`/`fr-exec` 子 crate
- 新增 `http-util` crate (HTTP 客户端工厂)
- 新增 `tool-routing` crate (tool 路由评分与搜索)
- 彻底移除 JS workflow 脚本与废弃 skill references
- JS workflow (`/workflow`) 废弃清理
- 生命周期四阶段 skill 退场 (`discussx`/`planx`/`implementx`/`verifyx`)
- `my-light` goal profile 删除，区分 linear/loop 模式
- Goal 全周期治理：自动检测、amend、严格退出验证、anti-drift 核查
- 注册表驱动全面推行：所有宿主硬编码从工具层清除

### Host Management
- 移除 MiMo 第 5 宿主，闭集收敛为 4 宿主 (`cursor`/`claude`/`opencode`/`codex`)
- `router-rs-cli`→`router-rs` 二进制回退兼容层清除
- 删除废弃 `per-host` agent 模块与 `host_extensions`
- 宿主统一 verify + 工具层运行时逻辑清除 + `schema_drift` 泛化

### Security
- 运行时安全加固：6 项 Critical 级审计修复
- MCP `dispatch_tool` 区分"未注册"与"执行失败"
- SSRF 防护回归

### Governance & Code Quality
- 多轮 clippy 治理：200→0 清理，unwrap/expect 严格化
- `cargo test --workspace` 编译修复（7 处编译器错误）
- 清除 11 处死函数/废弃导入/未使用变量（12 编译警告清零）
- `core-policy`: `hook_common` 拆分子模块，`tool_safety_rules` 死代码清除
- 4 阶段字段兼容层 + 死/重复字段清理 + env flag 合并
- 对抗审核修复：`goal_state` 共享读取、`SKILL.md` network_access 等
- `research-harness`: 合并 `extract_content_words` + 加固 claim 覆盖度算法
- `browser-mcp` 独立特征门控
- `closeout record` 原子写入 + compaction 修复

### Testing
- 新增 `cargo-fuzz` 目标 (MCP/stdio/hook)
- `insta` snapshot 从 5 扩展到 33 个
- 测试覆盖完成（W4 阶段）
- CodeGraph 深度集成：skill prompt 指引 + routing signal + hook 软告警

### Documentation
- 注册表驱动重构全面文档适配
- ADR 更新 + AGENTS.md 六层→八层运行时模型同步
- 文档体系科学化重构 + `last_verified` 日期治理
- codegraph 支持 `.md` 索引
- 文档体系维护 + `research-harness` KG 专章

### Config
- 注册表驱动配置全面更新 + 路由索引 `skill_flags` 对齐
- `RUNTIME_REGISTRY.json` 缓存优化 + 路由引擎拆分

Baseline for v7. See `artifacts/current/system-evolution-roadmap-v6.md` for v6.x history.
