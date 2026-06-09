---
last_verified: "2026-06-09"
depends_on:
  - ../host_adapter_contract.md
---

# Antigravity 宿主

Antigravity 为 Google 桌面 agent orchestration hub（非 IDE）；本框架通过 **`.gemini/` MCP 投影**集成。

| 项 | 值 |
|----|-----|
| **宿主 id** | `antigravity` |
| **安装** | `framework host-integration install --to antigravity` |
| **MCP 循环** | `router-rs antigravity agent` |
| **策略入口** | [`AGENTS.md`](../../AGENTS.md) + [`AGENTS_ANTIGRAVITY.md`](../../AGENTS_ANTIGRAVITY.md)（双文件注入；delta transport only） |

## 能力

- **MCP**：`router-rs-framework` → `router-rs antigravity agent`
- **Planning Mode** + 物化 `ROADMAP.md` / `WAVE_STATE.json`
- **无 shell hook 表**；`goal_state_manage` / `closeout_gate` 在 MCP 工具层执行（非 my-light 时 closeout 可 fail-closed，与 review 分层）
- **Review**：清门 **Claude canonical**（`reviewer_lanes` + independent evidence）；`closeout_gate` / `goal_state_manage` 上 review 缺口为 **ADVISORY**（**不**硬拦 Stop）；物理 `review-lanes/*.md`；无 Cursor multiset 遥测

## Hook 事件矩阵

Antigravity 为 **纯 MCP 宿主**（闭集 id：`antigravity`），无 shell hook 面；门控与 closeout 经 MCP `router-rs-framework` 实现。对照 hook 宿主（含 **Codex** `codex`）见 [`codex.md`](codex.md)、[`cursor.md`](cursor.md)、[`claude.md`](claude.md)。

## 投影路径

- 项目：`.gemini/mcp.json`、`.gemini/settings.json`、`.gemini/antigravity/rules/framework.md`
- 用户：`~/.gemini/` 下同名结构

## 安装

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to antigravity --repo-root "$PWD"
```

## 生命周期与 closeout

- 默认 My 链：`/discussx` → `/planx` → `/implementx` → `/verifyx`
- 显式辅助命令（五宿主同路径）：`/deepinterview`、`/gitx`、`/update`
- 连续性：`artifacts/current/<task_id>/` + `goal_state_manage` MCP / `framework_goal_drive` stdio

## 退役宿主（勿再写入 registry）

| 退役 id | 说明 |
|---------|------|
| `antigravity-app` | 已合并入 `antigravity`；CLI 或仍接受为 deprecated 别名 |
| `antigravity-cli` | 已从闭集移除；`.antigravitycli/` 为历史残留 |

详见 [`MIGRATION.md`](../../MIGRATION.md) §闭集宿主收敛（2026-06）。

跨宿主政策：**[`AGENTS.md`](../../AGENTS.md)** + 本宿主 **[`AGENTS_ANTIGRAVITY.md`](../../AGENTS_ANTIGRAVITY.md)**（双文件，禁止合并）。
