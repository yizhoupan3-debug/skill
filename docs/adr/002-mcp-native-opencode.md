---
last_verified: "2026-06-16"
depends_on:
  - ../spec.md
  - ../hosts/opencode.md
---

# ADR-002: OpenCode MCP-Native 架构决策

## Status

Accepted (2026-06-14).

## Context

OpenCode 加入闭集宿主列表时，其 hook 处理架构与 cursor/claude/codex 有本质差异：OpenCode 的 hook 处理在 JS/TS 插件系统中执行，而非 Rust 侧。这引发了「OpenCode 是否算完整的 hook 体系宿主」的架构讨论。

## Decision

1. **OpenCode 是完整的 hook 体系宿主**：Provider trait、harness capabilities（FULL）、注册表元数据与 cursor/claude/codex 完全一致。差异仅在 hook 处理层的实现语言（JS/TS 插件 vs Rust），不影响架构地位。

2. **Rust 侧无需构建 hook 分发层**：OpenCode 的 PreToolUse/PostToolUse/Stop 等事件由插件系统处理，Rust 侧 `opencode_hooks.rs` 仅处理框架元逻辑（review gate 状态、session key、evidence 采集）。

3. **MCP 工具层独立**：`framework_snapshot`、`skill_route`、`goal_state_manage` 等 MCP tool 在 opencode 中通过 `opencode.json` 的 `mcpServers` 注册，与 opencode-agent MCP stdio loop 集成。

4. **Session/review 状态管理**：使用 `.opencode/hook-state/` 目录，磁盘格式与 cursor/claude/codex 兼容，复用 `core-policy` 的共享实现（`HookReviewDiskCore`、`FileStateLock`）。

## Consequences

- **优势**：opencode 的 fail-open 策略使 hook 失败不影响核心编辑功能；插件系统提供更灵活的配置能力。
- **代价**：opencode 不支持 Rust 侧 hard gate hooks（`has_hard_gate_hooks = false`），closeout evidence hooks 不可用。
- **迁移成本**：无 legacy 迁移需求（新宿主）。

## Related

- `docs/spec.md` §0.1 — opencode 宿主深度评估
- `docs/hosts/opencode.md` — 宿主操作手册
- `configs/framework/RUNTIME_REGISTRY.json` — 注册表定义
