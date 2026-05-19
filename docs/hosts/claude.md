# Claude Code / Claude Desktop 宿主操作手册

**权威能力矩阵**：`configs/framework/RUNTIME_REGISTRY.json` → `host_projections.claude-code` / `claude-desktop`

## Claude Code (`claude-code`)

**官方**：[Hooks reference](https://code.claude.com/docs/en/hooks)

| 组件 | 路径 |
|------|------|
| Hooks | `.claude/settings.json` → `router-rs claude hook` |
| Framework 规则 | `.claude/rules/framework.md` |
| 项目叙事 | `.claude/CLAUDE.md`（可选） |

**能力**：`PreToolUse` **可 deny**；`Stop` 每轮一次；REVIEW_GATE 除 registry 闭集外还接受 `review`/`reviewer`/`critic`/`code-review`（**仅 Code**，Cursor/Codex 不认）。

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- \
  framework host-integration install --to claude --scope project
```

## Claude Desktop (`claude-desktop`)

**传输**：MCP stdio — **无** CLI 级 PreToolUse / SubagentStop hook 表。

| 能力 | 状态 |
|------|------|
| 热路由 + L2 continuity | ✓ |
| `closeout_evidence_hooks` | **unsupported**（registry exception） |
| `review_gate_router_observation` | **unsupported** |
| 硬门控 REVIEW_GATE | **勿声称**与 Claude Code 相同 |

Desktop 用户须在 MCP 侧 **手动** `record_evidence` / `session_checkpoint`；证据与 goal 仍写在 `artifacts/current/<task_id>/`。

## 默认工作流（两者）

**GSD** 为默认生命周期（`/gsd-*` 已在 registry 注册）；hook 能力差异 **不改变** 默认命令顺序。

## 自检（Code）

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml claude
```
