---
last_verified: "2026-06-09"
---

# 安全运维

## SSRF 与 URL 策略

| 工具 | 防护层 | 覆盖 |
|------|--------|------|
| `web_fetch`（MCP） | `web_fetch_guard.rs` | HTTP(S)、IP 黑名单（loopback/private/link-local/CGNAT/metadata）、host 后缀黑名单（`.localhost/.local/.internal`）、DNS pinning、重定向逐跳校验 |
| `browser_open`（MCP） | `validate_browser_open_url` | 阻断非 http(s) scheme（`file://`/`data:`/`javascript:`）、复用 IP/host 黑名单 |
| Bash `curl`/`wget` | 宿主 `excludedCommands` / 沙箱 | 沙箱开启时不自动放行 |

**browser_open 已知限制**：
- `browser_click`/`browser_fill` 可绕过 SSRF guard（通过页面内链接导航到内网）
- CDP `Page.navigate` 后的 3xx 重定向目标未经校验
- 无 DNS pinning（Chrome 自行解析，存在 TOCTOU 窗口）

回归：`cargo test --manifest-path core/router-rs/Cargo.toml -- web_fetch_guard`

## MCP 工具策略（core-policy）

- `session_launch` 的 host 参数禁止元数据端点
- `browser_get_network` 检测凭证关键词
- Shell 注入模式检测
- MCP 参数中危险 git 命令拦截

Smoke：`cargo test -p router-rs smoke_p0_hook_policy`（`mcp_safety` / `contract`）。

## 沙箱

沙箱由**各宿主运行时**管理；框架通过 hook_policy 与 excludedCommands 协同，不在本仓库统一开启 Seatbelt。具体默认值见各 [`docs/hosts/`](../hosts/) 手册。

## 运维注意

- 勿将 `.env`、密钥提交 Git
- `framework doctor` 不替代渗透测试；生产暴露 MCP 前审查 `RUNTIME_REGISTRY` 工具面
- 详细 env 安全相关开关：见 [`AGENTS.md`](../../AGENTS.md) § Coding First Principles
