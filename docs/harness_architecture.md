# Continuity harness architecture

本文件是 harness 的唯一长解释面，负责说明：

- 五层结构与数据流
- 热路径应该读什么、不该读什么
- hook 可见提示如何投影
- 哪些环境变量仍然有效
- 哪些兼容层被刻意删除

跨宿主执行协议、语言与收口原则见仓库根 [`AGENTS.md`](../AGENTS.md)。宿主接入见 [`host_adapter_contract.md`](host_adapter_contract.md)。Rust 运行时契约见 [`rust_contracts.md`](rust_contracts.md)。

## 1. 五层模型

```text
L5  Skill / RFV / orchestration contract
L4  Host projection (Cursor/Codex/Claude hooks)
L3  router-rs control plane
L2  Continuity artifacts under artifacts/current/
L1  Executable verification and exit codes
```

依赖方向只允许 `L1 -> L2 -> L3 -> L4 -> L5` 向上消费事实。L5 不得绕过 L2 自称“已完成”。

## 2. 热路径真源

### 2.1 SessionStart

- Codex / Cursor SessionStart 只允许注入动态活信息。
- **`ROUTER_RS_OPERATOR_INJECT` 总闸**：Codex `SessionStart` 与 Cursor `SessionStart` 在闸关时均**不**注入连续性 advisory（Codex 返回无 `additionalContext`；Cursor 返回空字符串 `additional_context`）。**例外**：Cursor 仍可在闸关时执行 `SessionStart` 所需的**非 advisory**副作用（例如终端 baseline ledger 初始化），不视为绕过总闸。
- 允许内容：以 [`build_framework_continuity_digest_prompt`](../scripts/router-rs/src/framework_runtime/continuity_digest.rs) 为核心的连续性 digest（含 `depth_compliance_refresh_hint`、可选 `GOAL_STATE` 段落等），外加 `Repo:` 与 Codex 侧的 `SessionStart source:`；Cursor 在存在 `continuity:active_goal_missing_focus_has_goal` 观测时，将 [`CONTINUITY_ACTIVE_FOCUS_GOAL_MISMATCH_HINT_ZH`](../scripts/router-rs/src/task_state.rs) **置于 digest 正文之前**，以便与 Codex 一样尽量扛前缀截断。
- 禁止内容：repo onboarding、Quick Reference、Build & test、Key paths、Tool cost hierarchy 之类静态说明。
- Cursor / Codex SessionStart 出站均按 UTF-8 **字节**预算截断；Cursor 使用与 Stop 等路径相同的 [`truncate_cursor_hook_outbound_context`](../scripts/router-rs/src/cursor_hooks/frag_04_review_gate_runtime.rs)（末尾 **`...[~trunc]`** 固定后缀），与 Codex `truncate_codex_additional_context_bytes` 的 `...` 形态不同但语义等价（均为「预算截断」可观测标记）。
- **Codex vs Cursor 段落顺序**：digest 内 ZH mismatch 行位于 `depth_compliance_refresh_hint` **之前**；Codex 整段 digest 仍可能先于 Repo 行被小预算截断，运维请对照源码拼接顺序与 `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX(_BYTES)`。

### 2.2 Skill routing

- `skills/SKILL_ROUTING_RUNTIME.json` 是唯一**热路由**真源；运行时由 `scripts/router-rs/src/route/records.rs` 机读。
- 热 runtime 只保留：`version`、`schema_version`、`scope`、`keys`、`skills`。
- 任何 plugin、projection、routing explain、兼容迁移叙事都不进热 runtime。
- 冷真源 = **编译器 / 契约 / CI 消费集**，并非 hook 热路径读物：
  - [`skills/SKILL_PLUGIN_CATALOG.json`](../skills/SKILL_PLUGIN_CATALOG.json)：`scripts/skill-compiler-rs/src/main.rs` 在 plugin 投影时消费。
  - [`skills/SKILL_ROUTING_METADATA.json`](../skills/SKILL_ROUTING_METADATA.json)：路由 metadata 真源；`tests/policy_contracts.rs` 与 `host_integration.rs` 校验。
  - [`skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`](../skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json)：路由解释器衍生物，policy 契约校验目标；不要把它当 router-rs 第二真源去删。

## 3. 主数据流

### 3.1 证据流

`L1` 验证命令或验证形工具输出
→ `router-rs` 采样/追加
→ `artifacts/current/<task_id>/EVIDENCE_INDEX.json`
→ closeout / digest / gate 消费。

原则：

- hook 只记录证据，不替模型“编造验证通过”。
- 长尾命令通过显式 append 或更窄启发式补齐。

**Task ledger 写入（跨进程）**：`GOAL_STATE` / `RFV_LOOP_STATE` / `STEP_LEDGER.jsonl` append（`framework step-ledger`）、session artifact 批量写 / `EVIDENCE_INDEX` 的 RMW，默认经 [`task_write_lock.rs`](../scripts/router-rs/src/task_write_lock.rs) 在 `artifacts/current/.router-rs.task-ledger.lock` 上持 **`flock(2)`**，按**仓库根**互斥（多宿主 hook 子进程不共享 Rust 进程内互斥量）。`EVIDENCE_INDEX` 仍可再持单文件旁路锁（`runtime_storage::acquire_runtime_path_lock`）；锁序约定为 **先 repo ledger flock，再 path lock**。`runtime_storage` 的 **memory** 回归后端对 `append_text` 使用进程内 `Mutex` 串行化（不参与 repo flock）。`ROUTER_RS_TASK_LEDGER_FLOCK=0|false|off|no` 可关闭 flock（如不稳定的网络 FS），关闭后并行写为 best-effort；`router-rs framework doctor` 在 flock 关闭时会打印醒目提示。`TASK_STATE.json` 仅为投影，权威仍以分文件为准；聚合失败会以 stderr 前缀 **`TASK_STATE_AGGREGATE_SYNC_FAILED`** 记录；单独跑 `task-state-aggregate-sync` 的修复路径不替代上述写锁。

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

不再保留“聊天区 vs additional_context”切换开关，也不在多个事件上重复投影同一段 Goal/RFV 续跑文案。

**可读模型**：当 `active_task.json` 指向的任务缺少可读 `GOAL_STATE.json`，但 `focus_task.json` 指向另一任务且该任务盘上存在合法 GOAL 时，[`resolve_task_view`](../scripts/router-rs/src/task_state.rs) 会在 `resolution_notes` 写入短码 `continuity:active_goal_missing_focus_has_goal`（仅观测；[`read_goal_state_for_hydration`](../scripts/router-rs/src/autopilot_goal.rs) 仍不回退 focus）。`framework task-state-resolve` 与连续性 digest（[`continuity_digest.rs`](../scripts/router-rs/src/framework_runtime/continuity_digest.rs)）可透出该行提示。

## 4. Hook 文案策略

- 对模型可见的 hook 文案默认短码优先、短句优先。
- `AUTOPILOT_DRIVE`、`RFV_LOOP_CONTINUE`、`REVIEW_GATE`、`AG_FOLLOWUP`、`CLOSEOUT_FOLLOWUP` 等保留单段紧凑输出。
- lock failure、degraded mode、pre-goal 等提示应压缩为单行或极短段，最多附一个动作提示。
- 禁止把长策略解释混进 runtime 提示；长解释只留在本文件和相关契约文档。

### 4.1 Claude `claude hook` 与 Cursor stdin 误接

`router-rs claude hook` 若误收到 Cursor hook 的 stdin，仅在 JSON **顶层**同时满足：**非空字符串** `cursor_version`、**数组** `workspace_roots`、以及 **非空字符串** `hook_event_name` 或 `hookEventName` 之一时整段静默（`suppressOutput`），避免把 Claude 管道接到 Cursor 事件流。  
**不**再对嵌套字段里的 `/.cursor/` 等路径做子串匹配：否则合法 Claude 载荷（例如编辑 `.cursor/` 下文件）会被误判为 Cursor 而旁路门禁。实现见 [`scripts/router-rs/src/claude_hooks.rs`](../scripts/router-rs/src/claude_hooks.rs)（`payload_looks_like_cursor_hook_stdin`）。

**stdin 体量**：`router-rs claude hook` 从 stdin 读取的原始输入 **上限 4 MiB**（与 Codex hook 限量读取一致），溢出返回错误；合法 JSON 解析错误返回 `stdin_json_invalid:` 前缀消息。

`router-rs framework install --to claude` 写入的 hook **command** 须将 stdin **原样交给** `router-rs claude hook`；不在 Bash 层用 `grep` 对 `cursor_version` / `workspace_roots` / `/.cursor/` 做预短路（历史上曾与 Rust 真源分裂）。安装串见 [`host_integration.rs`](../scripts/router-rs/src/host_integration.rs) 的 `build_router_rs_claude_hook_command`。

### 4.2 Cursor `additional_context`：合并链路与出站字节上限

- **合并**：各 hook handler 通过 [`merge_additional_context`](../scripts/router-rs/src/cursor_hooks/frag_03_paths_terminal_merge_lock_persist.rs) 将 advisory 段落追加进出站 JSON 的 `additional_context` 字符串（多事件可达多次追加）。
- **出站裁剪**：Cursor CLI 入口 [`review_gate.rs`](../scripts/router-rs/src/review_gate.rs) 在写出 stdout 前调用 [`apply_cursor_hook_output_policy`](../scripts/router-rs/src/cursor_hooks/frag_04_review_gate_runtime.rs)：对 `additional_context` 与超长 `followup_message` 使用 **`truncate_cursor_hook_outbound_context`** — UTF-8 字节上限取自 **`ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS`**（[`router_env_flags.rs`](../scripts/router-rs/src/router_env_flags.rs) ，默认 8192，clamp 1024–65536）；超长时 **保留前缀**，末尾以省略及固定 **`...[~trunc]`** 标记结束（见 §5 环境变量表脚注）。**因此较晚并入的段落更可能被砍掉**，排查时宜对照源码合并顺序或拆分到硬短码字段。
- **对照**：Codex `additionalContext` 另有字节上限（[`codex_hooks.rs`](../scripts/router-rs/src/codex_hooks.rs) `truncate_codex_additional_context_bytes`）；两套宿主互不替代。

### 4.3 仿宿主续跑行（`RG_FOLLOWUP` 等）与机读真源

- Cursor hook 出站 JSON 中，**深度审稿未完成**与 **Autopilot goal 缺块** 所依赖的机读 leader 真源为 **`router-rs REVIEW_GATE incomplete …`**、**`router-rs AG_FOLLOWUP missing_parts=…`**（均须以 ASCII 前缀 **`router-rs `** 起行；实现见 [`frag_04_review_gate_runtime.rs`](../scripts/router-rs/src/cursor_hooks/frag_04_review_gate_runtime.rs)）。审稿链未收尾时以 **`router-rs REVIEW_GATE incomplete`** 行内 `need=`、`hint=` 排障。
- **其它**由本仓库注入、在 **§4 上文列表** 中的续跑 / 软提示（如 **`AUTOPILOT_DRIVE`、`RFV_LOOP_CONTINUE`、`CLOSEOUT_FOLLOWUP`** 等）仍按该列表及各自字段形态识别，**不要求** `router-rs ` 前缀；与上条 **router-rs leader** 并存，勿用「凡机读句一律须 `router-rs `」误读。
- **`RG_FOLLOWUP`、`RG FOLLOWUP`、`RG-FOLLOWUP`**，以及**无** `router-rs ` 前缀、却仿照 `*_FOLLOWUP` 与 `missing_parts=` / `escalation=` 组合的整行，**不是**本 harness 的注入格式；常见来源为助手复述或误粘贴。真源里 **`router-rs AG_FOLLOWUP` 的 `missing_parts=`** 仅由 `goal_contract`、`checkpoint_progress`、`verification_or_blocker` 等片段逗号拼接（见同文件 `goal_missing_parts`），**不会出现** `independent_subagent_or_reject_reason` 这类占位串。
- **出站剥线**：[`review_gate.rs`](../scripts/router-rs/src/review_gate.rs) 写出 stdout 前对 `followup_message` / `additional_context` 调用 [`scrub_followup_fields_in_hook_output`](../scripts/router-rs/src/autopilot_goal.rs)；[`merge_additional_context`](../scripts/router-rs/src/cursor_hooks/frag_03_paths_terminal_merge_lock_persist.rs) 在合并追加时亦对片段与整段复用 `scrub_spoof_host_followup_lines`。助手**聊天可见正文**不经该剥线，故仍可能看到仿造行——判读时 **优先** 核对 **`router-rs …` 审稿/goal 行** 与 **`.cursor/hook-state` / 磁盘门控**；**不排除** 同字段内 §4 所列其它真源短码段落。
- **Codex**：`additionalContext` 的截断与注入形态以 [`codex_hooks.rs`](../scripts/router-rs/src/codex_hooks.rs) 及上文 **§4.2「对照」** 为准，与 Cursor 出站 **不互为替身**。
- **清门**：不得以整段会话 scrape 误认拒因；[`saw_reject_reason`](../scripts/router-rs/src/hook_common.rs) 仅承认：`signal_text` 中的拒因 token、单独成行的 `rg_clear` / `/rg_clear`，以及**用户本轮**粘贴的 **goal** `ag_followup…` 前缀行。**用户粘贴 `RG_FOLLOWUP…` 不作为合法清门**（与上条「仿冒」一致）；曾依赖旧行为时请改用 `rg_clear` 或拒因 token。

## 5. 开关面

只保留真正改变行为边界的少量开关；文案分叉和投影位置分叉不再保留。

### 5.0 Review gate：深度审稿可清点 lane（按宿主）

使用者一页速查（误报 `RG_FOLLOWUP` 行、**Stop 上 `REVIEW_GATE` 与 `AG_FOLLOWUP` 的先后**、同轮混写 `/autopilot` 时的武装结果）：[`framework_operator_primer.md`](framework_operator_primer.md)（§「机读短码真源与常见误报」「混用时的实际武装顺序」）。

**Cursor / Codex（`PostToolUse` / `Task` / `functions.subagent` 等记入 subagent 的载荷）**：仅当 lane 归为 **`general-purpose`** 或 **`best-of-n-runner`**（及 `normalize_subagent_type` 等价名，如 `generalpurpose` / `bestofnrunner`）且 **`fork_context`/`forkContext` 经 [`fork_context_from_values`](../scripts/router-rs/src/review_gate_engine.rs) 解析为逻辑 `false`** 时（典型为 JSON **布尔** `false`；亦接受布尔**字符串**与 JSON **整数** `0`（false）/`1`（true），见同节「排障」），可推进「独立审稿」相位并清点 `REVIEW_GATE`（Cursor）或 Codex independent reviewer 证据。**机器判定真源**：[`hook_common.rs`](../scripts/router-rs/src/hook_common.rs) → `is_deep_review_gate_lane_normalized`（对已规范化 lane 字符串求值）；本段不维护第二份可清点 lane 枚举表。

**Claude Code**：在满足显式 `fork_context=false` 的前提下，[`claude_hooks.rs`](../scripts/router-rs/src/claude_hooks.rs) 中 **`claude_reviewer_lane`** 允许的 lane 为深度审稿：**`general-purpose` / `best-of-n-runner`**（及 `normalize_subagent_type` 等价名）**以及** **`review` / `reviewer` / `critic` / `code-review`**；**`explore` / `explorer` 不计入**（与 Cursor「默认可清点深度 lane」对齐，避免轻量 explore 子代理形式过关）。**不要将「Claude 的 `reviewer` 字面」套用到 Cursor/Codex**：后两宿主 **不认**仅用该字符串清门。

**Cursor：`REVIEW_GATE` 子代理 cycle 状态机**：[`ReviewGateState`](../scripts/router-rs/src/cursor_hooks/frag_02_gate_event.rs) 使用 `review_subagent_pending_cycle_keys`（Vec，**multiset**）：武装 review 后，每次 **qualifying start**（`PostToolUse` / `subagentStart`，lane 为可数深度 lane 且 **`fork_context` 解析为 `false`**——布尔、可走布尔字符串表或 JSON **整数** `0`/`1`，语义同 [`fork_context_from_values`](../scripts/router-rs/src/review_gate_engine.rs)）**压入**一条 **cycle key**（优先稳定 `subagent_id` / `agent_id` / `task_id` 等，见 [`review_subagent_cycle_key`](../scripts/router-rs/src/cursor_hooks/frag_01_continuity_intent.rs)；缺失时退化为 `lane:<lane>`——并行同 lane 且无 id 时依赖两条重复 key，各自需一次 qualifying **stop** 核销）。qualifying **stop** 的 key 若命中 pending 中任一条，则 **移除恰好一条** 匹配记录；**仅当** pending **完全排空**时，相位才满足清门条件（`phase≥3` 表示本轮 multiset 已全部收尾）。**核销**依赖宿主发出带一致 `cycle_key` 的 **`subagentStop`**（或等价可解析字段）；对 **`id:`** 前缀 key，若 **`subagentStart` 已入队**，随后同一 id 的 **`PostToolUse` 不再二次入队**，以免双 pending 需两次 stop。**磁盘字段 `subagent_start_count` 仅随 `subagentStart` 上 qualifying review 递增；`PostToolUse`  multiset 入队不增加该计数。** 磁盘上旧版仅有 `review_subagent_cycle_open` + 单 `review_subagent_cycle_key` 时，加载会 hydrate 进同一向量。**pre-goal**（`/autopilot`）在常态门控启用下与可数深度 lane 阈值一致；`ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 应急开启时，pre-goal 仍接受「任一带名 lane/agent 字段」宽松判定（与应急路径下 `REVIEW_GATE` 的宽松 lane 列表不同，勿混读）。Stop 可见短码里 `need=` 段与代码常量 `REVIEW_GATE_FOLLOWUP_NEED_SEGMENT`（[`frag_04_review_gate_runtime.rs`](../scripts/router-rs/src/cursor_hooks/frag_04_review_gate_runtime.rs)）对齐。`ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 开启时，lane 判定走宽松实现（与常态语义不同），仅作应急。

**排障**：Stop 仍被 `*_REVIEW_GATE` 拦住时，先在 hook stdin JSON 核对 **`tool_input`/`subagent_type`/`agent_type`/`agentType`/`type`** 规范化结果，以及 **`fork_context`/`forkContext`** 是否解析为逻辑 **`false`**（典型为布尔 `false`，亦可为可走布尔字符串表的 **`"false"` / `"0"`** 等；JSON **整数** **`0`** 与布尔 **`false`** 等价；**整数 `1`** 解析为 **`true`**（非独立 fork）；其它 JSON **Number**（如 `2`、浮点）仍为 **`None`**，与字段缺失同化。**字符串** `"0"` 与 **数值** `0` 在此等价为 false。仍推荐宿主使用 **JSON 布尔** 显式表达 `fork_context`。）**缺省（字段缺失）不等价于 `false`**：独立审稿 / pre-goal 子代理统计不到「独立 fork」证据时，应补全载荷而非假定宿主默认独立。Codex：`PostToolUse` 事件根部与 `tool_input` 内均可携带 `fork_context`（与 Cursor 对齐的 `fork_context_from_values` 次级来源）。

**Cursor `Stop`：`review_override` / `delegation_override` / `has_override`（以及门控用的 `delegation_override` 句式）**：仅以**用户本轮 prompt** 为信源；**不**把助手 `response` 拼进 `signal_text` 来匹配这些 override（与 `beforeSubmit` 一致），避免模型在可见回复里复述「不要用子代理」等句式误解除 `REVIEW_GATE`。拒因清门仍遵循 `saw_reject_reason`（整树 + 用户轮粘贴行）既有规则。

**Cursor 应急关闭审稿门控**：**仅当** `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 为 `1`/`true`/`yes`/`on`（大小写不敏感，与 `cursor_review_gate_disabled_by_env()` 对齐）时关闭 Cursor 审稿门控；unset、空串与其它任意非 truthy 值均保持启用。`beforeSubmitPrompt` / `userPromptSubmit`（归一化后的 `beforesubmitprompt` / `userpromptsubmit`）**不**调用 `handle_before_submit`，stdout **仅** `{"continue":true}`，无 `additional_context`/`followup_message`，故无 review 默认 nudge、pre-goal、`paper_adversarial` 合并，且不读写 `.cursor/hook-state`；**对照**常态路径仍走 [`handle_before_submit`](../scripts/router-rs/src/cursor_hooks/frag_05_handlers_core.rs)。`Stop` 仍进入 `handle_stop`，但在获取 `.cursor/hook-state` 锁**之前**即返回：仅合并 closeout 硬拦（若有）并经 `finalize_stop_hook_outputs` 写入 `AUTOPILOT_DRIVE`/`RFV_LOOP` 与软 `SESSION_CLOSE_STYLE`（与常态 Stop 收口共用同名函数，`include!` 编入 [`cursor_hooks`](../scripts/router-rs/src/cursor_hooks/mod.rs)）；**不进行**常态下的 review / goal Stop 分支与 hook-state 门控推演。
- **`afterAgentResponse` / hook-state**：[`dispatch.rs`](../scripts/router-rs/src/cursor_hooks/dispatch.rs) 应急分支对 `afterAgentResponse` **与常态同**派发 [`handle_after_agent_response`](../scripts/router-rs/src/cursor_hooks/frag_05_handlers_core.rs)；PostTool/subagent/start/stop/sessionEnd/preCompact/shell 等均仍走各自 `handle_*`，**可**继续读写 `.cursor/hook-state`。仅上述 **beforeSubmit** 两事件跳过 `handle_before_submit`；「关审稿」**不等于**冻结全局 hook-state。
- **连续性读盘**：应急 `Stop` 仍可读 `artifacts/current` 等连续性视图以驱动 `finalize_stop_hook_outputs`（续跑/软段），与「不跑常态 hook-state review/goal 推演」**不冲突**。

**Claude Code：磁盘状态不可读（与 Codex 同形 fail-closed）**：当 `.claude/review_gate_<hash>.json` 或 `.claude/hook_state_<hash>.json` **已存在**但无法读取或 JSON 非法（含空文件）时，`Stop` 硬阻塞，`stopReason` 含 `router-rs CLAUDE_HOOK_STATE_UNREADABLE need=repair_hook_state_json_or_permissions`；`UserPromptSubmit` 侧若需合并 review gate 状态而遇不可读，会注入 `additionalContext` 提示修复而非覆盖文件。排障：检查权限、修复或删除损坏文件。Codex 侧对照短码见 [`host_adapter_contract.md`](host_adapter_contract.md) 中 **`CODEX_HOOK_STATE_UNREADABLE`** 行。

**深度 review 的透镜目录与 lane 自选、穷尽语义**：见 [`skills/code-review-deep/SKILL.md`](../skills/code-review-deep/SKILL.md)（本文件不展开 checklist）。

**环境变量表（脚注，读表前先看）**：下表及 §4.2 中凡出现 `…_CHARS`、`…_MAX`、`…_MAX_BYTES` 或未写明「字符数」的上下文长度，**一律按 UTF-8 字节**计（实现见 [`router_env_flags.rs`](../scripts/router-rs/src/router_env_flags.rs)、[`frag_04_review_gate_runtime.rs`](../scripts/router-rs/src/cursor_hooks/frag_04_review_gate_runtime.rs)；变量名遗留 `_CHARS` 时不改语义）。出站裁剪超长时，前缀保留后末尾会追加**固定截断标记**（与纯 `...` 相比可区分「预算截断」与门控逻辑未满足）。子代理 **`fork_context`**：**推荐** JSON **布尔**；实现亦接受布尔字符串及 **JSON 整数** `0`/`1`（见 §5.0 [`fork_context_from_values`](../scripts/router-rs/src/review_gate_engine.rs)）；其它 **Number** 与**字段缺失**均不为 `false`。

| 环境变量 | 默认 | 作用 |
|---------|------|------|
| `ROUTER_RS_OPERATOR_INJECT` | 开 | 总闸：关闭 advisory 注入（含 Codex lifecycle `additionalContext`、**Cursor `SessionStart` `additional_context`**、Cursor `SESSION_CLOSE_STYLE` 软段落，及与 `AUTOPILOT_DRIVE`/`RFV_LOOP`/`paper_adversarial` 等同样经本闸聚合的子能力）；不影响硬门控短码 |
| `ROUTER_RS_HARNESS_OPERATOR_NUDGES` | 开 | 仅关闭 operator nudge 文案；不改 gate 逻辑 |
| `ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT` | 开 | **仅** `0`/`false`/`off`/`no`：关闭 RFV advisory 结构化外研 hint（[`router_env_flags.rs`](../scripts/router-rs/src/router_env_flags.rs)；仍受 `ROUTER_RS_OPERATOR_INJECT` 总闸约束） |
| `ROUTER_RS_AUTOPILOT_DRIVE_HOOK` | 开 | 关闭 Stop 等必要事件上的 `AUTOPILOT_DRIVE` advisory |
| `ROUTER_RS_RFV_LOOP_HOOK` | 开 | 关闭必要事件上的 `RFV_LOOP_CONTINUE` advisory |
| `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` | 开 | **仅** `0`/`false`/`off`/`no`：关闭 PostTool 在满足连续性就绪且启发式判定为验证类命令时向 `EVIDENCE_INDEX` 自动追加（[`framework_runtime/mod.rs`](../scripts/router-rs/src/framework_runtime/mod.rs)）；unset 等同启用 |
| `ROUTER_RS_CONTINUITY_STOP_CHECKPOINT` | 开 | **仅** `0`/`false`/`off`/`no`：Codex `Stop` 自动写入进行中连续性 checkpoint（[`codex_hooks.rs`](../scripts/router-rs/src/codex_hooks.rs)）；unset 等同启用 |
| `ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS` | 8192，clamp 1024–65536 | Cursor hook stdout：`additional_context` /（极端长度）`followup_message` 经 [`apply_cursor_hook_output_policy`](../scripts/router-rs/src/cursor_hooks/frag_04_review_gate_runtime.rs) UTF-8 **字节**裁剪（变量名为 `_CHARS`，语义为字节上限）；详见 §4.2 |
| `ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS` | 1200，clamp 256–8192 | Cursor SessionStart `additional_context` 合成字节上限 |
| `ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE` | 开 | **仅** `0`/`false`/`off`/`no`：关闭 Stop 软 `SESSION_CLOSE_STYLE` 单行收口提示 |
| `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK` | 关 | Cursor beforeSubmit 中显式开启论文/手稿强对抗审稿短段 |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED` | 关 | 显式开启 Cursor `/autopilot` pre-goal beforeSubmit 提示 |
| `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` | 关 | **仅** `1`/`true`/`yes`/`on`：beforeSubmit 路径上**禁止**仅凭磁盘 `GOAL_STATE` hydration 将 pre-goal 视同已满足（Stop 收口路径不受影响）；降低 checkout/遗留 `artifacts/current` 带入旧 GOAL 的误放行；pre-goal 仍可由 subagent / `reject_reason` / nag cap 等满足 |
| `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP` | 关 | **仅** `1`/`true`/`yes`/`on` 时：Cursor `SessionEnd` 在清当前 `session_key` 与全局 tmp 孤儿之外，对 `.cursor/hook-state/` 再做**全目录前缀清扫**（历史行为），用于单人单会话下清 session_id/cwd 漂移遗留；**默认关**以免同仓库并行 Cursor 会话的门控状态被其它会话的 SessionEnd 误删 |
| `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` | 关 | **仅**当值为 `1`/`true`/`yes`/`on`（大小写不敏感）时关闭 Cursor 审稿门控并走 `dispatch_cursor_hook_event` 应急分支；unset、空串与其它任意值均保持启用。**应急下 beforeSubmit 仅 `continue:true`；`Stop` 仍在取 hook-state 锁前返回并完成 `finalize_stop_hook_outputs`**；`afterAgentResponse` 等仍写 hook-state（详见 §5.0「Cursor 应急关闭审稿门控」段末 bullets）。机器判定：[frag_04_review_gate_runtime.rs](../scripts/router-rs/src/cursor_hooks/frag_04_review_gate_runtime.rs) → `cursor_review_gate_disabled_by_env` |
| `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES` | 内置数值默认 **8**（单测 unset 视为严格不降频） | `REVIEW_GATE` 仍未满足时，**连续多少轮 `Stop`** 仍将完整 `need=`/`hint=` 写入 `followup_message`；超过后改为短 `followup_message`（仍含 `router-rs REVIEW_GATE` 前缀）并将完整行并入 `additional_context`，且该轮 **跳过** `AUTOPILOT_DRIVE`/`RFV` 的 Stop 合并以免双叠。`0`/`false`/`off`/`no`：**关闭**降频（每轮 Stop 始终完整硬行）。机器判定：[`router_env_flags.rs`](../scripts/router-rs/src/router_env_flags.rs) → `router_rs_cursor_review_gate_stop_max_nudges_cap`；实现：[`frag_05_handlers_core.rs`](../scripts/router-rs/src/cursor_hooks/frag_05_handlers_core.rs) `handle_stop` + [`frag_01_continuity_intent.rs`](../scripts/router-rs/src/cursor_hooks/frag_01_continuity_intent.rs) `finalize_stop_hook_outputs` |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES` | 内置数值默认 | `/autopilot` pre-goal beforeSubmit 提示次数上限（[`frag_04_review_gate_runtime.rs`](../scripts/router-rs/src/cursor_hooks/frag_04_review_gate_runtime.rs)） |
| `ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS` | 内置数值默认 | 仍可打开的并发 subagent 上限，`0` 关闭限制 |
| `ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS` | 内置数值默认 | subagent stale 判定阈值（秒） |
| `ROUTER_RS_CURSOR_SESSION_NAMESPACE` | unset | 同仓库并行 Cursor 会话时分流 `.cursor/hook-state` 文件名组件（[`frag_02_gate_event.rs`](../scripts/router-rs/src/cursor_hooks/frag_02_gate_event.rs)） |
| `ROUTER_RS_CURSOR_WORKSPACE_ROOT` | unset | Cursor workspace/repo root 解析兜底（[`repo_root.rs`](../scripts/router-rs/src/cursor_hooks/repo_root.rs)） |
| `ROUTER_RS_CURSOR_TERMINAL_KILL_MODE` | 内置默认 | 终端 kill 策略（[`frag_03_paths_terminal_merge_lock_persist.rs`](../scripts/router-rs/src/cursor_hooks/frag_03_paths_terminal_merge_lock_persist.rs)） |
| `ROUTER_RS_CURSOR_KILL_STALE_TERMINALS` | 内置阈值默认 | 陈旧会话终端清理（[`frag_06_session_terminal_kill.rs`](../scripts/router-rs/src/cursor_hooks/frag_06_session_terminal_kill.rs)） |
| `ROUTER_RS_CLOSEOUT_ENFORCEMENT` | 本地软、CI 硬 | 控制 closeout record 是否程序化硬门禁 |
| `ROUTER_RS_DEPTH_SCORE_MODE` | `legacy` | `strict` 时启用更严格 depth 第三分公式 |
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` | 640，clamp 256–8192 | Codex SessionStart `additionalContext` **字节**上限（遗留变量名；[`codex_additional_context_max_bytes`](../scripts/router-rs/src/codex_hooks.rs)） |
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` | unset（可选覆盖） | 若设置：**优先于** `_MAX`；二者解析均为 UTF-8 **字节**，clamp 256–8192 |
| `ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY` | 关 | **仅** `1`/`true`/`yes`/`on`：Codex `UserPromptSubmit` / `PostToolUse` / `Stop` 在无法从 hook stdin（`session_id`/`sessionId`/`conversation_id`/`conversationId`/`thread_id`/`threadId`）或环境 `CODEX_SESSION_ID`/`CODEX_CONVERSATION_ID` 得到稳定会话键时 **block**（`SessionStart` 不受影响）；默认关闭以保持与旧 Codex payload 兼容 |
| `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE` | 关 | **仅**当值为 `1`/`true`/`yes`/`on`（大小写不敏感）时关闭 Claude Code `CLAUDE_REVIEW_GATE`（含 UserPromptSubmit review 提示）；unset、空串与其它任意值均保持启用（与 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 对称）。可选：在项目根 `.claude/router-rs-hook.env` 写 `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE=1`（由安装的 Claude hook command 包装自动加载；重装/合并 hook 后以 `scripts/router-rs` 的 Claude settings 投影为准） |
| `ROUTER_RS_CLAUDE_SESSION_NAMESPACE` | unset | **仅 Claude** session 状态：当 stdin 缺少会话 id、`cwd` 类字段又不足以分流时，同仓多会话可能共用 `.claude/review_gate_*.json` / `hook_state_*.json`；设非空串可为并行会话隔离状态文件名组件（语义对齐 `ROUTER_RS_CURSOR_SESSION_NAMESPACE`；见 [`claude_hooks.rs`](../scripts/router-rs/src/claude_hooks.rs) `claude_session_key`） |
| `ROUTER_RS_TASK_LEDGER_FLOCK` | 开 | **仅** `0`/`false`/`off`/`no`（与 `ROUTER_RS_OPERATOR_INJECT` 同类 default-true 语义）关闭 `artifacts/current/.router-rs.task-ledger.lock` 的 `flock`；关闭后多进程并行写账本为 best-effort（见 §3.1 证据流下 Task ledger 段） |
| `ROUTER_RS_CLIPBOARD_PATH` | unset（可选） | CLI/read_clipboard：自定义剪贴板文件路径（[`runtime_ops.inc`](../scripts/router-rs/src/cli/runtime_ops.inc)） |
| `ROUTER_RS_STORAGE_ROOT` | unset（可选） | `runtime_storage` 持久根重写 |
| `ROUTER_RS_BIN` | unset（可选） | host_integration：`router-rs` 可执行路径提示 |
| `ROUTER_RS_GENERATOR_TIMEOUT_SECONDS` | unset（可选） | host_integration：生成步骤超时秒 |
| `ROUTER_RS_SHARED_TARGET` | unset（可选） | `router_self` 共享 target 路径 |
| `ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS` / `ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS` | unset | framework `/update` maint 流专用（[`framework_maint.rs`](../scripts/router-rs/src/framework_maint.rs)） |
已退役的文案分叉、beforeSubmit 双续跑、聊天区投影切换、静默例外模式、Plan→Build goal 门控开关都不再支持；相关变量已从活跃代码与主真源文档移除。

## 6. Closeout 与深度

- closeout 真相来自证据、diff、产物和明确 blocker，而不是“我完成了”的叙述。
- `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 未设置且非 CI 时，允许本地软门禁；CI 或显式开启时走硬门禁。
- `DepthCompliance`、`GOAL_STATE`、`RFV_LOOP_STATE` 的更细语义由 `router-rs` 和对应 schema 负责；本文件只定义它们属于 L2/L3 正式控制面，而不是聊天补丁。

### 深度调研：三轨对齐（无自动合并）

宿主里「说要深度调研」并不等于自动落盘 RFV 外研账本；三件事分工如下（仅指针，不重述全文）：

- **Execute 内核**：`research_mode`/live prompt 塑形（[`runtime_ops.inc`](../scripts/router-rs/src/cli/runtime_ops.inc) 的 `infer_research_mode` / `build_live_execute_prompt`）— 只管当次执行的回复结构提示，不起账本。
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
| L3 control plane | `scripts/router-rs/src/`：`codex_hooks.rs`、`claude_hooks.rs`、`cursor_hooks/mod.rs`（同目录 `frag_*.rs` / `dispatch.rs` 等经 `include!` 编入同一 Rust 模块）、`autopilot_goal.rs`、`rfv_loop.rs`、`framework_runtime/mod.rs`、`task_state.rs`、`host_integration.rs` |
| L2 continuity | `artifacts/current/`、`TRACE_EVENTS.jsonl`、`STEP_LEDGER.jsonl`、`configs/framework/*SCHEMA*` |
| Skill 热路由（router-rs hot path） | `skills/SKILL_ROUTING_RUNTIME.json` |
| Skill 冷元数据（**非** router-rs hot path；由 skill-compiler / policy contract / CI 消费） | `skills/SKILL_PLUGIN_CATALOG.json`、`skills/SKILL_ROUTING_METADATA.json`、`skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json` |
| Host registry | `configs/framework/RUNTIME_REGISTRY.json` |
| 弱模型 / 上下文预算调研索引 | `docs/plans/RESEARCH_harness_weak_model_top_tier.md`、`docs/plans/context_token_audit_deep_dive.md` |
| 全面自检清单（减法审计，非合并门槛） | `docs/plans/harness_subtraction_first_principles_audit_checklist.md` |

## 9. 刻意不做的事

- 不在 SessionStart 注入 repo onboarding。
- 不保留旧 runtime shape 兼容层。
- 不在 `AGENTS.md`、Cursor rules、docs、hook 文案里重复展开同一套长叙事。
- 不为了“也许以后需要”保留 verbose 模式、双通道切换或多事件重复续跑注入。
