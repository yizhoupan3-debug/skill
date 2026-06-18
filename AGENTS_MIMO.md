# MIMO Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：须与 `AGENTS.md` 同时生效，勿单独使用本文件。本文仅 **MIMO**（`mimo`）transport delta。

## PreToolUse 硬阻断

`.mimo/settings.json` + hook 机制：未物化 `GOAL_STATE.json` 或未授权执行区 → **硬阻断**（`continue: false`）。

## Transport 要点

- **Review gate（Rust 侧）**：清门语义以 [`docs/spec.md`](docs/spec.md) §0.1 为准；Stop **`REVIEW_GATE` advisory-only**；review 通过 `closeout_gate` + `record_evidence` 完成。
- **框架命令流**：无 `AG_FOLLOWUP` / `updateCurrentStep`；续跑 `framework_goal_drive` + `artifacts/current/<task_id>/` 手动画板。
- **my-light**：suppress spawn-first 与 review Stop nudge（skill 层 findings-only 仍适用）。
- **Hook 支持**：`PreToolUse`、`Stop`（最小集）；review gate 在 Rust 侧。

## 安装与文件分布

- **文件 Scope 配置**：`.mimo/settings.json`。
- **环境安装**（与 Cursor 对齐 My 生命周期）：
  ```bash
  ./scripts/install-claude.sh  # 框架投影安装
  ```