# Continuity harness — upper-level architecture

本文件是 **宿主 hook + router-rs + 连续性工件** 的**上层设计真源**：说明分层、数据流与扩展规则，避免在代码里零散堆「又一个环境变量 / 又一个硬编码提示句」。

**与 `AGENTS.md` 的分工**：`AGENTS.md` = 跨宿主**执行与语言策略**；本文 = **控制面结构**（谁写盘、谁注入、谁算证据）。

**文档索引**：steady-state 契约导航与历史边界见 [`README.md`](README.md)（本目录）；多账本只读聚合见 [`task_state_unified_resolve.md`](task_state_unified_resolve.md)。

---

## 1. 五层模型（自下而上）

```text
┌─────────────────────────────────────────────────────────────┐
│  L5 技能与编排契约（SKILL.md / RFV / swarm gate）            │  ← 人读：lane、verify_commands、拒因
├─────────────────────────────────────────────────────────────┤
│  L4 宿主投影（Cursor hooks.json / Codex hooks）               │  ← 只转发事件 + 超时；不写业务规则长文
├─────────────────────────────────────────────────────────────┤
│  L3 router-rs 控制面（cursor/codex hook、framework_* CLI）   │  ← 门控状态机、合并续跑提示、证据追加
├─────────────────────────────────────────────────────────────┤
│  L2 连续性真源（artifacts/current/*、EVIDENCE_INDEX、账本）   │  ← 可审计事实；唯一跨会话接力面
├─────────────────────────────────────────────────────────────┤
│  L1 可执行验证（cargo/pytest/… 与 exit code）                │  ← 真值来源；hook 只记录不「替跑」
└─────────────────────────────────────────────────────────────┘
```

**依赖方向**：只允许 **L1→L2→L3→L4** 向上消费事实；**L5 不得绕过 L2** 自称「完成」（除非显式软门禁场景）。

---

## 2. 各层职责与反模式

| 层 | 应当做 | 不应当做 |
|----|--------|----------|
| **L5** | 定义 lane 边界、`verify_commands`、轮次契约 | 在技能里复制门控实现或发明第二套 `EVIDENCE_INDEX` 格式 |
| **L4** | 调用 `router-rs`、串联 stdin、固定超时 | 写长段策略 prose；把验证逻辑写进 bash（除极窄如 rustfmt） |
| **L3** | 合并续跑块、解析 PostTool、写 `EVIDENCE_INDEX` 行、review gate | 承担业务领域规则（论文/产品文案）；无节制增加「一次性 env」 |
| **L2** | 单一真源目录、schema 版本、任务指针 | 把聊天当状态机 |
| **L1** | 产生 exit code / 测试报告 | 无 |

---

## 3. 两条「主数据流」

### 3.1 证据流（executable → audit）

- **触发**：终端类 PostTool（Codex/Cursor）命中验证启发式；或 `rust-lint` / `hook-evidence-append`。
- **落点**：`artifacts/current/<task_id>/EVIDENCE_INDEX.json`。
- **原则**：启发式是 **L3 的采样器**，不是真理；冷僻命令走 **显式 append** 或 **收窄后的模式**（见下节「扩展」）。

### 3.2 续跑提示流（disk → hook → model）

- **触发**：`GOAL_STATE`、`RFV_LOOP_STATE` 等处于 active + hook 未关闭。
- **落点**：`additional_context` / `followup_message`。
- **原则**：**叙事型 nudge**（如「推理深度」）属于 **产品文案**，应与 **门控算法** 解耦；若需开关或多语言，应进 **统一配置面**（见 §5），而不是在 Rust 里散落 `const` + 新 env。

---

## 4. 「推理深度」在上层的位置

- **语义归属**：L5 契约（`reasoning-depth-contract.md`、RFV skill）定义 **何为正确工作方式**。
- **运行时归属**：L3 仅做 **轻量提醒**（可选、可关），**不得**用长文案替代 L1/L2。
- **单一结论**：深度来自 **分工 + 可执行验证 + 落盘**；不是单模型 CoT。该结论 **只应在一处** 写长文，其余层 **链接或一行指针**。

---

## 5. 扩展规则（避免继续「加抽象」失控）

1. **新宿主行为** → 先标清属于 L3 哪条管道（PostTool / Stop / refresh），再实现；禁止在 L4 bash 里复制 L3 逻辑。
2. **新 env 开关** → 仅在 **跨用户可见噪音 / 合规** 需要时添加；优先收束到 `router_env_flags` + 文档表，**禁止**在随机模块读裸 `std::env::var`。
3. **新验证启发式** → 必须 **可测**（单测含命令样例）；宁可 **少而准**，用 `hook-evidence-append` 补长尾。
4. **新 operator 文案** → 默认进 **L5 文档**；注入宿主时以 **`configs/framework/HARNESS_OPERATOR_NUDGES.json`** 为真源（`router-rs` 启动时合并内置默认值）。**关闭全部此类注入**：`ROUTER_RS_HARNESS_OPERATOR_NUDGES=0`（与其它 `ROUTER_RS_*` 软关断语义一致）。Schema 说明见同目录 `HARNESS_OPERATOR_NUDGES_SCHEMA.json`。

---

## 6. 与仓库文件的映射

| 概念 | 主要落地 |
|------|----------|
| L4 | `.cursor/hooks.json`、`.cursor/hooks/*.sh`、Codex `hooks.json` |
| L3 | `scripts/router-rs/src/{cursor_hooks,codex_hooks,framework_runtime,rfv_loop,autopilot_goal,task_state,task_state_aggregate,task_command,task_write_lock,harness_operator_nudges}.rs` |
| L2 | `artifacts/current/`、`configs/framework/*SCHEMA*` |
| L5 | `skills/**/SKILL.md`、`skills/review-fix-verify-loop/references/*` |

---

## 7. 刻意不做的事

- 不在本文定义具体模型名、定价或 Cursor Auto 路由（属产品侧，易变）。
- 不把 **closeout 硬门禁** 规则重复写全（真源仍在 `closeout_enforcement` + schema）。

维护：当新增一类 hook 行为或全局开关时，**至少更新本节 §5 与 §6 表格中的一行**，避免「只有代码没有地图」。

**读模型**：多账本统一只读聚合见 [`task_state_unified_resolve.md`](task_state_unified_resolve.md)（`router-rs` `task_state` / `framework task-state-resolve`；阶段 3 另见 `TASK_STATE.json` 与 `framework task-state-aggregate-sync`）。完整文档目录见 [`README.md`](README.md)。
