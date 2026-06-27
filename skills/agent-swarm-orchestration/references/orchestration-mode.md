# Orchestration mode（模式选择与触发）

## Gate 输出

```text
orchestration: { mode: local|sidecar|team|, trigger: explicit|auto_multi_phase|sidecar_default|local_default, reason: "<一句>" }
```

## 优先级（HARD）

1. **explicit team** — 用户当轮提到 `team` / `多 agent 团队` / `团队执行`
2. **auto_multi_phase** — 见下表，且未命中拒绝规则
3. **sidecar_default** — 声明式模式表（`review-parallel` 等）
4. **local_default** — 主线程足够

显式与 auto 均不满足时 **禁止** 升格 team。

## Explicit 触发

| 信号 | 动作 |
|------|------|
| `team`、`团队`、`多 agent`、`多阶段` | `trigger: explicit` → 选 team 编排 |

仍适用拒绝：`small_task`、`token_overhead_dominates`、`verification_missing`（写重且无 verify）、`write_scope_overlap`。

## Auto multi-phase 触发

满足 **任一** 且无 explicit 关键词：

| 模式 | 示例 |
|------|------|
| ≥3 命名阶段 | 「Phase1 扫描 / Phase2 合并 / Phase3 验证」；「第一步…第二步…第三步…」 |
| 管道型 | `Scan → Merge → Verify`；「先串行审查，再批量验证，最后出报告」 |
| 串并行混合 | 「多 lens 串行扫描后批量验证」 |

**不触发**：仅「全面审查」但无阶段结构 → 走 `multi-lens-review` sidecar，非 team。

## 模式 × 宿主

| mode | 宿主 | 执行 |
|------|------|------|
| `team` | 全宿主 | `agent-orchestrator` MCP 工具（`orchestrator_team_*`）；成员通过 `orchestrator_team_send_message` / `orchestrator_team_read_messages` 通信 |
| `sidecar` | 全宿主 | Bounded subagent，无 team 编排 |
| `local` | 全宿主 | 无 delegation |

## 与声明式 sidecar 表关系

Team 触发后 **不再** 用「全面审查 → review-parallel」表选拓扑；拓扑以 **TeamDescriptor.members + task_contract** 为准。未触发 team 时，声明式表仍有效。

## 拒绝与降级

| reason | 含义 |
|--------|------|
| `small_task` | 单文件/单阶段，team 过重 |
| `token_overhead_dominates` | 协调成本 > 收益 |
| `verification_missing` | 写重 spawn 无 verify_commands |

降级路径：team 某成员失败 → `team_remove_member` 标记错误 → 可替换成员或 `team_complete` 终止；禁止静默改拓扑。
