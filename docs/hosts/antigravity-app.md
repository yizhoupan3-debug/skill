---
last_verified: "2026-06-02"
depends_on:
  - ../host_adapter_contract.md
---

# Antigravity App 宿主操作手册

**闭集 id**：`antigravity-app` · **传输**：MCP stdio · **配置根**：`.gemini/`

**兼容**：`host_id` / `install --to antigravity` 为 **本 App 的别名**（deprecated）；新集成请用 `antigravity-app`。

## 能力

- **MCP**：`router-rs-framework` → `router-rs antigravity-app agent`（或 deprecated `antigravity agent`）
- **Planning Mode** + 物化 `ROADMAP.md` / `WAVE_STATE.json`
- **无 shell hook 表**；closeout 靠 MCP `goal_state_manage` / `closeout_gate`（非 my-light 时可 [Antigravity Hard Block]）
- **Review**：物理 `review-lanes/*.md`；非 Cursor multiset

## 安装

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to antigravity-app --repo-root "$PWD"
```

材料化 `.gemini/mcp.json`、`.gemini/settings.json`、`.gemini/antigravity/rules/framework.md`。

## CLI 对照

终端 **Antigravity CLI**（hooks）见 [`antigravity-cli.md`](antigravity-cli.md)。二者共享 harness，传输不同。
