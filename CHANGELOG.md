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

## [v7.2] — 2026-06-29

### Task Engine / Goal Management / Exit Gates — 对抗性 Review 全修复（46 项 findings 闭环）

#### Security 修复
- **P0-001**: D5 证据门禁绕过 — `validate_complete_transition` 改为直接检查 GOAL_STATE.json，不依赖 TASK_POINTERS.tasks 数组（该数组在 set_focus=false 时为空洞）
- **P2-003**: `task_create` 幂等性间隙 — 在 set_task_focus 前检查 TASK_POINTERS.tasks 防止指针漂移

#### 行为修复
- **P0-002**: `drive_until_done` 默认值统一为 `true`（MCP/CLI 路径一致）
- **P1-001**: `closeout_record_write` 失败记录写入 `.failed.json` 后缀，不污染验证路径
- **P1-002**: 嵌套 flock 锁 — `tool_task_complete` goal 路径移至锁外调用 `framework_goal_drive`
- **P1-004**: `max_iterations` 重试活锁 — 阻断前先递增 iteration_count，retry 可正常通过
- **P1-005**: `resume_goal_running` 添加 running/blocked/review_pending 状态守卫
- **P1-008**: `max_iterations` 检查移至 QG 之前，防止 QG 持续阻断导致迭代上限守卫失效
- **P2-001**: `tool_task_complete` 锁内重解析 task_id 修复 TOCTOU
- **P2-004**: `chain_advance` 区分 loop_goal/non-loop（检查 goal_type），不再误报 loop_goal_skipped
- **P2-005**: runner QG 场景从硬编码 `"general"` 改为读取 goal state 的 scene 字段
- **P2-010**: `verify_evidence_index` 检查 `has_successful_verification` 而非仅检查数组非空
- **P2-014**: runner QG/closeout gate 默认启用（`unwrap_or(true)`）
- **P2-015**: `clear_goal_state` 添加 stale guard，与会话隔离机制一致
- **P2-016**: `set_terminal_flags` 阻止 paused→blocked/blocked→paused 跨状态转换
- **P2-017**: 防止 `drive_until_done=true` + `requires_completion_evidence=false` 矛盾配置
- **P2-020**: amend 支持 `drive_until_done` 更新（含合约再验证）

#### Gate 完整性修复
- **P1-003**: QG 自动触发在 hooks 未注册时添加 `tracing::warn!` 日志警告
- **P1-006**: runner GOAL_STATE 心跳同步错误从 warn 升级为 Err 传播，防止静默发散
- **P1-007**: runner QG hook 错误统一为 fail-closed（降级 aggregate 为 "fail"）
- **P2-006**: stdio_dispatch QG 路径传入 `tokio::runtime::Handle`
- **P2-007**: QGEntry 自证证据警告纳入 `GateVerdict.advisories`（原仅 tracing::warn!）
- **P2-008**: `evaluate_closeout_gate_hook` 支持 `reviewer_lane`/`fork_context` 转发
- **P2-012**: closeout hook 路径添加 checkpoint-only 三级区分（PASS/ADVISORY-checkpoint/ADVISORY-general）

#### 互操作修复
- **P2-002**: `sync_task_pointers_after_goal_drive` 合并为单次原子写入
- **P2-013**: 自证证据处理三路径统一（QGEntry/closeout_tool/closeout_hook）
- **P2-019**: `set_terminal_flags` 增加 completed/archived 守卫

#### 清理与维护
- 全 workspace `cargo check --workspace --all-targets` 零 error 零 warning
- 移除 dead function `write_focus_task_pointer_minimal`
- 清理 96MB codegraph 缓存 + 6 个跟踪的测试生成文件
- .gitignore 更新：`scratch/`, `workflows/`, `codegraph/`, `research-log/`, `last_idle_trigger`

## [v7.1] — 2026-06-25

### Tool Routing — 全面对抗审计重构（29 项 findings 闭环）

#### Bug 修复
- `input_schema_json` continue bug 修复（对象类型列在 v1 解析器中被错误跳过）
- Host 过滤 + 模糊救援交互缺陷修复（模糊救援阶段绕过 host 过滤）
- `display_name` 双重加分修复（Step 3 独立得分与 Step 5 alias 机制重叠）

#### 基础设施提取
- `mcp-tool-registry` + `tool-routing-engine` hooks 合并到 `routing-core::config_hooks`，消除两套 OnceLock 系统（L9 fallback 层级清除）
- `McpToolRegistryError` 死代码模块删除，统一 `Result<..., String>`
- `MAX_QUERY_LEN` 重复常量清除 + `signal/tooling.rs` 硬编码正则清理
- `McpToolDecision` 替代 `ToolSearchResult`，消除冗余类型

#### 数据模型升级
- `MCP_TOOL_REGISTRY.json` 升级 v2 对象格式（columnar→object），54 工具全量迁移
- `gate` 字段 `#[doc(hidden)]` 标注；`host_platforms` 语义变更为空数组=通配所有平台
- 评分引擎权重外部化到 `configs/tool_scoring_weights.json` + 注释文档化

#### 算法对齐
- `search_tools` 增加 fuzzy rescue fallback（trigram Jaccard 匹配），与 `route_tool` 行为对齐
- CJK tokenizer 补充 7 类标点分隔符（顿号、全角空格、书名号、角括号、破折号、间隔号）
- do-not-use 惩罚增强（`per_hit` 5→25，`max_ratio` 0.3→0.8）
- 权重文件 `tool_scoring_weights.json` 编译期 fallback（hook → FRAMEWORK_ROOT → 内嵌默认值）

#### 数据质量提升
- 54 工具 `trigger_hints` 密度提升，每工具补充 2-3 条场景化同义词
- `browser_*`/`codegraph_*` 系列增加去重叠独有 hint
- `tool_flags` 审计标记（`research_aigc_humanize` → deprecated）
- 短描述扩展（`skill_route_status`、`closeout_gate` 30+ 字符）

#### 安全加固
- `web_fetch_guard` 旧 API `validate_web_fetch_url` 标注 `#[deprecated]`，全面迁移至 `validate_and_resolve_web_fetch_url`（TOCTOU 防护）
- `routing_evolution` telemetry 文件读取加 `lock_shared()` flock 保护
- `pre_tool_use_guard` 集成 `check_auxiliary_file_reference()` 安全检查

#### 复合域分发拆分（H3）
- `dispatch_domain` 从 `"composite"` 拆分为 5 子域：`domain:goal` / `domain:quality-gate` / `domain:closeout` / `domain:routing-evolution` / `domain:framework`
- `tool_handlers.rs`（647 行）拆分为目录模块 `tool_handlers/`，4 个独立 handler 文件：`goal_handler.rs` / `quality_gate_handler.rs` / `closeout_handler.rs` / `routing_evolution_handler.rs`
- `mod.rs` 统一 re-export 接口，host-projection 按子域注册不同 handler

#### 测试覆盖
- `tool-routing-engine` 新增 5 项单元测试（do-not-use penalty / layer_penalty / alias / description / 超长查询）
- `tool-routing-engine` 新增 2 项集成测试（真实注册表 54+ 工具加载验证 / 全量路由可达性）

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
