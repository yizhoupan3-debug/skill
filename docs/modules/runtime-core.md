---
module: runtime-core
lines: ~14000
layer: B1
last_verified: "2026-06-13"
---

# runtime-core（B1 层）

框架运行时核心 crate，包含 CLI、框架运行时、会话管理、closeout 强制、RFV 循环等。

## 职责

提供框架运行时的完整执行引擎：CLI 解析、命令分发、会话监督、closeout 评估、evidence 管理。

## 顶层模块索引

| 模块 | 行数 | 功能 |
|------|------|------|
| **`framework_runtime/`** | 12,100+ | **核心运行时**（详见 [runtime-core-framework-runtime.md](runtime-core-framework-runtime.md)） |
| `session_supervisor/` | 2,443 | 会话监督器（driver/worker/process/evolution_idle） |
| `runtime_storage/` | 3,252 | 存储后端（filesystem/sqlite/operation/paths） |
| `closeout_enforcement` | 1,119 | closeout 记录评估与强制执行 |
| `execution_contract` | 1,121 | 执行契约（前置/后置条件验证） |
| `rfv_loop` | 1,800 | RFV（Review-Fix-Verify）循环完整实现 |
| `framework_maint` | 1,808 | 框架维护 CLI 子命令 |
| `background_state/` | 1,895 | 后台任务状态管理 |
| `cli/` | 973 | CLI 参数解析与分发 |
| `stdio_transport` | 868 | stdio 传输层 |
| `trace_runtime` | 858 | 运行时 trace 管道 |
| `web_fetch_guard` | 422 | web fetch URL SSRF 防护 |
| `paper_adversarial_hook` | 412 | 论文对抗性审查 hook |
| `eval_route` | 446 | 评估路由 |
| `router_env_flags` | 454 | `ROUTER_RS_*` 环境变量标志 |
| `session_call_tracker` | 391 | 会话调用跟踪 |
| `harness_operator_nudges` | 351 | harness 操作员提示 |
| `harness_contract` | 336 | harness 契约 |
| `router_rs_observation` | 325 | router-rs 观测数据附加/剥离 |
| `framework_skills` | 314 | 框架 skill 管理 |
| `telemetry_emit` | 280 | 遥测发射 |
| `hook_observation_rules` | 275 | hook 观测规则 |
| `paper_prose_hook` | 240 | 论文散文质量 hook |
| `harness_context_signals` | 205 | harness 上下文信号 |
| `schema_drift` | 617 | Schema 版本漂移检测 |
| `route/` | 703 | 路由元数据 |
| `hook_event_routing` | 81 | hook 事件路由 |
| `hook_outbound_protect` | 133 | hook 出站保护 |
| `hook_timing` | 116 | hook 时序记录 |
| `mcp_pre_guard` | 111 | MCP 预守卫 |

## 安全/防护机制

| 模块 | 功能 |
|------|------|
| `web_fetch_guard` | SSRF 防护：URL 域名白名单/黑名单 |
| `hook_outbound_protect` | hook 出站内容过滤 |
| `mcp_pre_guard` | MCP 工具调用预检 |

## 依赖关系

- **依赖**: `framework-kernel`、`core-policy`、`core-state`、`host-projection`（re-export）
- **被依赖**: `router-rs`（CLI binary）

## 近期变更

- v6.5: `framework_host_targets` 迁移至 `framework-kernel`，通过 `pub use` 保持兼容
- v6.5: 新增 `mod_tests.rs`（evidence lock order、resolve_repo_root、truncate_utf8 测试）

## 已知技术债

- `framework_runtime/` 子模块 12,000+ 行，是仓库最大单一模块
- `is_terminal` 函数在 `mod.rs` 和 `runtime_view.rs` 中重复定义
- `paper_adversarial_hook` 和 `paper_prose_hook` 在文档中未描述
- `harness_operator_nudges` 和 `hook_observation_rules` 在 AGENTS.md 中无对应描述
