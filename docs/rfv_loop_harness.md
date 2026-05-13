# RFV  harness 参考（`framework_rfv_loop` / `RFV_LOOP_STATE.json`）

> **范围**：RFV 多轮账本与 lane 契约说明；**不进入** `skills/SKILL_ROUTING_RUNTIME.json` 热路由。  
> Rust 行为真源：`scripts/router-rs/src/rfv_loop.rs` 及相关 hook 合并逻辑。

**不进入热 skill 路由**（`skills/SKILL_ROUTING_RUNTIME.json` 不含本主题的独立 slug）。此处保留 **`framework_rfv_loop` 字段契约、lane 模板与推理深度说明**，供 Autopilot / Team / [`loop`](../skills/loop/SKILL.md) 与工具链引用。

编排前先过 [`agent-swarm-orchestration`](../skills/agent-swarm-orchestration/SKILL.md)：确认各 lane 可独立、`fix_scope` disjoint、且 `verify_commands` 已定义；若启用外部调研并行，还须明确 **external 与 review 的只读边界** 及汇总责任在 supervisor。不满足则拒绝 spawning 并给出 reject reason。

可复制 lane 与轮次日志模板见 [references/rfv-loop/lane-templates.md](references/rfv-loop/lane-templates.md)。

## Rust 轮次真源（长任务必用）

当 `max_rounds` 较大（例如 **100**）或跨多会话执行时，必须用 `router-rs` 落盘轮次账本，避免仅靠聊天上下文：

- 文件：`artifacts/current/<task_id>/RFV_LOOP_STATE.json`
- stdio op：**`framework_rfv_loop`**
  - `operation: start` — 字段含 `goal`、`max_rounds`、`allow_external_research`、`parallel_external_with_review`（默认 `true`）、`prefer_structured_external_research`（**当 `allow_external_research=true` 时默认 `true`**；显式 `false` 可关闭；`null`/缺省走默认）、**`external_research_strict`**（默认 **`true`**；显式 `false` 关闭；旧 `RFV_LOOP_STATE.json` 缺此键时 `append_round` 视为宽松）、可选 **`close_gates`**（显式 RFV 收口硬门禁，默认 off；语义见 [`reasoning-depth-contract.md`](references/rfv-loop/reasoning-depth-contract.md)）、`review_scope`、`fix_scope`、`verify_commands`、`stop_when`
  - `operation: append_round` — 每轮结束后 supervisor 写入：`round`、`review_summary`、`external_research_summary`（可空）、可选结构化 **`external_research`**（校验见 [references/rfv-loop/external-research-harness.md](references/rfv-loop/external-research-harness.md)）、`fix_summary`、`verify_result`（`PASS|FAIL|SKIPPED`）、`supervisor_decision`（`continue|close|block`）、`reason`
  - `operation: status`
- `max_rounds` 在 Rust 侧有 **硬上限 1000**（防止误填天文数字）；超过会截断并在响应中带 `warning`。
- **Cursor**：若 `.cursor/hooks.json` 接入 `router-rs cursor hook`，且 **`RFV_LOOP_STATE.json`** 中 **`loop_status=active`**，Stop / beforeSubmit 可合并 **RFV_LOOP_CONTINUE** 跟进；preCompact 可附带一行 RFV 摘要。关闭注入：`ROUTER_RS_RFV_LOOP_HOOK=0`。

**GOAL（`GOAL_STATE.json` / `framework_autopilot_goal`）与 RFV 账本的关系**：

- 同一目录 `artifacts/current/<task_id>/` **可以**先后或交替出现 `GOAL_STATE.json` 与 `RFV_LOOP_STATE.json` 文件。
- **不能**在同一任务上「双 macro **同时要求续跑**」：`GOAL` 处于需要续跑的 running/drive、且 **`RFV_LOOP_STATE.loop_status=active`** 时，`resolve_task_view` 会得到 **Conflict**（`autopilot_goal_and_rfv_loop_both_active`），真源：[`scripts/router-rs/src/task_state.rs`](../scripts/router-rs/src/task_state.rs) 的 `classify_control_mode`。
- **`framework_rfv_loop` `operation: start`** 会摘掉同任务的 `GOAL_STATE.json`，真源：`deactivate_goal_for_conflict_with_rfv`（[`scripts/router-rs/src/autopilot_goal.rs`](../scripts/router-rs/src/autopilot_goal.rs)）。
- 编排上：**二选一作为主控制面**，或先做 RFV 多轮账本、或先做 Autopilot GOAL；需要切换时重建/显式收口另一套账本，避免误认为「可与 RFV active 并行续跑 GOAL macro」。

### 迁移说明（`prefer_structured_external_research` 默认）

自本仓库行为更新起：**`allow_external_research=true` 且未传 `prefer_structured_external_research`** 时，落盘默认 **`true`**（旧脚本若依赖「默认 false、不触发单行 struct hint」须显式传 **`prefer_structured_external_research: false`**）。

### 迁移说明（`external_research_strict` 默认 **`true`**）

**`append_round` 写入结构化 `external_research` 时**：新启动的 RFV 任务默认 **`external_research_strict: true`**，blob 须同时满足结构化形状与 **strict** 附加规则（见 `validate_external_research_strict`）。**旧磁盘 `RFV_LOOP_STATE.json` 若缺少该键**：`append_round` 按 **宽松** 路径接受仅结构化校验通过的 blob（与 `rfv_loop.rs` 一致）。若要从宽松迁到 strict：用 **`operation: start`** 重建账本并传入显式布尔，或手改 JSON 写入 **`"external_research_strict": true`** 后再按 strict 补全外研字段。

### 推理深度契约（与可审计链）

- **真源**：[references/rfv-loop/reasoning-depth-contract.md](references/rfv-loop/reasoning-depth-contract.md) — **不靠单模型拉长 CoT**；靠 **`review ∥ external → fix → verify`** + **`EVIDENCE_INDEX` / `append_round`** 形成可审计链。
- **提升调研深度的计划（契约级）**：同文件 §**提升调研深度的 harness 方向** — **A** 外研 API 式输出（Claims / Contradiction sweep / Unknowns）；**B** 检索留下可复核轨迹并与定量复算命令同源；**C** 多视角真并行、角色分离（禁止同上下文换帽）。
- **宿主注入文案**：默认 RFV / Autopilot 续跑保持紧凑；RFV 在 `allow_external_research=true` 时追加检索 / `retrieval_trace` 短句，在 goal/scope/verify 命中数学或 checker 信号时追加数理 witness/checker 短句。`configs/framework/HARNESS_OPERATOR_NUDGES.json` 仍是文案真源；`ROUTER_RS_HARNESS_OPERATOR_NUDGES=0` 可关闭这些 nudge；`ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT=0` 仅关结构化短提示。
- **数理 / STEM 契约长文**：[references/rfv-loop/math-reasoning-harness.md](references/rfv-loop/math-reasoning-harness.md)（witness、符号 verifier、反事实探针；与 lane 模板同目录）。
- **外研不得顶替 verify**：external 只产出可引用结论与假设；**Pass/Fail 只认可执行验证**。定量复算的 **replay** 与 `cargo test` 同类 spirit：写入 `verify_commands` 或 `quantitative_replays` 并由 verifier / supervisor 执行，证据进 `EVIDENCE_INDEX`（或等价记录）。

### 可执行验证与证据落盘（PostTool 启发式 vs 显式 append）

- **宿主 PostTool 自动追加**（`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` 未关闭、连续性已初始化）：Codex/Cursor 在 **`postToolUse` / `postTool`** 路径上解析终端类 tool 载荷，命令字符串命中 `router-rs` 内置验证启发式（如 `cargo test`、`pytest`、**窄域数理/形式化**子串见 `framework_runtime::shell_command_looks_like_verification`）时，写入 `EVIDENCE_INDEX.json` 行（`kind` 如 `cursor_post_tool_verification` / `codex_post_tool_verification`）。**不替跑命令**，只记账采样到的 preview / exit hint。
- **`framework hook-evidence-append`**：任意宿主或人工可把 **`command_preview`** 写入证据索引；非 Cursor `cursor_*` 来源时仍须通过同一套验证启发式（否则 `skipped`），用于 **PostTool 未覆盖** 的长尾命令。
- `verify_commands` 建议优先选仓库内 **短、确定性** 命令，并与上述启发式有交集，便于自动记账；仍不匹配时用 **`hook-evidence-append`** 或选用带关键字的安全拼法（**避免**依赖裸 `python` 作为唯一验证信号）。

## When to use

- 用户明确要求可配置轮次的闭环执行，而不是单轮修复
- 任务适合拆为独立 lane：审查、修复、验证
- 需要每轮都保留 supervisor 集成判断和停机决策
- 需要把失败模式从“感觉完成”改成“证据驱动继续/停止”
- 需要 **外部调研** 与仓库内审查 **对照**（并行或串行均可，但须在契约里写明）
- **`max_rounds` 很大**（如 100）：必须启用 **`framework_rfv_loop` 账本**，并在 `stop_when` 中保留提前收敛条件（见下）

## Do not use

- 小任务在一轮内可稳定完成
- 三个 lane 不能独立（共享强上下文或写入范围重叠严重）
- 缺少可执行验证命令或可观测验证标准
- 用户明确要求不要使用 subagent

## Loop contract

先建立最小契约：

```text
goal:
max_rounds:
round_timeout_hint:
review_scope:
fix_scope:
verify_commands:
stop_when:
```

默认 `max_rounds=3`。**用户显式给定轮次（如 100）时**：以用户值写入 `RFV_LOOP_STATE.max_rounds`，但仍必须满足下列 **提前停机** 规则之一才可 `close`，禁止“刷满轮次才算完”：

`stop_when` 至少包含一条（推荐多条同时启用）：

- verifier 全部通过且无 A 级 blocker
- **连续两轮无实质 delta**（无新修复、无新高优先级发现、外部调研无新的高置信结论）
- 到达 `max_rounds`（耗尽预算）
- 出现明确外部 blocker（缺权限、缺数据、人工确认）
- **外部调研与内部审查结论收敛**（对关键假设/风险登记达成一致或明确存疑点）

## Round execution model

每一轮推荐 **两阶段并行 + 两阶段串行**（在 `allow_external_research=true` 且 `parallel_external_with_review=true` 时）：

**阶段 A（可并行）**

1. `reviewer lane`（独立 subagent，仓库内只读为主）
2. `external research lane`（独立 subagent，仅做 **可引用来源** 的调研；默认 **禁止** 改仓库；与 reviewer **并行**启动）

supervisor **汇总** A 阶段：合并内部 findings 与外部证据，形成本轮唯一 handoff（写入 `append_round` 前的中间笔记即可）。

**阶段 B（串行）**

3. `fixer lane`（独立 subagent，仅允许改 `fix_scope`）
4. `verifier lane`（独立 subagent，执行 `verify_commands`；默认只报告不修复）

**阶段 C**

5. supervisor 合并证据，**`framework_rfv_loop` `append_round`**，并决定：
   - close（完成）
   - continue（下一轮）
   - block（输出 blocker 与 next action）

每个 lane 必须 **每轮新起独立 worker**，不复用同一会话在 reviewer / external / fixer / verifier 间切换。

## Lane prompt contract

每个 lane 的提示必须包含：

- lane goal（本轮目标）
- allowed scope / forbidden scope（文件或模块边界）
- required output format（固定字段）
- verification evidence requirement（命令、日志、差异）

统一输出格式：

```text
changed_files:
findings_or_fixes:
verification:
risk:
next_action:
```

## Supervisor responsibilities

- 维护每轮状态：`round`, `key_findings`, `applied_fixes`, `verification_result`
- 防止 lane 写入重叠；发现重叠时暂停并重分配范围
- 仅 supervisor 决定是否进入下一轮
- 最终输出包含：
  - 轮次执行摘要
  - 最终验证证据
  - 残余风险或 blocker

## Minimal execution checklist

```text
- [ ] contract complete
- [ ] round 1 review/fix/verify done
- [ ] convergence decision recorded
- [ ] if needed, next round started with updated scopes
- [ ] final verification evidence attached
```
