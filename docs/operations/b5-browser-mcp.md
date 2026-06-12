---
last_verified: "2026-06-09"
plate: B5
---

# B5 — browser-mcp

## 职责

浏览器自动化 MCP：`browser-mcp` stdio 服务、`session_launch`、页面 attach、与 **web_fetch_guard** 协同的 URL 策略。实现：`core/browser-mcp/`；Node 侧包由 `host-integration install` 投影注册。

## 启动 / 配置

```bash
# MCP stdio 环（调试）
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  browser mcp-stdio --repo-root "$PWD"

# 经宿主 MCP 配置启动（推荐）
# 安装投影后 MCP 条目见各宿主 mcp.json；server id: browser-mcp
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to <host_id> --repo-root "$PWD"
```

Registry 中 MCP 投影键：见 `RUNTIME_REGISTRY.json` → `managed_mcp_servers.browser-mcp` 及各宿主 `managed_mcp_server_ids`。

P8 后 **无 tmux** 依赖；`session_supervisor` 管理 attach 生命周期。

## 排障

| 现象 | 处理 |
|------|------|
| `no browser-mcp runtime attach artifact` | 先 `session_launch` 或检查 `artifacts/` 下 attach 候选 |
| MCP 命令指向陈旧路径 | 重跑 `host-integration install`（含 browser-mcp 重写测试） |
| 非 http(s) URL 被拦 | 预期行为；`web_fetch_guard::validate_browser_open_url` |
| host filter 测试失败 | `cargo test -p router-rs browser_mcp` 簇 |

## 相关路径

- `core/browser-mcp/`
- `core/runtime-core/src/web_fetch_guard.rs`
- `docs/operations/security.md` §SSRF
- `RUNTIME_REGISTRY.json` → `managed_mcp_servers.browser-mcp`（跨五宿主统一声明）
