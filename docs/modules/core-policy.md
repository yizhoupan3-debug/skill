---
module: core-policy
lines: ~4400
layer: B1
last_verified: "2026-06-19"
---

# core-policy（B1 层）

策略评估与 review gate 引擎，提供跨宿主的 hook 策略判断、安全检查、review gate 状态机。

## 职责

为所有宿主提供统一的策略评估：hook 行为决策、安全防护、review gate 状态管理。

## 核心功能

| 模块 | 行数 | 功能 | 关键 API |
|------|------|------|----------|
| `hook_policy` | 1,187 | hook 策略评估引擎 | `evaluate_hook_policy()`、`regex_is_match()`、`dangerous_bash_reason()` |
| `hook_common` | 851 | hook 公共逻辑 | `is_review_prompt()`、`is_completion_claim()`、`normalize_subagent_type()` |
| `registry_review_gate` | 432 | registry review gate | `lifecycle_profile_disables_spawn_first_nudge()` |
| `review_gate_engine` | 334 | review gate 引擎 | `ReviewGateVerdict`、`fork_context_from_values()` |
| `hook_review_disk_state` | 374 | hook review 磁盘状态 | `HookReviewDiskCore` 序列化/反序列化 |
| `review_output_lint` | 262 | review 输出 lint | `lint_review_output()` |
| `env_flags` | 280 | 环境变量标志 | `router_rs_review_disabled()`、`router_rs_spawn_first_nudge()` |
| `review_context_signals` | 133 | review 上下文信号 | GitHub PR 检测、论文上下文检测 |
| `review_routing_signals` | 120 | review 路由信号 | parallel review candidate regex |
| `dev_exempt` | 163 | 开发豁免路径 | `is_dev_exempt_path()` |
| `crypto_util` | 18 | 加密工具 | `hex_lower()`、`short_hash()`（SHA-256 前 16 字节） |
| `session_key` | 44 | 跨宿主 session key | `session_key_core()`（env → cwd → fallback 优先级链） |
| `lane_normalize` | 25 | lane 标准化 | `normalize_lane()` |

## 安全机制

- `hook_policy::evaluate_hook_policy`: 分层策略评估（protected-path → file-category → MCP 工具名/参数检查）
- `hook_policy::MCP_ARG_RISK_PATTERNS`: MCP 工具参数风险模式匹配
- `hook_policy::regex_is_match`: 预编译正则匹配（OnceLock 模式）

## 依赖关系

- **依赖**: `framework-kernel`（`telemetry`、`tokenizer`）
- **被依赖**: `runtime-core`、`host-projection`

## 近期变更

- v6.5: 新增 `crypto_util.rs`（SHA-256 短哈希）和 `session_key.rs`（跨宿主 session 标识）
- v6.5: `hook_policy.rs` 中 `repo_root` 重复解析已修复
- v6.5: 测试模块中冗余 `use serde_json` 导入已清理

## 已知技术债

- `regex_is_match` 函数 ~120 行 match-dispatch，每新增模式需加 arm
- `MCP_ARG_RISK_PATTERNS` 的 `_field` 参数未使用（对整个 JSON 字符串匹配而非指定字段）
- 三个密码相关正则模式功能高度重叠
