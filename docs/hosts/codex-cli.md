# Codex CLI 宿主操作手册

**权威能力矩阵**：`configs/framework/RUNTIME_REGISTRY.json` → `host_projections.codex-cli`  
**接入契约**：`docs/host_adapter_contract.md`  
**官方文档**：[Hooks](https://developers.openai.com/codex/hooks) · [AGENTS.md](https://developers.openai.com/codex/guides/agents-md)

## 安装 scope

| 组件 | Scope | 路径 |
|------|-------|------|
| `AGENTS.md`、`.codex/hooks.json` | **Project**（trusted project） | 仓库根 |
| Framework prompt 快照 | **Project** | `.codex/prompts/framework.md` |
| 全局 skill surface | **User** | `$CODEX_HOME/skills` → 仓库 `artifacts/codex-skill-surface/skills` |

## 何时跑 `codex sync` / `sync-entrypoints`

**要跑**：改了 `router-rs` 嵌入的 AGENTS 文本、Codex hook 模板、或需重材料化 `.codex/*` 入口。  
**不要跑**：仅为刷新 Cursor user `framework.mdc`（用 `host-integration install --to cursor --scope user`）。  
**不要**把 sync 当作「三端强行对齐」的默认日常步骤。

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"
# 或
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
```

磁盘上 **已存在** 的 `AGENTS.md` 不应被旧二进制 bootstrap 覆盖（官方近源优先）。

## 默认工作流

与全宿主相同：**My lifecycle** `/discussx` → `/planx` → `/implementx`（一口气跑完 `WAVE_STATE`）→ `/verifyx`；执行区 `/implementx` 启动 goal-style 连续执行（`/autopilot` 已退役；legacy 冷表 `/gsd-execute-phase` 见 `MIGRATION.md`）。

**续跑（2026-05）**：Codex/Cursor hooks **均不**注入 `GOAL_CONTINUE`、continuity digest 或 Stop checkpoint（代码 no-op；`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT=1` 亦不写盘）。连续执行依赖：`/implementx` 一口气、`framework_goal_drive` stdio、以及 `artifacts/current/<task_id>/` 手动画板。

## Hook 能力

- 多层 hook **全部加载、并发**；项目 hook 需 trusted project
- PostTool（opt-in）→ `EVIDENCE_INDEX`；SessionStart → 轻量指针/Repo（**无** continuity digest）；Stop → `CODEX_REVIEW_GATE` / closeout（**无** `GOAL_CONTINUE` / checkpoint 写盘）
- REVIEW_GATE 可数 lane：`review_gate.deep_gate_lanes`（与 Cursor 相同；**不含** Claude 的 `review`/`reviewer` 等）。单次 PostToolUse 证据 + Stop compact（wave-2 部分移植），无 multiset
- **Spawn-first**：`UserPromptSubmit` 注入 registry `spawn_first_nudge` 一行；窄范围（`review ./file`、`small_task`）不武装 gate；`my-light` 关闭硬拦与 spawn-first
- **Stop 清门（wave-2 部分）**：PostTool 深度 lane 可数证据 → `phase≥2`；Stop 上 compact findings **仅在有可数证据时** 升 `phase=3` 清门；`rg_clear` / bounded reject token 亦可清门。`ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` 全局关闭硬门控
- **Stop 与 Cursor 不对齐（刻意）**：Codex **无** subagentStart/Stop multiset、Cursor phase 软 nag cap、afterAgentResponse hook

## 独有

- **`session_supervisor`** / tmux 长会话（见 `rust-session-supervisor` skill）
- `$CODEX_HOME/skills` 为 **27 pinned** surface（`skills/SKILL_ROUTING_RUNTIME.json` `hot_skill_count`），≠ 全量 **52** on-disk skills（`manifest_skill_count`）。仓库内投影目录 `artifacts/codex-skill-surface/skills` 须 `just publish` 或 `framework maint update-one-shot` 生成，克隆后可能为空。

## 自检

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint verify-codex-hooks
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework skills validate
```
