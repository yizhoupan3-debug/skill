---
last_verified: "2026-06-02"
depends_on:
  - components.md
  - ../closeout_enforcement.md
---

# 安全模型

> **status: aspirational** — 本文件描述了框架的理想安全模型。§0（mcp-tool-safety）已实现于 `hook_policy/`；§1 以下的测试/CI/Schema drift 描述为计划中的完整安全体系。

本文档覆盖框架的测试层次、CI 流水线、schema drift 检测和生成物 drift 检测。

## 0. MCP 工具安全拦截层 (mcp-tool-safety)

`hook_policy/` 实现了三层 MCP 工具安全拦截，作为 PreToolUse 守卫的补充：

### Layer 1: 高风险工具名拦截

无条件拦截以下工具（不依赖参数内容）：

| 工具 | 风险 |
|------|------|
| `session_launch` | 可通过 prompt 注入执行任意远程代码 |
| `session_resume_due` | 可能以陈旧状态重新触发已阻断的 worker |

> 注：`session_launch` 本身是合法工具，仅在参数含危险内容时拦截（见 Layer 2）。`tools/call` 路径已通过 `mcp_pre_guard` 接线（`browser_mcp`、`mcp_stdio_harness`）。

### Layer 2: 参数级风险模式

| 工具 | 参数字段 | 匹配模式 | 风险说明 |
|------|----------|----------|----------|
| `browser_get_network` | (any) | `password\|token\|secret\|cookie\|authorization\|api_key` | 可能捕获网络流量中的敏感 header 或 token |
| `browser_fill` | `value` | `password\|secret\|token\|credential` | 填充凭据类值可能泄露到页面 |
| `session_launch` | `prompt` | `curl\|wget ... \| sh\|bash` | prompt 含管道到 shell 的远程代码执行 |
| `session_launch` | `prompt` | `rm -rf` | prompt 含破坏性删除命令 |
| `session_launch` | `host` | `0.0.0.0\|169.254\|metadata.google` | host 指向云元数据端点，可能泄露凭据 |
| `session_mark_blocked` | `evidenceText` | `password\|token\|secret\|api_key` | evidenceText 可能将敏感数据持久化到磁盘 |

### Layer 3: Shell 注入模式

对含命令字符串的 MCP 工具参数（如 `browser-mcp` JS eval、session prompt）复用 bash-danger 启发式：

- 远程脚本管道到 shell：`curl|wget ... | sh|bash`
- 进程替换：`sh <(curl ...)`
- `git reset --hard`
- `git push --force`

**实现位置**：`core/core-policy/src/hook_policy.rs`（`dangerous_mcp_tool_reason`）；`tools/call` 接线：`core/runtime-core/src/mcp_pre_guard.rs` → `browser_mcp` / `mcp_stdio_harness`。

## 1. 测试层次

| 测试文件 | 覆盖范围 | 运行方式 |
|----------|----------|----------|
| `tests/policy_contracts.rs` (111KB) | skill 路由契约、plugin catalog 闭集、manifest 一致性、research contract、深度 lane 等 | `cargo test --test policy_contracts` |
| `tests/host_integration.rs` (81KB) | 宿主集成测试、hook 输出格式、安装产物校验 | `cargo test --test host_integration` |
| `tests/documentation_contracts.rs` | 文档链接、命名约定、tracked markdown UTF-8 契约 | `cargo test --test documentation_contracts` |
| `tests/routing_eval_cases.json` | 路由评估用例（25KB JSON fixture） | 通过 `eval_route` 模块消费 |
| `tests/routing_route_fixtures.json` | 路由 fixture（10KB） | 路由引擎单测 |
| `tests/browser_mcp_scripts.rs` | Browser MCP 脚本测试 | `cargo test` |
| `tests/policy_cursor_rules_links.rs` | Cursor rules 链接校验 | `cargo test` |
| `tests/policy_markdown_links.rs` | Markdown 链接校验 | `cargo test` |
| `tests/tracked_markdown_utf8_contract.rs` | Tracked markdown UTF-8 契约 | `cargo test` |
| `tests/rust_cli_tools.rs` | Rust CLI 工具集成测试 | `cargo test` |
| `tests/autoresearch_cli.rs` | Autoresearch CLI 测试 | `cargo test` |
| `core/router-rs/tests/` | router-rs 单元测试 | `cargo test --manifest-path core/router-rs/Cargo.toml` |

## 2. Justfile 命令

```bash
just fmt           # cargo fmt
just clippy        # cargo clippy -D warnings
just test          # router-rs 测试
just test-all      # 全量测试（router-rs + antigravity + policy_contracts + host_integration）
just validate-skills  # skill 路由校验
just compile-skills   # skill 路由刷新
just doctor        # 框架健康检查
just ci            # validate-skills + test-all
```

## 3. CI 流水线

`.github/workflows/skill-ci.yml`：

- push/PR 触发
- 运行 `cargo test`（全量）
- 运行 `framework skills validate`
- 校验生成物漂移（metadata-only 模式）

`.github/workflows/evolution-audit.yml`：

- 定时触发
- 健康审计
- 同步 routing 产物
- 创建维护 issue

## 4. Schema Drift 检测

`router-rs schema-drift` 子命令组用于检测：

- hook 事件闭集（7 事件 Cursor、4 事件 Claude）是否与 contract 一致
- REQUIREMENTS/ROADMAP 标题格式是否符合约定
- 模板 parity（跨仓库 hooks.json 与 workspace-template 是否匹配）

## 5. 生成物 Drift 检测

`framework host-integration generated-artifacts-status` 有两种模式：

- **metadata-only**（默认，`framework doctor` 使用）：只检查声明路径存在、forbidden marker、undeclared 路径
- **drift-gate**（全量，`framework maint update-one-shot` 使用）：在隔离 temp root 重跑 generator，byte/normalized 对比
