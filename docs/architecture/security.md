---
last_verified: "2026-06-02"
depends_on:
  - components.md
  - ../closeout_enforcement.md
---

# 安全模型

本文档覆盖框架的测试层次、CI 流水线、schema drift 检测和生成物 drift 检测。

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
| `core/router-rs/tests/` | router-rs 单元测试（含 Claude Desktop hooks 测试） | `cargo test --manifest-path core/router-rs/Cargo.toml` |

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
