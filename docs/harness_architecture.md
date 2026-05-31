# Continuity harness architecture

本文件是 harness 的唯一长解释面，负责说明：

- 五层结构与数据流
- 热路径应该读什么、不该读什么
- hook 可见提示如何投影
- 哪些环境变量仍然有效
- 哪些兼容层被刻意删除

跨宿主执行协议、语言与收口原则见仓库根 [`AGENTS.md`](../AGENTS.md)。宿主接入见 [`host_adapter_contract.md`](host_adapter_contract.md)。Rust 运行时契约见 [`rust_contracts.md`](rust_contracts.md)。Hook **flock 锁序**（L1/L2/L3）见 [`hook_lock_order.md`](hook_lock_order.md)。

## 1. 五层模型

```text
L5  Skill / RFV / orchestration contract
L4  Host projection (Cursor/Codex/Claude hooks)
L3  router-rs control plane
L2  Continuity artifacts under artifacts/current/
L1  Executable verification and exit codes
```

依赖方向只允许 `L1 -> L2 -> L3 -> L4 -> L5` 向上消费事实。L5 不得绕过 L2 自称“已完成”。

**术语**：上文 **L4 = 宿主 hook 投影**（Cursor/Codex/Claude）。`SKILL.md` frontmatter 里的 **`routing_layer: L4`** 表示 **冷表 manifest 技能**（如 `python-env-management`），与 harness 层号**不是同一概念**。

## 2. 热路径真源

### 2.1 SessionStart（2026-05 连续性拔除后）

- Codex / Cursor SessionStart **不**注入连续性 digest、`GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` 或 `depth_compliance` / `depth_compliance_refresh_hint` 段落（2026-05 hook 路径已拔除；`depth_compliance_aggregate` 与 `depth_compliance_refresh_hint` 仍供 **stdio / `task-state-resolve` / 遗留 env 单测**，**不**注入 SessionStart）。
- **`ROUTER_RS_OPERATOR_INJECT` 总闸**：闸关时 Codex 无 `additionalContext`、Cursor `additional_context` 为空；闸开时仅允许 **轻量** 动态信息：Cursor **`Repo:`** 单行（[`handle_session_start`](../core/router-rs/src/hosts/cursor_hooks/handlers_parts/handlers_session.inc.rs)）；Codex `SessionStart source:`。**无** digest、**无** SessionStart 指针 hint（分裂观测用 `framework task-state-resolve` / `framework doctor`）。
- **禁止**：repo onboarding、Quick Reference、Build & test、Key paths、Tool cost hierarchy 等静态说明；禁止恢复 hook 驱动的 `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE`。
- 出站仍按 UTF-8 **字节**预算截断（Cursor `...[~trunc]`；Codex `...`）。
- 宏目标 / RFV 多轮：仅 **`framework_goal_drive` / `framework_rfv_loop` stdio** 与 `artifacts/current/<task_id>/` 手动画板；见 [`AGENTS_OPERATOR_SURFACE.md`](references/AGENTS_OPERATOR_SURFACE.md)。

### 2.2 Skill routing

- `skills/SKILL_ROUTING_RUNTIME.json` 是唯一**热路由**真源；运行时由 `core/router-rs/src/route/records.rs` 机读。
- 热 runtime 只保留：`version`、`schema_version`、`scope`、`keys`、`skills`。
- 任何 plugin、projection、routing explain、兼容迁移叙事都不进热 runtime。
- 冷真源 = **编译器 / 契约 / CI 消费集**，并非 hook 热路径读物：
  - [`skills/SKILL_PLUGIN_CATALOG.json`](../skills/SKILL_PLUGIN_CATALOG.json)：`router-rs framework skills` 校验/刷新；policy contract 消费。
  - [`skills/SKILL_ROUTING_METADATA.json`](../skills/SKILL_ROUTING_METADATA.json)：路由 metadata 真源；`tests/policy_contracts.rs` 与 `host_integration.rs` 校验。
  - [`skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`](../skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json)：路由解释器衍生物，policy 契约校验目标；不要把它当 router-rs 第二真源去删。

### 2.3 控制面配置与生成物（2026-05-20 硬化）

| 真源 | 用途 |
|------|------|
| [`configs/framework/RUNTIME_REGISTRY.json`](../configs/framework/RUNTIME_REGISTRY.json) | 闭集宿主、`review_gate.deep_gate_lanes`、profile 投影；**运行时**经 [`runtime_registry/mod.rs`](../core/router-rs/src/runtime_registry/mod.rs) 从磁盘读取（`registry_loader.rs` 仅为 re-export shim；**非** `include_str!` 嵌入）。读盘失败时 lane 判定 **fail-closed**（不计入深度 lane）；`framework doctor` 探测 snapshot。改 lane 后重启 hook 子进程即可，**无需** `cargo build`。 |
| [`configs/framework/host_projection_narrative.json`](../configs/framework/host_projection_narrative.json) | 各宿主 framework 投影内的 **My lifecycle 默认链** 与 **review findings-only** 英文段落；`framework host-integration install` 渲染时读取。叙事政策仍以 [`AGENTS.md`](../AGENTS.md) 为跨宿主真源，本 JSON 仅为安装产物文案真源。 |
| [`configs/framework/GENERATED_ARTIFACTS.json`](../configs/framework/GENERATED_ARTIFACTS.json) | 声明须纳入版本库的生成物路径、generator 命令与 `compare` 模式（`byte-for-byte` / `normalized-text`）。 |

**`generated-artifacts-status` 两种模式**（[`host_integration.rs`](../core/router-rs/src/host_integration.rs)）：

| 模式 | 触发 | 行为 |
|------|------|------|
| **metadata-only** | `framework doctor`（默认）；CLI `--skip-generator-run`；env `ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS=1` | 不跑 manifest generator；检查声明路径存在、forbidden marker、undeclared 路径与 per-artifact `clean`；`manifest_status.mode` = `manifest-backed-generated-artifact-metadata-only`。 |
| **drift-gate（全量）** | `framework maint update-one-shot`；显式全量探针 | 在隔离 temp root 执行声明 generator（含 `host-integration install` 等慢步骤，默认单 generator **300s** 超时，可用 `ROUTER_RS_GENERATOR_TIMEOUT_SECONDS` 覆盖），再 byte/normalized 对比 checked-in 与再生副本。 |

集成测试与日常 `doctor` 应使用 **metadata-only**；提交前维护流仍须至少一次 **drift-gate** 绿（见 [`skills/update/SKILL.md`](../skills/update/SKILL.md)）。

**Cursor project L4 手维护面**（相对 Codex/Claude 的 `GENERATED_ARTIFACTS` drift-gate **不对称**；`framework host-integration install --to cursor` **不**托管 project hooks）：

| 路径 | 说明 |
|------|------|
| `.cursor/hooks.json` | 7 事件闭集；parity 见 `scripts/ci/check-cursor-hooks-parity.sh` |
| `.cursor/router-rs-hook.env` | hook 子进程 env |
| `.cursor/rules/*.mdc` | gate/plan alwaysApply rules |
| `.cursor/commands/*.md` | My lifecycle slash stubs（4 文件） |
| `.cursor/agents/deep-reviewer.md` | 深度 review lane 定义 |
| `configs/framework/cursor-router-rs-hook.sh` | launcher（repo 根相对路径经 hooks.json command 引用） |

User-scope install 产出：`~/.cursor/rules/framework.mdc`（叙事真源 `host_projection_narrative.json`）。详见 [`docs/hosts/cursor.md`](hosts/cursor.md)。

**Companion 生成物**（`SKILL_PLUGIN_CATALOG.json`、`SKILL_HEALTH_MANIFEST.json` 等）：`source_of_truth: false` stub；默认 `cargo test --test policy_contracts` 只断言闭集与形态，**不**恢复历史 `capability_classes` 富契约（见 `plugin_catalog_routing_metadata_and_health_manifest_form_closed_loop`，`tests/policy_contracts.rs`）。

## 3. 主数据流

### 3.1 证据流

`L1` 验证命令或验证形工具输出
→ `router-rs` 采样/追加
→ `artifacts/current/<task_id>/EVIDENCE_INDEX.json`
→ closeout / gate 消费。

原则：

- hook 只记录证据，不替模型“编造验证通过”。
- 长尾命令通过显式 append 或更窄启发式补齐。

**Task ledger 写入（跨进程）**：`GOAL_STATE` / `RFV_LOOP_STATE` / `STEP_LEDGER.jsonl` append（`framework step-ledger`）、session artifact 批量写 / `EVIDENCE_INDEX` 的 RMW，默认经 [`task_write_lock.rs`](../core/antigravity/src/utils/task_write_lock.rs) 在 `artifacts/current/.router-rs.task-ledger.lock` 上持 **`flock(2)`**，按**仓库根**互斥（多宿主 hook 子进程不共享 Rust 进程内互斥量）。`EVIDENCE_INDEX` 仍可再持单文件旁路锁（`runtime_storage::acquire_runtime_path_lock`）；锁序约定为 **先 repo ledger flock，再 path lock**。`runtime_storage` 的 **memory** 回归后端对 `append_text` 使用进程内 `Mutex` 串行化（不参与 repo flock）。`ROUTER_RS_TASK_LEDGER_FLOCK=0|false|off|no` 可关闭 flock（如不稳定的网络 FS），关闭后并行写为 best-effort；`router-rs framework doctor` 在 flock 关闭时会打印醒目提示。`TASK_STATE.json` 仅为投影，权威仍以分文件为准；聚合失败会以 stderr 前缀 **`TASK_STATE_AGGREGATE_SYNC_FAILED`** 记录；单独跑 `task-state-aggregate-sync` 的修复路径不替代上述写锁。

### 3.1.1 轨迹与 step 恢复流

`TRACE_EVENTS.jsonl` 是轨迹诊断流，复用 `trace_runtime record-event`，用于记录
`task_id / owner / gate / overlay / horizon / phase / tool_or_lane / status /
failure_class / evidence_ref / context_bytes` 等复盘字段。它不替代
`EVIDENCE_INDEX`：前者解释过程，后者支撑验证。

`STEP_LEDGER.jsonl` 是 task-scoped 长任务 step 恢复流，由
`router-rs framework step-ledger` 追加；`TASK_STATE.json` 只投影摘要
（条数、状态计数、最新 step/ref），不把整份 ledger 注入模型上下文。

统一 failure taxonomy 的机器可读表在
`configs/framework/HARNESS_FAILURE_TAXONOMY.json`；behavioral eval fixture 表在
`configs/framework/HARNESS_BEHAVIORAL_EVAL_CASES.json`。这些配置只描述评估与分类，
不成为第二套路由、证据或 closeout 真源。

### 3.2 续跑与门控流

磁盘状态
→ `router-rs`
→ 宿主输出字段
→ 模型可见提示。

固定投影策略：

- 硬门控短码进 `followup_message`
- advisory 提示进 `additional_context`

**2026-05**：hook **不**再投影 `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE`；续跑仅 stdio + 手动画板（见 §2.1）。

**可读模型**：当 `active_task.json` 指向的任务缺少可读 `GOAL_STATE.json`，但 `focus_task.json` 指向另一任务且该任务盘上存在合法 GOAL 时，[`resolve_task_view`](../core/antigravity/src/task_state.rs) 会在 `resolution_notes` 写入短码 `continuity:active_goal_missing_focus_has_goal`（仅观测；[`read_goal_state_for_hydration`](../core/router-rs/src/autopilot_goal.rs) 仍不回退 focus）。`framework task-state-resolve` 可透出该行提示。

**Stop 自动 checkpoint（已删除，2026-05）**：Cursor/Codex hook **不再**在 Stop 写盘。显式 checkpoint 仅用 Desktop MCP `session_checkpoint` 或 `framework_session_artifact_write` stdio。

**L1 运行时视图与读模型**：[`load_framework_runtime_view`](../core/router-rs/src/framework_runtime/runtime_view.rs) 的 `active_task_id` 选择与 [`resolve_task_view`](../core/antigravity/src/task_state.rs) 一致（`override > active > focus > supervisor`），见 [`task_state_unified_resolve.md`](task_state_unified_resolve.md)。

## 4. Hook 文案策略

- 对模型可见的 hook 文案默认短码优先、短句优先。
- `REVIEW_GATE`、`AG_FOLLOWUP`、`CLOSEOUT_FOLLOWUP`、`SESSION_CLOSE_STYLE` 等保留单段紧凑输出；**无** `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` hook 注入。
- lock failure、degraded mode、pre-goal 等提示应压缩为单行或极短段，最多附一个动作提示。
- 禁止把长策略解释混进 runtime 提示；长解释只留在本文件和相关契约文档。

### 4.1 Claude `claude hook` 与 Cursor stdin 误接

`router-rs claude hook` 若误收到 Cursor hook 的 stdin，仅在 JSON **顶层**同时满足：**非空字符串** `cursor_version`、**数组** `workspace_roots`、以及 **非空字符串** `hook_event_name` 或 `hookEventName` 之一时整段静默（`suppressOutput`），避免把 Claude 管道接到 Cursor 事件流。  
**不**再对嵌套字段里的 `/.cursor/` 等路径做子串匹配：否则合法 Claude 载荷（例如编辑 `.cursor/` 下文件）会被误判为 Cursor 而旁路门禁。实现见 [`core/router-rs/src/hosts/claude_hooks.rs`](../core/router-rs/src/hosts/claude_hooks.rs)（`payload_looks_like_cursor_hook_stdin`）。

**stdin 体量**：`router-rs claude hook` 从 stdin 读取的原始输入 **上限 4 MiB**（与 Codex hook 限量读取一致），溢出返回错误；合法 JSON 解析错误返回 `stdin_json_invalid:` 前缀消息。

`router-rs framework host-integration install --to claude` 写入的 hook **command** 须将 stdin **原样交给** `router-rs claude hook`；不在 Bash 层用 `grep` 对 `cursor_version` / `workspace_roots` / `/.cursor/` 做预短路（历史上曾与 Rust 真源分裂）。安装串见 [`host_integration.rs`](../core/router-rs/src/host_integration.rs) 的 `build_router_rs_claude_hook_command`。

### 4.2 Cursor `additional_context`：合并链路与出站字节上限

- **合并**：各 hook handler 通过 [`merge_additional_context`](../core/router-rs/src/hosts/cursor_hooks/handlers.rs) 将 advisory 段落追加进出站 JSON 的 `additional_context` 字符串（多事件可达多次追加）。
- **出站裁剪**：Cursor CLI 入口 [`review_gate.rs`](../core/router-rs/src/review_gate.rs) 在写出 stdout 前调用 [`apply_cursor_hook_output_policy`](../core/router-rs/src/hosts/cursor_hooks/handlers.rs)：对 `additional_context` 与超长 `followup_message` 使用 **`truncate_cursor_hook_outbound_context_preserving_gate`** / **`truncate_cursor_hook_followup_preserving_review_gate`** — UTF-8 字节上限取自 **`ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS`**（[`router_env_flags.rs`](../core/router-rs/src/router_env_flags.rs) ，默认 8192，clamp 1024–65536）；超长时 **优先保留** 含 `router-rs REVIEW_GATE`、`REVIEW_GATE detail` 前缀的行，对其余 filler 做前缀截断并以固定 **`...[~trunc]`** 结束（见 §5 环境变量表脚注）。仍建议将硬门控信息放在 `followup_message` 的 `router-rs …` 行。
- **对照**：Codex `additionalContext` 另有字节上限（[`codex_hooks/mod.rs`](../core/router-rs/src/hosts/codex_hooks/mod.rs) `truncate_codex_additional_context_bytes`）；两套宿主互不替代。

### 4.3 仿宿主续跑行（`RG_FOLLOWUP` 等）与机读真源

- Cursor hook 出站 JSON 中，**深度审稿未完成**与 **Autopilot goal 缺块** 所依赖的机读 leader 真源为 **`router-rs REVIEW_GATE incomplete …`**、**`router-rs AG_FOLLOWUP missing_parts=…`**（均须以 ASCII 前缀 **`router-rs `** 起行；实现见 [`cursor_hooks/handlers.rs`](../core/router-rs/src/hosts/cursor_hooks/handlers.rs)）。审稿链未收尾时以 **`router-rs REVIEW_GATE incomplete`** 行内 `need=`、`hint=` 排障。
- **其它**由本仓库注入的软提示（如 **`CLOSEOUT_FOLLOWUP`、`SESSION_CLOSE_STYLE`** 等）仍按该列表及各自字段形态识别；历史 `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` 行若出现在旧会话 scrape 中，**不是**当前 harness 注入。
- **`RG_FOLLOWUP`、`RG FOLLOWUP`、`RG-FOLLOWUP`**，以及**无** `router-rs ` 前缀、却仿照 `*_FOLLOWUP` 与 `missing_parts=` / `escalation=` 组合的整行，**不是**本 harness 的注入格式；常见来源为助手复述或误粘贴。真源里 **`router-rs AG_FOLLOWUP` 的 `missing_parts=`** 由 [`ship_readiness.rs`](../core/router-rs/src/ship_readiness.rs) 拼接，并附 **`primary_fix=`**；盘上已有 `GOAL_STATE` 时 Stop **不**再读聊天 `Goal:` 标题。不会出现 `independent_subagent_or_reject_reason` 这类占位串。
- **出站剥线**：[`review_gate.rs`](../core/router-rs/src/review_gate.rs) 写出 stdout 前对 `followup_message` / `additional_context` 调用 [`scrub_followup_fields_in_hook_output`](../core/router-rs/src/autopilot_goal.rs)；[`merge_additional_context`](../core/router-rs/src/hosts/cursor_hooks/handlers.rs) 在合并追加时亦对片段与整段复用 `scrub_spoof_host_followup_lines`。助手**聊天可见正文**不经该剥线，故仍可能看到仿造行——判读时 **优先** 核对 **`router-rs …` 审稿/goal 行** 与 **`.cursor/hook-state` / 磁盘门控**；**不排除** 同字段内 §4 所列其它真源短码段落。
- **Codex**：`additionalContext` 的截断与注入形态以 [`codex_hooks/mod.rs`](../core/router-rs/src/hosts/codex_hooks/mod.rs) 及上文 **§4.2「对照」** 为准，与 Cursor 出站 **不互为替身**。
- **清门**：不得以整段会话 scrape 误认拒因；[`saw_reject_reason`](../core/router-rs/src/hook_common.rs) 仅承认：`signal_text` 中的拒因 token、单独成行的 `rg_clear` / `/rg_clear`，以及**用户本轮**粘贴的 **goal** `ag_followup…` 前缀行。**用户粘贴 `RG_FOLLOWUP…` 不作为合法清门**（与上条「仿冒」一致）；曾依赖旧行为时请改用 `rg_clear` 或拒因 token。

## 5. 开关面

只保留真正改变行为边界的少量开关；文案分叉和投影位置分叉不再保留。

### 5.0 Review gate（操作叙述真源）

**唯一操作手册**：[`framework_operator_primer.md`](framework_operator_primer.md)（§机读短码、Spawn-first、深度审稿 `REVIEW_GATE`、混用武装顺序、fork_context 排障）。本文件只保留实现锚点：

- **Lane 闭集**：`configs/framework/RUNTIME_REGISTRY.json` → `review_gate.deep_gate_lanes`（Cursor/Codex）；Claude 另见 `claude_reviewer_lanes`。跨宿主差异表：[`host_adapter_contract.md`](host_adapter_contract.md) §0.1。
- **机器判定**：[`hook_common.rs`](../core/router-rs/src/hook_common.rs) `is_deep_review_gate_lane_normalized`；`fork_context` 解析：[`review_gate_engine.rs`](../core/router-rs/src/review_gate_engine.rs) `fork_context_from_values`。
- **状态机实现**：Cursor multiset — [`cursor_hooks/handlers.rs`](../core/router-rs/src/hosts/cursor_hooks/handlers.rs) `ReviewGateState`；Codex phase — [`codex_hooks/mod.rs`](../core/router-rs/src/hosts/codex_hooks/mod.rs) `CodexLifecycleContextState`；Claude — [`claude_hooks.rs`](../core/router-rs/src/hosts/claude_hooks.rs)。
- **透镜与产出形状**：[`skills/code-review-deep/SKILL.md`](../skills/code-review-deep/SKILL.md)（本文件不展开 checklist）。
- **应急关闭**：`ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` / `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE` / `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE`（见下表）。

**环境变量表（脚注，读表前先看）**：下表及 §4.2 中凡出现 `…_CHARS`、`…_MAX`、`…_MAX_BYTES` 或未写明「字符数」的上下文长度，**一律按 UTF-8 字节**计（实现见 [`router_env_flags.rs`](../core/router-rs/src/router_env_flags.rs)、[`cursor_hooks/handlers.rs`](../core/router-rs/src/hosts/cursor_hooks/handlers.rs)；变量名遗留 `_CHARS` 时不改语义）。出站裁剪超长时，前缀保留后末尾会追加**固定截断标记**（与纯 `...` 相比可区分「预算截断」与门控逻辑未满足）。子代理 **`fork_context`**：**推荐** JSON **布尔**；实现亦接受布尔字符串及 **JSON 整数** `0`/`1`（见 [`fork_context_from_values`](../core/router-rs/src/review_gate_engine.rs)）；其它 **Number** 与**字段缺失**均不为 `false`。

| 环境变量 | 默认 | 作用 |
|---------|------|------|
| `ROUTER_RS_OPERATOR_INJECT` | 开 | 总闸：关闭 SessionStart 轻量 advisory、`SESSION_CLOSE_STYLE`、`paper_adversarial` 等；不影响 `REVIEW_GATE` / closeout 硬短码 |
| `ROUTER_RS_HARNESS_OPERATOR_NUDGES` | 开 | 仅关闭 operator nudge 文案；不改 gate 逻辑 |
| `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` | **关** | **仅** `1`：PostTool → `EVIDENCE_INDEX` 自动追加（opt-in） |
| `ROUTER_RS_CONTINUITY_WRITE_JOURNAL` | **关** | **仅** `1`：`CONTINUITY_JOURNAL.json`（opt-in） |
| `ROUTER_RS_TASK_STATE_AGGREGATE_AUTO` | **关** | **仅** `1`：自动刷新 `TASK_STATE.json`；否则 `framework task-state-aggregate-sync` |
| `ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS` | 1200，clamp 256–8192 | Cursor SessionStart `additional_context` 合成字节上限（实现为 **`Repo:`** 单行 + 截断） |
| `ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE` | 开 | **仅** `0`/`false`/`off`/`no`：关闭 Stop 软 `SESSION_CLOSE_STYLE` 单行收口提示 |
| `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK` | 关 | Cursor beforeSubmit 中显式开启论文/手稿强对抗审稿短段 |
| `ROUTER_RS_CURSOR_PAPER_PROSE_HOOK` | **开** | Cursor beforeSubmit：手稿写作/润色 prose chain 短段（**默认开**；`0`/`false`/`off`/`no` 关） |
| `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE` | 开 | Cursor beforeSubmit：子代理/Task **继承主会话模型** 单行 nudge（registry `subagent_model_inherit_nudge`）；**仅** `0`/`false`/`off`/`no` 关闭；与 my-light / REVIEW_GATE 无关 |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED` | 关 | 显式开启 Cursor `/implementx` pre-goal beforeSubmit 提示（env 名保留） |
| `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` | 开 | **默认开启**（unset 即 strict，[`router_rs_cursor_pre_goal_strict_disk_enabled`](../core/router-rs/src/router_env_flags.rs) 为 default-true）：**禁止**仅凭磁盘 `GOAL_STATE` hydration 将 `pre_goal_review_satisfied` 置真（beforeSubmit 与 Stop 均适用）；**仅** `0`/`false`/`off`/`no` 恢复历史宽松语义；pre-goal 仍可由 subagent / `reject_reason` / nag cap 等满足 |
| `ROUTER_RS_CURSOR_REVIEW_GATE_MODE` | strict（unset） | **仅** `lite`：启用 `review_lite_pending_cycle_keys`（`id:` only）；非 `id:` 回退 strict。`strict` 显式值与 unset 等价 |
| `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` | 开 | **仅** `0`/`false`/`off`/`no`：关闭 Cursor 可数深度 lane 在 `fork_context` **缺失**时的 `false` 推断；显式 `fork_context: true` 永不算独立证据 |
| `ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX` | 32，clamp 1–256 | `review_subagent_pending_cycle_keys` multiset 上限；满则拒绝新 key 且 subagentStart 不增加 open 计数 |
| `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP` | 关 | **仅** `1`/`true`/`yes`/`on` 时：Cursor `SessionEnd` 在清当前 `session_key` 与全局 tmp 孤儿之外，对 `.cursor/hook-state/` 再做**全目录前缀清扫**（历史行为），用于单人单会话下清 session_id/cwd 漂移遗留；**默认关**以免同仓库并行 Cursor 会话的门控状态被其它会话的 SessionEnd 误删 |
| `ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS` | 关 | **仅** `1`/`true`/`yes`/`on`：5 个减法事件在未写入 `hooks.json` 时仍走完整 handler（单测/对照）；默认 no-op |
| `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` | 关 | 应急：beforeSubmit 仅 `continue:true`；Stop 仅 closeout + `SESSION_CLOSE_STYLE`（无 goal/RFV 续跑） |
| `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES` | 默认 **8** | `REVIEW_GATE` 未满足时 Stop 降频；超 cap 后短 `followup_message` + `additional_context` 细节段 |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES` | 内置数值默认 | `/implementx` pre-goal beforeSubmit 提示次数上限（[`cursor_hooks/handlers.rs`](../core/router-rs/src/hosts/cursor_hooks/handlers.rs)） |
| `ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS` | 内置数值默认 | 仍可打开的并发 subagent 上限，`0` 关闭限制 |
| `ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS` | 内置数值默认（2h） | subagent stale 判定阈值（秒）；**仅** `0`/`false`/`off`/`no`：**关闭**自动 stale 回收（不重置 `active_subagent_count`、不 prune pending）；清门仍用 `rg_clear` / SessionEnd / `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` |
| `ROUTER_RS_CURSOR_SESSION_NAMESPACE` | unset | 同仓库并行 Cursor 会话时分流 `.cursor/hook-state` 文件名组件（[`cursor_hooks/handlers.rs`](../core/router-rs/src/hosts/cursor_hooks/handlers.rs)） |
| `ROUTER_RS_CURSOR_WORKSPACE_ROOT` | unset | Cursor workspace/repo root 解析兜底（[`repo_root.rs`](../core/router-rs/src/hosts/cursor_hooks/repo_root.rs)） |
| `ROUTER_RS_CURSOR_TERMINAL_KILL_MODE` | 内置默认 | 终端 kill 策略（[`cursor_hooks/handlers.rs`](../core/router-rs/src/hosts/cursor_hooks/handlers.rs)） |
| `ROUTER_RS_CURSOR_KILL_STALE_TERMINALS` | 内置阈值默认 | 陈旧会话终端清理（[`cursor_hooks/handlers.rs`](../core/router-rs/src/hosts/cursor_hooks/handlers.rs)） |
| `ROUTER_RS_CLOSEOUT_ENFORCEMENT` | 本地软、CI 硬 | 控制 closeout record 是否程序化硬门禁 |
| `ROUTER_RS_DEPTH_SCORE_MODE` | `legacy` | `strict` 时启用更严格 depth 第三分公式 |
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` | 640，clamp 256–8192 | Codex SessionStart `additionalContext` **字节**上限（遗留变量名；[`codex_additional_context_max_bytes`](../core/router-rs/src/hosts/codex_hooks/mod.rs)） |
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` | unset（可选覆盖） | 若设置：**优先于** `_MAX`；二者解析均为 UTF-8 **字节**，clamp 256–8192 |
| `ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY` | **开**（unset=on） | Codex `UserPromptSubmit` / `PostToolUse` / `Stop` 在无法从 hook stdin（`session_id`/`sessionId`/`conversation_id`/`conversationId`/`thread_id`/`threadId`）或环境 `CODEX_SESSION_ID`/`CODEX_CONVERSATION_ID` 得到稳定会话键时 **block**（`SessionStart` 不受影响）；legacy `=0`/`false`/`off`/`no` 关闭硬前置并使用 **repo+cwd+payload session** 确定性 fallback（可选 `ROUTER_RS_CODEX_HOOK_STATE_SALT`；`cwd` 空 stderr 警告） |
| `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE` | 关 | **仅** `1`/`true`/`yes`/`on`：关闭 Codex `CODEX_REVIEW_GATE` 硬门控（unset 保持启用；对称 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` / `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE`） |
| `ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS` | 关 | **仅** `1`：Codex `Stop` 在 `stop_hook_active` 重放时跳过 review/closeout 门控 |
| `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE` | 关 | **仅**当值为 `1`/`true`/`yes`/`on`（大小写不敏感）时关闭 Claude Code `CLAUDE_REVIEW_GATE`（含 UserPromptSubmit review 提示）；unset、空串与其它任意值均保持启用（与 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 对称）。可选：在项目根 `.claude/router-rs-hook.env` 写 `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE=1`（由安装的 Claude hook command 包装自动加载；重装/合并 hook 后以 `core/router-rs` 的 Claude settings 投影为准） |
| `ROUTER_RS_CLAUDE_SESSION_NAMESPACE` | unset | **仅 Claude** session 状态：当 stdin 缺少会话 id、`cwd` 类字段又不足以分流时，同仓多会话可能共用 `.claude/review_gate_*.json` / `hook_state_*.json`；设非空串可为并行会话隔离状态文件名组件（语义对齐 `ROUTER_RS_CURSOR_SESSION_NAMESPACE`；见 [`claude_hooks.rs`](../core/router-rs/src/hosts/claude_hooks.rs) `claude_session_key`） |
| `ROUTER_RS_TASK_LEDGER_FLOCK` | 开 | **仅** `0`/`false`/`off`/`no`（与 `ROUTER_RS_OPERATOR_INJECT` 同类 default-true 语义）关闭 `artifacts/current/.router-rs.task-ledger.lock` 的 `flock`；关闭后多进程并行写账本为 best-effort（见 §3.1 证据流下 Task ledger 段） |
| `ROUTER_RS_CLIPBOARD_PATH` | unset（可选） | CLI/read_clipboard：自定义剪贴板文件路径（[`runtime_ops.inc`](../core/router-rs/src/cli/runtime_ops.inc)） |
| `ROUTER_RS_STORAGE_ROOT` | unset（可选） | `runtime_storage` 持久根重写 |
| `ROUTER_RS_BIN` | unset（可选） | host_integration：`router-rs` 可执行路径提示 |
| `ROUTER_RS_GENERATOR_TIMEOUT_SECONDS` | unset → **300s** | `generated-artifacts-status` drift-gate：单条 manifest generator 超时（秒）；`0` 仍用默认 300s |
| `ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS` | 关 | **仅** `1`/`true`/`yes`/`on`：等同 CLI `--skip-generator-run`（metadata-only，不跑 generator、不比 drift） |
| `ROUTER_RS_SHARED_TARGET` | unset（可选） | `router_self` 共享 target 路径 |
| `ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS` / `ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS` | unset | framework `/update` maint 流专用（[`framework_maint.rs`](../core/router-rs/src/framework_maint.rs)） |
已退役的文案分叉、beforeSubmit 双续跑、聊天区投影切换、静默例外模式、Plan→Build goal 门控开关都不再支持。**2026-05 拔除、env 名保留但无操作**：`ROUTER_RS_GOAL_CONTINUE_HOOK`、`ROUTER_RS_RFV_LOOP_HOOK`（及兼容 `ROUTER_RS_AUTOPILOT_DRIVE_HOOK`）、`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT`、`ROUTER_RS_DEPTH_COMPLIANCE_HINT`；SessionStart `digest` 模式字符串亦不再产出 digest 正文。

## 6. Closeout 与深度

- closeout 真相来自证据、diff、产物和明确 blocker，而不是“我完成了”的叙述。
- `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 未设置且非 CI 时，允许本地软门禁；CI 或显式开启时走硬门禁。
- `DepthCompliance`、`GOAL_STATE`、`RFV_LOOP_STATE` 的更细语义由 `router-rs` 和对应 schema 负责；本文件只定义它们属于 L2/L3 正式控制面，而不是聊天补丁。
- **`depth_compliance` advisory rollup**：真源 `core/antigravity/src/task_state.rs` 的 `depth_compliance_aggregate`；`ROUTER_RS_DEPTH_COMPLIANCE_HINT` 为 **遗留 env / 单测**，**无** SessionStart 或其它 hook 注入。

### 深度调研：三轨对齐（无自动合并）

宿主里「说要深度调研」并不等于自动落盘 RFV 外研账本；三件事分工如下（仅指针，不重述全文）：

- **Execute 内核**：`research_mode`/live prompt 塑形（[`runtime_ops.inc`](../core/router-rs/src/cli/runtime_ops.inc) 的 `infer_research_mode` / `build_live_execute_prompt`）— 只管当次执行的回复结构提示，不起账本。
- **Plan 闸门**：`plan_profile: research` 与 Cursor 规则见 [`skills/plan-mode/SKILL.md`](../skills/plan-mode/SKILL.md)、[`.cursor/rules/cursor-plan-output.mdc`](../.cursor/rules/cursor-plan-output.mdc) — 约束计划形态，不经 hook 程序化强制 RFV。
- **账本与外研**：可审计多轮 + 结构化 `external_research` 走 **`framework_rfv_loop`** / `RFV_LOOP_STATE.json`，见 [`docs/rfv_loop_harness.md`](rfv_loop_harness.md)、[`references/rfv-loop/external-research-harness.md`](references/rfv-loop/external-research-harness.md) 与 [`references/rfv-loop/reasoning-depth-contract.md`](references/rfv-loop/reasoning-depth-contract.md)。

**操作者自检（最短）**：Execute 判 `deep` 只影响当轮 prompt，**不**创建 `RFV_LOOP_STATE`；要可审计外研须显式跑 `framework_rfv_loop`。`RUNTIME_REGISTRY.json` 里 `research_contract` 为叙事契约，Execute 塑形真源在 `runtime_ops.inc`（见 `external-research-harness.md` 与 `tests/policy_contracts.rs` 防漂移用例）。默认 `ROUTER_RS_DEPTH_SCORE_MODE=legacy` 下，仅有结构化外研轮次**不等于** `depth_score` 第三分已满；需 checkpoint / 对抗轮或 `strict`。Cursor 出站 `additional_context` 前缀保留裁剪（第 4.2 节），硬短码与合并后的 schema 指针优先落在段落前部更易存活。

## 7. 扩展规则

1. 新宿主行为先判断属于哪条现有管道，再实现；不要在 L4 脚本复制 L3 逻辑。
2. 新环境变量只在确实改变行为边界时添加；默认合并分支而不是继续加旋钮。
3. 新 operator 文案默认写进配置或文档，不写进零散 `const`。
4. 新验证启发式必须有测试；宁可少而准。
5. 改动 SessionStart 或 routing 热路径时，先证明 token 预算更小、真源更少，而不是只换说法。

## 8. 文件映射

| 概念 | 主要落地 |
|------|----------|
| L4 hooks | `.cursor/hooks.json`、`.codex/hooks.json`、各宿主 hook 配置 |
| L3 control plane | `core/router-rs/src/`：`hosts/codex_hooks/mod.rs`、`claude_hooks.rs`、`cursor_hooks/mod.rs`（`handlers.rs` + `handlers_parts/*.inc.rs`）、`autopilot_goal.rs`、`rfv_loop.rs`、`framework_runtime/mod.rs`、`task_state.rs`、`host_integration.rs` |
| L2 continuity | `artifacts/current/`、`TRACE_EVENTS.jsonl`、`STEP_LEDGER.jsonl`、`configs/framework/*SCHEMA*` |
| Skill 热路由（router-rs hot path） | `skills/SKILL_ROUTING_RUNTIME.json` |
| Skill 伴生元数据（**非**每 prompt 热路径；`SKILL_PLUGIN_CATALOG` / `SKILL_ROUTING_RUNTIME_EXPLAIN` 由 refresh / policy / CI 消费；**`SKILL_ROUTING_METADATA.json` 在 `load_records_from_runtime` 时 merge**，见 `route/records.rs` `merge_sidecar_route_metadata_from_runtime`） | `skills/SKILL_PLUGIN_CATALOG.json`、`skills/SKILL_ROUTING_METADATA.json`、`skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`（EXPLAIN：CI/companion/人读，router route 模块不读） |
| Host registry（磁盘 loader） | `configs/framework/RUNTIME_REGISTRY.json` + `core/router-rs/src/runtime_registry/mod.rs`（shim：`registry_loader.rs`） |
| 宿主投影 My/review 文案 | `configs/framework/host_projection_narrative.json` |
| 生成物 manifest / drift | `configs/framework/GENERATED_ARTIFACTS.json` + `framework host-integration generated-artifacts-status` |
| 任务 schema drift / Cursor hooks 减法闭集 | `core/router-rs/src/schema_drift.rs`、`hosts/cursor_hooks/subtraction.rs`；CLI `router-rs schema-drift {contract,baseline,check}` |
| 弱模型 / 上下文预算调研索引 | 见 `skills/SKILL_ROUTING_RUNTIME.json` 的 hot/cold 分布，或运行 `router-rs eval route` |
| 全面自检清单（减法审计，非合并门槛） | 运行 `router-rs framework maint update-audit --repo-root .` |

## 9. 刻意不做的事

- 不在 SessionStart 注入 repo onboarding。
- 不保留旧 runtime shape 兼容层。
- 不在 `AGENTS.md`、Cursor rules、docs、hook 文案里重复展开同一套长叙事。
- 不为了“也许以后需要”保留 verbose 模式、双通道切换或多事件重复续跑注入。
