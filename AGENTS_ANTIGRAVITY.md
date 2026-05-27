# Antigravity 宿主专用策略

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。本文仅 Antigravity 宿主差异。

## 权威分层

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议 | 仓库根 [`AGENTS.md`](AGENTS.md) |
| Antigravity 宿主策略 | **`AGENTS_ANTIGRAVITY.md`**（本文件） |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架 CLI | `configs/framework/RUNTIME_REGISTRY.json` |

## 宿主集成与诊断

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration install --to antigravity --repo-root "$PWD"
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration status
```

## 并行子代理调度（Antigravity 差异）

- **`my-light`** 关闭 REVIEW_GATE 硬拦；大规模任务（多文件、>50 行 delta、跨模块）**须** `invoke_subagent` 并行，主线程 scheduler only。
- 子代理故障可降级串行；Verify 阶段主线程可 fix obvious。
- 深度 review：[`skills/code-review-deep/SKILL.md`](skills/code-review-deep/SKILL.md) spawn-first + findings-only。

## 连续性

- 真源：`artifacts/current/<task_id>/`；`GOAL_STATE.json`（`lifecycle_profile: my-light`）。
- 显式 stdio：`framework_goal_drive` / `framework_rfv_loop`；verify 后 purge task dir。

## Knowledge Hygiene

- 本文件是 Antigravity 地图；跨宿主正文在 [`AGENTS.md`](AGENTS.md)。
