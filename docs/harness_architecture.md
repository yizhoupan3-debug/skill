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
- 允许内容：`GOAL_STATE` / `RFV_LOOP_STATE` 单行头、active task 连续性摘要、必要时的一行 repo 指针；优先级也按此顺序截断。
- 禁止内容：repo onboarding、Quick Reference、Build & test、Key paths、Tool cost hierarchy 之类静态说明。
- Cursor `SessionStart` 采用固定紧凑模板和固定预算；超预算时统一截断，不再提供 verbose 模式或额外预算开关。

### 2.2 Skill routing

- `skills/SKILL_ROUTING_RUNTIME.json` 是唯一热路由真源。
- 热 runtime 只保留：`version`、`schema_version`、`scope`、`keys`、`skills`。
- 任何 plugin、projection、routing explain、兼容迁移叙事都不进热 runtime。
- 冷真源分工：
  - `skills/SKILL_PLUGIN_CATALOG.json`
  - `skills/SKILL_ROUTING_METADATA.json`
  - `skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`

## 3. 主数据流

### 3.1 证据流

`L1` 验证命令或验证形工具输出
→ `router-rs` 采样/追加
→ `artifacts/current/<task_id>/EVIDENCE_INDEX.json`
→ closeout / digest / gate 消费。

原则：

- hook 只记录证据，不替模型“编造验证通过”。
- 长尾命令通过显式 append 或更窄启发式补齐。

**Task ledger 写入（跨进程）**：`GOAL_STATE` / `RFV_LOOP_STATE` / session artifact 批量写 / `EVIDENCE_INDEX` 的 RMW，默认经 [`task_write_lock.rs`](../scripts/router-rs/src/task_write_lock.rs) 在 `artifacts/current/.router-rs.task-ledger.lock` 上持 **`flock(2)`**，按**仓库根**互斥（多宿主 hook 子进程不共享 Rust 进程内互斥量）。`EVIDENCE_INDEX` 仍可再持单文件旁路锁（`runtime_storage::acquire_runtime_path_lock`）；锁序约定为 **先 repo ledger flock，再 path lock**。`ROUTER_RS_TASK_LEDGER_FLOCK=0|false|off|no` 可关闭 flock（如不稳定的网络 FS），关闭后并行写为 best-effort。`TASK_STATE.json` 仅为投影，权威仍以分文件为准；单独跑 `task-state-aggregate-sync` 的修复路径不替代上述写锁。

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

不再保留“聊天区 vs additional_context”切换开关，也不再在多个事件上重复投影同一段 Goal/RFV 续跑文案。

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

## 5. 开关面

只保留真正改变行为边界的少量开关；文案分叉和投影位置分叉不再保留。

### 5.0 Review gate：深度审稿可清点 lane（按宿主）

**Cursor / Codex（`PostToolUse` / `Task` / `functions.subagent` 等记入 subagent 的载荷）**：仅当 lane 归为 **`general-purpose`** 或 **`best-of-n-runner`**（及 `normalize_subagent_type` 等价名，如 `generalpurpose` / `bestofnrunner`）且 **`fork_context`/`forkContext` 显式为 JSON 布尔 `false`** 时，可推进「独立审稿」相位并清点 `REVIEW_GATE`（Cursor）或 Codex independent reviewer 证据。**机器判定真源**：[`hook_common.rs`](../scripts/router-rs/src/hook_common.rs) → `is_deep_review_gate_lane_normalized`（对已规范化 lane 字符串求值）；本段不维护第二份可清点 lane 枚举表。

**Claude Code**：在满足显式 `fork_context=false` 的前提下，[`claude_hooks.rs`](../scripts/router-rs/src/claude_hooks.rs) 中 **`claude_reviewer_lane`** 允许的 lane 为深度审稿：**`general-purpose` / `best-of-n-runner`**（及 `normalize_subagent_type` 等价名）**以及** **`review` / `reviewer` / `critic` / `code-review`**；**`explore` / `explorer` 不计入**（与 Cursor「默认可清点深度 lane」对齐，避免轻量 explore 子代理形式过关）。**不要将「Claude 的 `reviewer` 字面」套用到 Cursor/Codex**：后两宿主 **不认**仅用该字符串清门。

**排障**：Stop 仍被 `*_REVIEW_GATE` 拦住时，先在 hook stdin JSON 核对 **`tool_input`/`subagent_type`/`agent_type`/`agentType`/`type`** 规范化结果，以及 **`fork_context`/`forkContext`** 是否为布尔 **`false`**（字符串等非 JSON 布尔可不计入）。**缺省（字段缺失）不等价于 `false`**：独立审稿 / pre-goal 子代理统计不到「独立 fork」证据时，应补全载荷而非假定宿主默认独立。

**Claude Code：磁盘状态不可读（与 Codex 同形 fail-closed）**：当 `.claude/review_gate_<hash>.json` 或 `.claude/hook_state_<hash>.json` **已存在**但无法读取或 JSON 非法（含空文件）时，`Stop` 硬阻塞，`stopReason` 含 `router-rs CLAUDE_HOOK_STATE_UNREADABLE need=repair_hook_state_json_or_permissions`；`UserPromptSubmit` 侧若需合并 review gate 状态而遇不可读，会注入 `additionalContext` 提示修复而非覆盖文件。排障：检查权限、修复或删除损坏文件。Codex 侧对照短码见 [`host_adapter_contract.md`](host_adapter_contract.md) 中 **`CODEX_HOOK_STATE_UNREADABLE`** 行。

**深度 review 的透镜目录与 lane 自选、穷尽语义**：见 [`skills/code-review-deep/SKILL.md`](../skills/code-review-deep/SKILL.md)（本文件不展开 checklist）。

| 环境变量 | 默认 | 作用 |
|---------|------|------|
| `ROUTER_RS_OPERATOR_INJECT` | 开 | 总闸：关闭 advisory 注入（含 Codex lifecycle `additionalContext`、`SESSION_CLOSE_STYLE` 等 Cursor 软段落，以及已由本闸 OR 的其他 nudge）；不影响硬门控短码 |
| `ROUTER_RS_HARNESS_OPERATOR_NUDGES` | 开 | 仅关闭 operator nudge 文案；不改 gate 逻辑 |
| `ROUTER_RS_AUTOPILOT_DRIVE_HOOK` | 开 | 关闭 Stop 等必要事件上的 `AUTOPILOT_DRIVE` advisory |
| `ROUTER_RS_RFV_LOOP_HOOK` | 开 | 关闭必要事件上的 `RFV_LOOP_CONTINUE` advisory |
| `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK` | 关 | Cursor beforeSubmit 中显式开启论文/手稿强对抗审稿短段 |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED` | 关 | 显式开启 Cursor `/autopilot` pre-goal beforeSubmit 提示 |
| `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` | 关 | **仅** `1`/`true`/`yes`/`on`：beforeSubmit 路径上**禁止**仅凭磁盘 `GOAL_STATE` hydration 将 pre-goal 视同已满足（Stop 收口路径不受影响）；降低 checkout/遗留 `artifacts/current` 带入旧 GOAL 的误放行；pre-goal 仍可由 subagent / `reject_reason` / nag cap 等满足 |
| `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP` | 关 | **仅** `1`/`true`/`yes`/`on` 时：Cursor `SessionEnd` 在清当前 `session_key` 与全局 tmp 孤儿之外，对 `.cursor/hook-state/` 再做**全目录前缀清扫**（历史行为），用于单人单会话下清 session_id/cwd 漂移遗留；**默认关**以免同仓库并行 Cursor 会话的门控状态被其它会话的 SessionEnd 误删 |
| `ROUTER_RS_CLOSEOUT_ENFORCEMENT` | 本地软、CI 硬 | 控制 closeout record 是否程序化硬门禁 |
| `ROUTER_RS_DEPTH_SCORE_MODE` | `legacy` | `strict` 时启用更严格 depth 第三分公式 |
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` | 640，clamp 256–8192 | 仅 Codex SessionStart 的字符预算 |
| `ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY` | 关 | **仅** `1`/`true`/`yes`/`on`：Codex `UserPromptSubmit` / `PostToolUse` / `Stop` 在无法从 hook stdin（`session_id`/`sessionId`/`conversation_id`/`conversationId`/`thread_id`/`threadId`）或环境 `CODEX_SESSION_ID`/`CODEX_CONVERSATION_ID` 得到稳定会话键时 **block**（`SessionStart` 不受影响）；默认关闭以保持与旧 Codex payload 兼容 |
| `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE` | 关 | **仅**当值为 `1`/`true`/`yes`/`on`（大小写不敏感）时关闭 Claude Code `CLAUDE_REVIEW_GATE`（含 UserPromptSubmit review 提示）；unset、空串与其它任意值均保持启用（与 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 对称）。可选：在项目根 `.claude/router-rs-hook.env` 写 `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE=1`（由安装的 Claude hook command 包装自动加载；重装/合并 hook 后以 `scripts/router-rs` 的 Claude settings 投影为准） |
| `ROUTER_RS_CLAUDE_SESSION_NAMESPACE` | unset | **仅 Claude** session 状态：当 stdin 缺少会话 id、`cwd` 类字段又不足以分流时，同仓多会话可能共用 `.claude/review_gate_*.json` / `hook_state_*.json`；设非空串可为并行会话隔离状态文件名组件（语义对齐 `ROUTER_RS_CURSOR_SESSION_NAMESPACE`；见 [`claude_hooks.rs`](../scripts/router-rs/src/claude_hooks.rs) `claude_session_key`） |
| `ROUTER_RS_TASK_LEDGER_FLOCK` | 开 | **仅** `0`/`false`/`off`/`no`（与 `ROUTER_RS_OPERATOR_INJECT` 同类 default-true 语义）关闭 `artifacts/current/.router-rs.task-ledger.lock` 的 `flock`；关闭后多进程并行写账本为 best-effort（见 §3.1 证据流下 Task ledger 段） |
已退役的文案分叉、beforeSubmit 双续跑、聊天区投影切换、静默例外模式、Plan→Build goal 门控开关都不再支持；相关变量已从活跃代码与主真源文档移除。

## 6. Closeout 与深度

- closeout 真相来自证据、diff、产物和明确 blocker，而不是“我完成了”的叙述。
- `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 未设置且非 CI 时，允许本地软门禁；CI 或显式开启时走硬门禁。
- `DepthCompliance`、`GOAL_STATE`、`RFV_LOOP_STATE` 的更细语义由 `router-rs` 和对应 schema 负责；本文件只定义它们属于 L2/L3 正式控制面，而不是聊天补丁。

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
| L3 control plane | `scripts/router-rs/src/{cursor_hooks,codex_hooks,claude_hooks,autopilot_goal,rfv_loop,framework_runtime,task_state,host_integration}.rs` |
| L2 continuity | `artifacts/current/`、`TRACE_EVENTS.jsonl`、`STEP_LEDGER.jsonl`、`configs/framework/*SCHEMA*` |
| Skill 热路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| Skill 冷元数据 | `skills/SKILL_PLUGIN_CATALOG.json`、`skills/SKILL_ROUTING_METADATA.json`、`skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json` |
| Host registry | `configs/framework/RUNTIME_REGISTRY.json` |
| 全面自检清单（减法审计，非合并门槛） | `docs/plans/harness_subtraction_first_principles_audit_checklist.md` |

## 9. 刻意不做的事

- 不在 SessionStart 注入 repo onboarding。
- 不保留旧 runtime shape 兼容层。
- 不在 `AGENTS.md`、Cursor rules、docs、hook 文案里重复展开同一套长叙事。
- 不为了“也许以后需要”保留 verbose 模式、双通道切换或多事件重复续跑注入。
