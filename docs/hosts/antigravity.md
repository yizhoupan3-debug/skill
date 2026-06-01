---
last_verified: "2026-06-02"
depends_on:
  - antigravity-cli.md
  - antigravity-app.md
---

# Antigravity 宿主（索引 — 已拆分）

Antigravity 官方提供 **两条** 集成面，本框架已拆为闭集宿主 id：

| 宿主 | 手册 |
|------|------|
| **Antigravity CLI**（终端 hooks） | [`antigravity-cli.md`](antigravity-cli.md) |
| **Antigravity App**（Desktop / MCP） | [`antigravity-app.md`](antigravity-app.md) |

**迁移**：`install --to antigravity` 与 `router-rs antigravity agent` 仍指向 **App（MCP）**，但已 deprecated；见 [`MIGRATION.md`](../../MIGRATION.md) § Antigravity CLI / App 拆分。

跨宿主政策：**`AGENTS_ANTIGRAVITY.md`**。
