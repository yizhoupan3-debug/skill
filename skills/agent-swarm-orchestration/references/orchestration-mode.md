# Orchestration mode（模式选择与触发）

> 编排 JS 真源：[`workflow-script-conventions.md`](./workflow-script-conventions.md) · 模板：[`workflow-template-catalog.md`](./workflow-template-catalog.md)

## Gate 输出

```text
orchestration: { mode: local|sidecar|workflow_native|workflow_supervisor, trigger: explicit|auto_multi_phase|sidecar_default|local_default, reason: "<一句>" }
```

## 优先级（HARD）

1. **explicit workflow** — 用户当轮提到 `workflow` / `ultracode` / 「用 workflow 跑」
2. **auto_multi_phase** — 见下表，且未命中拒绝规则
3. **sidecar_default** — 声明式模式表（`review-parallel` 等）
4. **local_default** — 主线程足够

显式与 auto 均不满足时 **禁止** 升格 workflow。

## Explicit 触发

| 信号 | 动作 |
|------|------|
| `workflow`、`ultracode`、中英「用 workflow」 | `trigger: explicit` → 选 native 或 supervisor |

仍适用拒绝：`small_task`、`token_overhead_dominates`、`verification_missing`（写重且无 verify）、`write_scope_overlap`。

## Auto multi-phase 触发

满足 **任一** 且无 explicit 关键词：

| 模式 | 示例 |
|------|------|
| ≥3 命名阶段 | 「Phase1 扫描 / Phase2 合并 / Phase3 验证」；「第一步…第二步…第三步…」 |
| 管道型 | `Scan → Merge → Verify`；「先串行审查，再批量验证，最后出报告」 |
| 串并行混合 | 「多 lens 串行扫描后批量验证」（执行/搜索场景可用 `parallel`） |

**不触发**：仅「全面审查」但无阶段结构 → 走 `multi-lens-review` sidecar，非 workflow。

## 模式 × 宿主

| mode | 宿主 | 执行 |
|------|------|------|
| `workflow_native` | claude-code（且未 `disableWorkflows`） | 后台跑 `.claude/workflows/*.js` |
| `sidecar` | 全宿主 |  Bounded Task，无 JS runtime |
| `local` | 全宿主 | 无 delegation |

## 与声明式 sidecar 表关系

Workflow 触发后 **不再** 用「全面审查 → review-parallel」表选拓扑；拓扑以 **JS `meta.phases` + LENSES** 为准。未触发 workflow 时，声明式表仍有效。

## 拒绝与降级

| reason | 含义 |
|--------|------|
| `small_task` | 单文件/单阶段，workflow 过重 |
| `token_overhead_dominates` | 协调成本 > 收益 |
| `verification_missing` | 写重 spawn 无 verify_commands |

降级路径：`workflow_supervisor` 某 phase 失败 → 记录 `phase-*.json` → 可 skip 该 agent 或 abort；禁止静默改拓扑。
