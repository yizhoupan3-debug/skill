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
| Codex CLI | `~/.codex/hooks.json` | `~/.codex/config.toml` | `docs/hosts/codex-cli.md` |
| Claude Code | `.claude/settings.json` (4 事件) | `.claude/settings.json` + `.claude/router-rs-hook.env` | `docs/hosts/claude.md` |
| Claude Desktop | Claude Desktop MCP | `.claude-desktop/` | `docs/hosts/claude-desktop.md` |
| Antigravity | Antigravity CLI hooks | `.antigravitycli/hooks.json` | `docs/hosts/antigravity-cli.md` |

## 2. Hook 事件与行为差异

| 事件 | Cursor | Codex | Claude Code |
|------|--------|-------|-------------|
| SessionStart | 轻量 `Repo:` 行 | 轻量 `source:` 行 | 类似 Cursor |
| UserPromptSubmit | pre-goal nudge | session key 硬前置 | review 提示 |
| PostToolUse | 证据采集 | 证据采集 | 证据采集 |
| Stop | review gate + closeout + SESSION_CLOSE_STYLE | review gate + closeout (可 `decision:block`) | review gate + closeout |
| beforeSubmit | paper adversarial/prose hook, subagent model inherit nudge | N/A | N/A |
| SessionEnd | hook-state 清理 | N/A | N/A |
| subagentStart | subagent 计数 | N/A | N/A |

关键差异：Codex 的 Stop 可以 `decision:block` 硬阻断；Cursor 的 Stop 是 `followup_message` 软提示。

## 3. Shell launcher

宿主 hook 配置不直接调用 `router-rs` 二进制，而是通过 shell launcher 脚本：

- Cursor: `configs/framework/cursor-router-rs-hook.sh`（二进制发现 + fail-closed/fail-open 分层）
- Claude: `configs/framework/claude-router-rs-hook.sh`
- Codex: `configs/framework/codex-router-rs-hook.sh`

二进制发现顺序：`ROUTER_RS_BIN` 环境变量 -> 仓库 `target/release/` -> `command -v router-rs`。缺失时关键门控事件 fail-closed，telemetry 事件 fail-open。

## 4. 跨仓库接入

`scripts/cursor-bootstrap-framework.sh` 将 `skills/` 和 `AGENTS.md` 符号链接到目标仓库，并复制 hook 配置模板。支持 `--with-cursor-rules` 和 `--with-configs` 选项。
