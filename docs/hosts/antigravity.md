---
last_verified: "2026-06-02"
depends_on:
  - ../host_adapter_contract.md
---

# Antigravity 宿主操作手册

**闭集 id**：`antigravity` · **传输**：MCP stdio · **配置根**：`.gemini/`

**兼容**：`antigravity-app` 为已退役别名；新集成请用 `antigravity`。


## 能力

- **MCP**：`router-rs-framework` · `browser-mcp` · `mcp-codegraph` · `paperplain`（via `.gemini/mcp.json`；**无 shell hook**）
- **Planning Mode** + 物化 `ROADMAP.md` / `WAVE_STATE.json`
- **无 shell hook 表**；门控经 MCP `goal_state_manage` / `closeout_gate`
- **Review**：物理 `review-lanes/*.md`；非 Cursor multiset

## Closeout 分层

- **Review**：MCP `closeout_gate` / `goal_state_manage` 报告 review 缺口为 **ADVISORY**（不硬拦 Stop）
- **Closeout**：**my-light**（默认 My 链）下 complete/closeout 为 advisory；**非 my-light** 时 MCP `closeout_gate` 未满足可 **hard-block** `goal_state_manage(complete)`（见 [`host_adapter_contract.md`](../host_adapter_contract.md) §0.1、`ROUTER_RS_CLOSEOUT_ENFORCEMENT`）

## 安装

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to antigravity --repo-root "$PWD"
```

材料化 `.gemini/mcp.json`、`.gemini/settings.json`、`.gemini/antigravity/rules/framework.md`。
