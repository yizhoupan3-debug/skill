# 上下文与 Token 热点 — 构建侧审计报告

**范围**：仓库根 `/Users/joe/Documents/skill`；只读命令与账本扫描；未修改计划文件。  
**时间**：2026-05-11（以本机审计 shell 为准）。  
**说明**：`ROUTER_*` 检测在**当前非 Cursor 集成的 zsh 子进程**中执行；若仅在 Cursor 应用内设置的环境变量，此处可能仍为未检出，以宿主实际 `env` 为准。

---

## 1. 环境变量矩阵（对照 `docs/harness_architecture.md` §8）

**结论**：`printenv | grep -E '^ROUTER_'` **未检出任何** `ROUTER_` / `ROUTER_RS_*` 覆写 → **unset → defaults**（与 §8 表中「默认」列一致：如 `ROUTER_RS_OPERATOR_INJECT` 默认开、`retired verbose followup mode` 默认关/紧凑等）。

| 变量名 | 本机值 | §8 关闭后影响摘要 | 功能减损等级（plan §0） |
|--------|--------|-------------------|-------------------------|
| （无检出项） | — | 全部走代码/文档默认注入与 digest 行为 | 不适用（无主动关断） |

**后续若设置变量**：按 `docs/harness_architecture.md` §8 逐条对照「关闭后影响」；减损等级按 plan §0 表归类（例：`ROUTER_RS_OPERATOR_INJECT=0` → **Tier 3 高损**；`ROUTER_RS_HARNESS_OPERATOR_NUDGES=0` → **Tier 2 中有损**；账本清理与窄读路由 → **Tier 0**）。

---

## 2. 账本只读扫描（`artifacts/current/`）

### 2.1 目录与指针文件

| 路径 | 说明 |
|------|------|
| `artifacts/current/depth-enforcement-2026-05-10/` | 唯一任务子目录 |
| `artifacts/current/active_task.json` | `task_id`: `depth-enforcement-2026-05-10`，`updated_at`: `2026-05-10T21:50:07+08:00` |
| `artifacts/current/focus_task.json` | 与上同 `task_id` |
| `artifacts/current/task_registry.json` | `focus_task_id` 同上；`tasks[0].status`: **`in_progress`**，`phase`: `implementation` |
| `artifacts/current/depth-enforcement-2026-05-10/GOAL_STATE.json` | `status`: **`completed`**，`drive_until_done`: `false`，`updated_at`: `2026-05-10T13:56:24Z` |
| `artifacts/current/depth-enforcement-2026-05-10/EVIDENCE_INDEX.json` | 存在；`artifacts` 数组长度 **120**，文件约 **49 671** 字节 |
| `RFV_LOOP_STATE.json` | **未找到**（当前树下无 RFV 账本文件） |

### 2.2 任务 ID 汇总

- **活跃指针**：`depth-enforcement-2026-05-10`（`active_task` / `focus_task` / `task_registry.focus_task_id` 一致）。

### 2.3 陈旧 / 异常形态观察

1. **注册表 vs Goal 终态不一致**：`GOAL_STATE.json` 已为 **`completed`**，但 `task_registry.json` 中同一 `task_id` 仍为 **`in_progress`**。若宿主仍消费 registry 语义，可能与「任务已结束」心智不一致；建议按工作流将 registry / 指针与 `complete` 后状态对齐（非本报告强制执行）。
2. **EVIDENCE 条数触顶**：`EVIDENCE_INDEX` **120** 条，体量约 **50 KB**，接近框架侧常见上限语境下的「厚账本」；若 digest 或其它工具整文件读入对话，易放大 token。属**膨胀型磁盘态**，非 `running` goal 误续跑。
3. **未发现**：`status=running` 且 `drive_until_done=true` 的悬挂 Goal；未发现 `RFV_LOOP_STATE` active 残留。

---

## 3. 静态体积基线与「习惯降税」（≤10 行）

**`wc -c` 结果**（字节）：

| 文件 | 字节数 |
|------|--------|
| `AGENTS.md` | 19 345 |
| `skills/SKILL_ROUTING_RUNTIME.json` | 62 855 |
| `.cursor/rules/chinese-output.mdc` | 539 |
| `.cursor/rules/cursor-plan-output.mdc` | 1 183 |
| `.cursor/rules/execution-subagent-gate.mdc` | 1 335 |
| `.cursor/rules/framework.mdc` | 626 |
| `.cursor/rules/review-subagent-gate.mdc` | 1 463 |
| **`.mdc` 合计** | 5 146 |

**习惯降税（要点）**：

- `AGENTS.md` 由 Cursor **alwaysApply** 注入，体积最大；改策略前想清楚是否真需扩写。
- `SKILL_ROUTING_RUNTIME.json` **不应**每轮整文件读入对话；路由命中后只读对应 `skill_path`（可用 `jq` 取单条 `records[]`）。
- 任务切换后及时 **`complete` / `clear` Goal**、对齐 `active_task`，避免误续跑（Tier 0）。
- 勿默认打开 `retired verbose followup mode`；保持 beforeSubmit 侧双续跑 opt-in 关闭，除非明确需要（Tier 0）。
- 大 `EVIDENCE_INDEX`：按需归档或收窄 PostTool 采样，避免把整份 JSON 反复贴进模型上下文。
- Review/合规相关开关（如 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE`）勿为省 token 随意关。
- 需要总闸静音排障时再考虑 `ROUTER_RS_OPERATOR_INJECT=0`，日常优先账本与窄读（Tier 0）。

---

## 4. Plan 收口与 Git（gitx）

本代理无法在子 shell 中执行 Cursor 宿主命令。**请在 Cursor 本会话中执行一行**：`/gitx plan` — 以完成 plan-mode 与 `skills/gitx/SKILL.md` 同契约的计划对照与 Git 收口。

---

## 5. 验证命令记录（可复现）

```bash
cd /Users/joe/Documents/skill
printenv | grep -E '^ROUTER_' || true
ls -la artifacts/current/
find artifacts/current -maxdepth 2 -type f \( -name 'GOAL_STATE.json' -o -name 'RFV_LOOP_STATE.json' -o -name 'EVIDENCE_INDEX.json' -o -name 'active_task.json' -o -name 'focus_task.json' \)
wc -c AGENTS.md skills/SKILL_ROUTING_RUNTIME.json .cursor/rules/*.mdc
wc -c artifacts/current/depth-enforcement-2026-05-10/EVIDENCE_INDEX.json
```
