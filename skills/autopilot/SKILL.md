---
name: autopilot
description: |
  Repo-native `/autopilot`：goal-style 连续执行、地平线切片、continuity 硬接力；
  bounded sidecar 并行（lane 清晰时），宏任务优先写满 `artifacts/current` 以便跨轮无隙推进。
  Use only when the user explicitly invokes `/autopilot`.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /autopilot
  - /autopilot-quick
  - /autopilot-deep
framework_roles:
  - alias
  - executor
framework_phase: execution
risk: medium
source: project
metadata:
  version: "1.2.0"
  platforms: [codex, cursor]
  tags:
    - autopilot
    - alias
    - execution
    - macro-task
---

# autopilot

面向 **`/autopilot`** 的宿主执行协议：先取 **live 投影**，再读 **continuity**，最后才动代码。

## 1. Live 投影（真源入口）

```bash
router-rs framework alias autopilot
```

- **cwd**：可在仓库根或其子目录（含 `scripts/router-rs/`）执行；`router-rs` 会将当前目录或 stdio 载荷里的 `repo_root` **解析到 framework 仓库根**后再读 `RUNTIME_REGISTRY` 与 `artifacts/current`，无需手动 `cd` 到根目录。
- 优先使用返回体里的 `state_machine`、`entry_contract`、`continuity`；不要重复展开整份 SKILL 长文，除非投影不足。
- Registry 真源：`configs/framework/RUNTIME_REGISTRY.json` → `framework_commands.autopilot`。
- 跨框架总政策：`skills/skill-framework-developer/SKILL.md`（若存在）。

### 1.1 与 Cursor / IDE 文案区分 + 前置条件

- **IDE / 产品内的「Autopilot」≠ 本仓库的 `/autopilot` harness**：除非本仓库的 **L4** hook 管线（含 **`router-rs`**）已按宿主装好并生效；否则不要把两者当成同一套能力。
- **分层指针**：L5 连续性文档见 [`docs/harness_architecture.md`](../../docs/harness_architecture.md)；Cursor 侧 hook 挂载见仓库根 **`.cursor/hooks.json`**（与 L4/L3/L2 职责分界以该文档为准，此处不重复全文）。
- **本地链自检（verify-local-chain）**：
  - `.cursor/hooks.json` 存在且指向可用的 **`router-rs`**（通常为 release 构建产物）。
  - `artifacts/current/` 对本会话可写（continuity / goal 写入所需）。

## 2. 「一口气」= 连续推进，不是单轮魔法

宿主（Cursor/Codex）仍可能有：**单轮工具预算、上下文压缩、会话切断**。宏任务的「一口气」指：

1. **在同一轮内**：在限制下尽量完成「计划 → 实现 → 验证 → 修」中最长的一条龙；禁止用长篇 planning 代替落地。
2. **跨轮时**：每一轮结束都留下 **可冷启动的 continuity**，使下一轮 **零复述成本** 接上，直到 `Done when` 满足或唯一 blocker 明确。

做不到「单轮跑完整个多周项目」时，必须用 **地平线（Horizon）** 把大目标切成多段，每段仍遵守同一套 goal 契约。

## 3. 宏任务启动清单（`/autopilot` 后立刻执行）

按顺序，缺啥补啥：

1. 运行或等价获取：`router-rs framework alias autopilot`（含 continuity 摘要）。
2. 若存在 `artifacts/current/`：读 `SESSION_SUMMARY.md`、`NEXT_ACTIONS.json`（及 `EVIDENCE_INDEX.json` 若已有）；对齐 `active_task.json` / `.supervisor_state.json` 指针（若仓库使用）。
3. 若用户目标仍模糊或根因不明：按 registry **reroute** 到 `deepinterview`，不要硬 autopilot。
4. 发布 **Goal 契约**（见下节），再进入实现。

## 4. Goal 契约（强制，未发布不得宣称 autopilot 已启动）

至少包含这些标题（可用简短列表）：

- **Goal**：可判定的一句话目标。
- **Non-goals**：明确不做什么（防范围蠕变）。
- **Done when**：验收条件，可勾选、可测试或可看 diff。
- **Validation commands**：具体命令（如 `cargo test …`）；未跑则说明原因与风险。
- **Checkpoint plan**：本轮要推进到的 checkpoint 名称 + 预计证据类型。

当输入源是「review 列出的一堆问题」且按 P0/P1/P2 分级时，`/autopilot` 的**默认**范围与验收是：

- **默认 Scope**：修复 **全部** review 发现（P0+P1+P2…），直到清单清零或仅剩明确 blocker。
- **默认 Done when**：清单全部关闭/修复，并且关键验证命令通过（至少覆盖会破坏主流程/CI 的部分）。
- **例外**：只有当用户明确说“只修 P0/先修 P0”时，才允许把本轮 Horizon 或整体 Goal 限定为 P0-only。

Cursor 侧 `router-rs` 可能要求 **pre-goal 独立 reviewer subagent**（在未落盘 `GOAL_STATE` 等条件下）或 **单行 reject_reason token** 供清门；遵守宿主 gate。真实 hook 短码以宿主注入为准（常见 **`AG_FOLLOWUP`**、**`AUTOPILOT_DRIVE`** 等）；**不要**在可见回复里自拟整段仿宿主的机读块或伪造 hook 版面。

## 5. 地平线 Horizon（宏任务核心）

当目标满足任一信号时启用：**多模块/多包、跨层（API+存储+UI）、大量文件、长周期、依赖未知或外部事实多**。

每个 Horizon 必须定义：

- **Scope**：本段唯一边界（文件/模块/行为）。
- **Exit**：离开本段前必须成立的条件（测试绿、lint、或明确 blocker）。
- **Artifacts**：本段对 `SESSION_SUMMARY` / `NEXT_ACTIONS` 的增量（下一段第一件事读这些）。

**禁止**在宏任务中「整盘规划写完再动手」：第一个 Horizon 要 **可在一两轮内验证**；验证通过再扩下一 Horizon。

## 6. 执行循环（每轮最少交付）

每完成一轮工具工作，在回复中**显式**包含：

1. **Checkpoint**：相对上一状态做了什么（一行起句即可）。
2. **Next**：下一条可执行动作（对应 `NEXT_ACTIONS` 语义）。
3. **Verify**：已跑命令 + 结果，或「未跑 + 原因 + 风险」。

若 continuity 已初始化：对「看起来像验证」的 shell（如 `cargo test`、`cargo check`、`pytest`），宿主 hook 可能写入 `EVIDENCE_INDEX.json`；不要依赖口述代替证据。

## 7. 并行 lane（bounded sidecar）

- 仅在 **写入范围不交叠、验证命令已定义** 时并行；并行上限以 `configs/framework/RUNTIME_REGISTRY.json` 的 `autonomy_contract.auto_agent_orchestration` 为准（勿硬编码旧「3 条」叙述）。
- 拒不并行时：内部选定 **一个** reject_reason；**面向用户**用业务语言说明阻塞与下一步。仅在宿主明确要求清门时，**单独一行**输出上述 token **之一**，不要展开拒因说明或自拟非宿主注入的续跑机读块。
- 主线程负责：集成、共享决策、**最终整体验证**。

## 8. 研究与 `/autopilot-deep`

- 外部论断必须可溯源；brownfield 优先 **仓库内证据**。
- 宏任务且依赖外部事实、对比方案、或「为什么」链长：用 **`/autopilot-deep`**（或 registry 中 deep 模式），遵守 deep 研究契约（多源、不确定性与反证登记等，见 alias 投影里的 `research_contract`）。

## 9. 宿主差异

- **Codex CLI**：可选用 `rust-session-supervisor` / tmux worker 等长会话外壳（以仓库与 AGENTS 描述为准）。
- **Cursor**：通常 **无** Codex 同款 tmux supervisor；长程依赖 **`artifacts/current` 接力**。`/autopilot` **不再**与 `/team` 等入口叠乘「并行委托」门控：评审后修复轮应主要受 **goal** 与 **`GOAL_STATE`** 约束；**不要**在可见回复里自拟非宿主注入的续跑机读长文。若磁盘上已有 **`GOAL_STATE.json`**（`framework_autopilot_goal start`），beforeSubmit **不再强制** pre-goal reviewer subagent 提示。若已写入 **`GOAL_STATE.json` 且 `drive_until_done`**，Stop/提交时 hook 会注入 **AUTOPILOT_DRIVE** 续跑提示（见第 11 节）；关闭：`ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0`。

## 10. 收口与暂停

- 收口：仅当 **Done when 满足** 且有 **验证证据**，或 **单一、明确的 blocker**（含所需用户输入/权限）。
- **goal_pause**：用户明确要求暂停时；之后不得隐式恢复，需显式 **goal_resume** 语义（由宿主对话触发）。
- 不把「未验证的乐观结论」当完成。
- **程序化完成态**（`framework session artifact write` 等路径声明 `completed`/`passed` 等）：在 CI / `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 硬门禁下须附带能通过评估的 **`closeout_record`**（字段见 `configs/framework/CLOSEOUT_RECORD_SCHEMA.json` 与 `docs/closeout_enforcement.md`）。本地软门禁仍建议按同一结构写证据，便于审计与跨宿主对齐。

## 11. Rust 续跑真源（`GOAL_STATE` + stdio）

在 **`active_task.json` 已指向当前 task**（通常已由 `framework session artifact write` / continuity 初始化）时，用 router-rs 落盘目标机并打开「未完成就跟进」：

```bash
printf '%s\n' '{"id":1,"op":"framework_autopilot_goal","payload":{"repo_root":"'"$REPO_ROOT"'","operation":"start","goal":"<可判定目标>","done_when":["<验收条件>"],"validation_commands":["<验证命令>"],"drive_until_done":true}}' | router-rs --stdio-json
```

- 写入：`artifacts/current/<task_id>/GOAL_STATE.json`（schema `router-rs-autopilot-goal-v1`）。
- 其它 `operation`：`status` | `checkpoint`（需 `note`）| `pause` | `resume` | `complete` | `block`（需 `blocker`）| `clear`（删除当前任务目录下 `GOAL_STATE.json`，停止续跑注入）。
- **真完成**必须调用 `complete`，否则 Cursor 侧可能持续收到 **AUTOPILOT_DRIVE**。

多轮 **review → fix → verify** 大轮次（含外部调研并行 lane）的字段与 lane 契约见 harness 参考 [`rfv_loop_harness.md`](../docs/rfv_loop_harness.md)（**非热 skill 路由**）；用户侧对抗式渐进披露见 [`loop`](../loop/SKILL.md)。轮次账本使用 **`framework_rfv_loop`**（`RFV_LOOP_STATE.json`，与 `GOAL_STATE.json` 同任务目录）。

**推理深度真源**：分工 + 可执行验证 + 可审计链的具体契约见 [`reasoning-depth-contract.md`](../docs/references/rfv-loop/reasoning-depth-contract.md)。**数理 / STEM** 见 [`math-reasoning-harness.md`](../docs/references/rfv-loop/math-reasoning-harness.md)（宿主短句：`HARNESS_OPERATOR_NUDGES.json` 的 **`math_reasoning_harness_line`**）。**深度外研 / 检索** 同契约 §B：`retrieval_trace` + contradiction sweep（宿主短句：**`retrieval_trace_harness_line`**）。Autopilot 同样适用——`Validation commands` 必须给出可执行命令；声称完成前须有 `EVIDENCE_INDEX` 成功行或显式 blocker。

**Codex SessionStart continuity digest**（`build_framework_continuity_digest_prompt`，与 `framework snapshot` / `contract-summary` 同源读模型）会在 **`prompt` 文本末尾拼接整段 `GOAL_STATE` 约束**、追加 `HARNESS_OPERATOR_NUDGES` 中的「推理深度」一句，并附带「**深度自检三问**」（来自 reasoning-depth-contract）——目标从「纯文件」变成 **会话注入里可直接执行的 checklist**；机器可读字段用 **`router-rs framework task-state-resolve`** 或读磁盘 `GOAL_STATE.json`。

Canonical owner: `autopilot`.
