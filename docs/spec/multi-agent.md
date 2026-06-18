---
parent: docs/spec.md
version: unified-v7
---

## 5. 多 Agent 编排契约

> team 已废弃，tmux 已废弃。仅 `subagent` + `workflow`。

### 5.1 Subagent 生命周期

```
spawned → running → draining → completed
                    → failed
            → interrupted
```

### 5.2 隔离模型

| 维度 | 机制 |
|------|------|
| 进程 | `std::process::Command` detached + PID 文件 |
| 文件系统 | git worktree |
| 状态 | SQLite-backed（同 background_state） |
| 上下文 | `fork_context=false` |

### 5.3 Workflow 执行

| 模式 | 宿主 |
|------|------|
| `workflow_native`（JS 运行时） | claude |
| `workflow_supervisor`（Task 模拟） | 所有 |

### 5.4 Spawn Admission

允许：读重 · 独立假设 · 不阻塞 supervisor · disjoint 写入

拒绝原因：`small_task` · `shared_context_heavy` · `write_scope_overlap` · `next_step_blocked` · `verification_missing` · `token_overhead_dominates`

### 5.5 REVIEW_GATE 差异

> **清门真源（2026-06）**：`core-policy::review_gate_satisfied` — `independent_reviewer_seen`（`reviewer_lanes` + `fork_context=false`）或 override。**全宿主 Stop advisory-only**（不 `permission: deny` / `decision:block`）。详见 [`../hosts/hook-hosts.md`](../hosts/hook-hosts.md) §门控。

| 能力 | claude | cursor | codex | opencode |
|------|:-----------:|:------:|:-----:|:--------:|
| 可数深度 lane | `reviewer_lanes`（registry 共用闭集） | 同左 | 同左 | skill + `review-lanes/` |
| spawn-first | ✅ | ✅ | ✅ | — |
| Stop 出站 | advisory nudge | advisory nudge | advisory nudge | MCP advisory |
| my-light | suppress nudge | suppress nudge | suppress nudge | suppress nudge |

---

