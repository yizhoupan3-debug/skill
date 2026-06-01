---
last_verified: "2026-06-02"
depends_on:
  - ../closeout_enforcement.md
  - ../rfv_loop_harness.md
  - ../references/rfv-loop/external-research-harness.md
  - ../references/rfv-loop/reasoning-depth-contract.md
---

# Closeout 与深度调研对齐

[返回索引](index.md)

## 6. Closeout 与深度

- closeout 真相来自证据、diff、产物和明确 blocker，而不是"我完成了"的叙述。
- `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 未设置且非 CI 时，允许本地软门禁；CI 或显式开启时走硬门禁。
- `DepthCompliance`、`GOAL_STATE`、`RFV_LOOP_STATE` 的更细语义由 `router-rs` 和对应 schema 负责；本文件只定义它们属于 L2/L3 正式控制面，而不是聊天补丁。
- **`depth_compliance` advisory rollup**：真源 `core/antigravity/src/task_state.rs` 的 `depth_compliance_aggregate`；`ROUTER_RS_DEPTH_COMPLIANCE_HINT` 为 **遗留 env / 单测**，**无** SessionStart 或其它 hook 注入。

### 深度调研：三轨对齐（无自动合并）

宿主里「说要深度调研」并不等于自动落盘 RFV 外研账本；三件事分工如下（仅指针，不重述全文）：

- **Execute 内核**：`research_mode`/live prompt 塑形（[`runtime_ops.inc`](../../core/router-rs/src/cli/runtime_ops.inc) 的 `infer_research_mode` / `build_live_execute_prompt`）— 只管当次执行的回复结构提示，不起账本。
- **Plan 闸门**：`plan_profile: research` 与 Cursor 规则见 [`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md)、[`.cursor/rules/cursor-plan-output.mdc`](../../.cursor/rules/cursor-plan-output.mdc) — 约束计划形态，不经 hook 程序化强制 RFV。
- **账本与外研**：可审计多轮 + 结构化 `external_research` 走 **`framework_rfv_loop`** / `RFV_LOOP_STATE.json`，见 [`docs/rfv_loop_harness.md`](../rfv_loop_harness.md)、[`references/rfv-loop/external-research-harness.md`](../references/rfv-loop/external-research-harness.md) 与 [`references/rfv-loop/reasoning-depth-contract.md`](../references/rfv-loop/reasoning-depth-contract.md)。

**操作者自检（最短）**：Execute 判 `deep` 只影响当轮 prompt，**不**创建 `RFV_LOOP_STATE`；要可审计外研须显式跑 `framework_rfv_loop`。`RUNTIME_REGISTRY.json` 里 `research_contract` 为叙事契约，Execute 塑形真源在 `runtime_ops.inc`（见 `external-research-harness.md` 与 `tests/policy_contracts.rs` 防漂移用例）。默认 `ROUTER_RS_DEPTH_SCORE_MODE=legacy` 下，仅有结构化外研轮次**不等于** `depth_score` 第三分已满；需 checkpoint / 对抗轮或 `strict`。Cursor 出站 `additional_context` 前缀保留裁剪（第 4.2 节），硬短码与合并后的 schema 指针优先落在段落前部更易存活。
