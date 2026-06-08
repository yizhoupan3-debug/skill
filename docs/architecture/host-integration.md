---
last_verified: "2026-06-02"
depends_on:
  - ../host_adapter_contract.md
  - ../harness_architecture/03-hook-and-switches.md
---

# 宿主集成

本文档覆盖框架的多宿主适配层：宿主列表、hook 事件差异、shell launcher、跨仓库接入。

## 1. 宿主列表

| 宿主 | Hook 入口 | 配置文件 | 文档 |
|------|-----------|----------|------|
| Cursor | `.cursor/hooks.json` (7 事件) | `.cursor/hooks.json` + `.cursor/router-rs-hook.env` | `docs/hosts/cursor.md` |
| Codex | `~/.codex/hooks.json` | `~/.codex/config.toml` | `docs/hosts/codex.md` |
| Claude Code | `.claude/settings.json` (4 事件) | `.claude/settings.json` + `.claude/router-rs-hook.env` | `docs/hosts/claude.md` |
| Antigravity | MCP stdio | `.gemini/` | `docs/hosts/antigravity.md` |
| OpenCode | MCP stdio | `.opencode/` | `docs/hosts/opencode.md` |

## 2. Hook 事件与行为差异

闭集五宿主中，**hook 宿主**（`cursor`、`codex`、`claude-code`）走 shell launcher → `router-rs`；**MCP 宿主**（`antigravity`、`opencode`）无 shell hook，门控在 MCP 工具层。各宿主细则见 [`docs/hosts/`](../hosts/) **Hook 事件矩阵**（hook 宿主）或 MCP 配置节（MCP 宿主）。

| 事件 | Cursor | Codex | Claude Code | Antigravity | OpenCode |
|------|--------|-------|-------------|-------------|----------|
| SessionStart | 轻量 `Repo:` 行 | 轻量 `source:` 行 | 类似 Cursor | —（MCP） | —（MCP） |
| UserPromptSubmit | pre-goal nudge | session key 硬前置 + re-arm | review 提示 | —（MCP advisory） | —（MCP advisory） |
| PostToolUse | 证据采集 | 证据采集 | 证据采集 | —（MCP） | —（MCP） |
| Stop | review gate + closeout + SESSION_CLOSE_STYLE | review gate（advisory nudge）+ closeout（可 `decision:block`） | review gate + closeout | —（MCP） | —（MCP） |
| beforeSubmit | paper / subagent model inherit nudge | — | — | — | — |
| SessionEnd | hook-state 清理 | — | — | — | — |
| subagentStart / Stop | subagent 计数 | — | — | — | — |

关键差异：**全宿主** Stop 上 `REVIEW_GATE` 均为 **advisory-only**（nudge / `followup_message`，不硬拦 Stop）；**closeout** 与 review 分层，Codex closeout 仍可 `decision:block`。**Antigravity / OpenCode** 无 hook 面，见 [`codex.md`](../hosts/codex.md)、[`antigravity.md`](../hosts/antigravity.md)、[`opencode.md`](../hosts/opencode.md)。

## 3. Shell launcher

宿主 hook 配置不直接调用 `router-rs` 二进制，而是通过 shell launcher 脚本：

- Cursor: `configs/framework/cursor-router-rs-hook.sh`（二进制发现 + fail-closed/fail-open 分层）
- Claude: `configs/framework/claude-router-rs-hook.sh`
- Codex: `configs/framework/codex-router-rs-hook.sh`

二进制发现顺序：`ROUTER_RS_BIN` 环境变量 -> 仓库 `target/release/` -> `command -v router-rs`。缺失时关键门控事件 fail-closed，telemetry 事件 fail-open。

## 4. 跨仓库接入

`scripts/cursor-bootstrap-framework.sh` 将 `skills/` 和 `AGENTS.md` 符号链接到目标仓库，并复制 hook 配置模板。支持 `--with-cursor-rules` 和 `--with-configs` 选项。
