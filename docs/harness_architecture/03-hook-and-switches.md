---
last_verified: "2026-06-02"
depends_on:
  - ../framework_operator_primer.md
  - ../framework_naming_conventions.md
  - ../hosts/cursor.md
  - ../hosts/claude.md
  - ../hosts/codex-cli.md
---

# Hook 文案策略与环境变量

[返回索引](index.md)

## 4. Hook 文案策略

- 对模型可见的 hook 文案默认短码优先、短句优先。
- `REVIEW_GATE`、`AG_FOLLOWUP`、`CLOSEOUT_FOLLOWUP`、`SESSION_CLOSE_STYLE` 等保留单段紧凑输出；**无** `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` hook 注入。
- lock failure、degraded mode、pre-goal 等提示应压缩为单行或极短段，最多附一个动作提示。
- 禁止把长策略解释混进 runtime 提示；长解释只留在本文件和相关契约文档。

### 4.1 Claude `claude hook` 与 Cursor stdin 误接

`router-rs claude hook` 若误收到 Cursor hook 的 stdin，仅在 JSON **顶层**同时满足：**非空字符串** `cursor_version`、**数组** `workspace_roots`、以及 **非空字符串** `hook_event_name` 或 `hookEventName` 之一时整段静默（`suppressOutput`），避免把 Claude 管道接到 Cursor 事件流。  
**不**再对嵌套字段里的 `/.cursor/` 等路径做子串匹配：否则合法 Claude 载荷（例如编辑 `.cursor/` 下文件）会被误判为 Cursor 而旁路门禁。实现见 [`core/router-rs/src/hosts/claude_hooks.rs`](../../core/router-rs/src/hosts/claude_hooks.rs)（`payload_looks_like_cursor_hook_stdin`）。

**stdin 体量**：`router-rs claude hook` 从 stdin 读取的原始输入 **上限 4 MiB**（与 Codex hook 限量读取一致），溢出返回错误；合法 JSON 解析错误返回 `stdin_json_invalid:` 前缀消息。

`router-rs framework host-integration install --to claude` 写入的 hook **command** 须将 stdin **原样交给** `router-rs claude hook`；不在 Bash 层用 `grep` 对 `cursor_version` / `workspace_roots` / `/.cursor/` 做预短路（历史上曾与 Rust 真源分裂）。安装串见 [`host_integration.rs`](../../core/router-rs/src/host_integration.rs) 的 `build_router_rs_claude_hook_command`。

### 4.2 Cursor `additional_context`：合并链路与出站字节上限

- **合并**：各 hook handler 通过 [`merge_additional_context`](../../core/router-rs/src/hosts/cursor_hooks/handlers.rs) 将 advisory 段落追加进出站 JSON 的 `additional_context` 字符串（多事件可达多次追加）。
- **出站裁剪**：Cursor CLI 入口 [`review_gate.rs`](../../core/router-rs/src/review_gate.rs) 在写出 stdout 前调用 [`apply_cursor_hook_output_policy`](../../core/router-rs/src/hosts/cursor_hooks/handlers.rs)：对 `additional_context` 与超长 `followup_message` 使用 **`truncate_cursor_hook_outbound_context_preserving_gate`** / **`truncate_cursor_hook_followup_preserving_review_gate`** — UTF-8 字节上限取自 **`ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS`**（[`router_env_flags.rs`](../../core/router-rs/src/router_env_flags.rs) ，默认 8192，clamp 1024–65536）；超长时 **优先保留** 含 `router-rs REVIEW_GATE`、`REVIEW_GATE detail` 前缀的行，对其余 filler 做前缀截断并以固定 **`...[~trunc]`** 结束（见 §5 环境变量表脚注）。仍建议将硬门控信息放在 `followup_message` 的 `router-rs …` 行。
- **对照**：Codex `additionalContext` 另有字节上限（[`codex_hooks/mod.rs`](../../core/router-rs/src/hosts/codex_hooks/mod.rs) `truncate_codex_additional_context_bytes`）；两套宿主互不替代。

### 4.3 仿宿主续跑行（`RG_FOLLOWUP` 等）与机读真源

- Cursor hook 出站 JSON 中，**深度审稿未完成**与 **Autopilot goal 缺块** 所依赖的机读 leader 真源为 **`router-rs REVIEW_GATE incomplete …`**、**`router-rs AG_FOLLOWUP missing_parts=…`**（均须以 ASCII 前缀 **`router-rs `** 起行；实现见 [`cursor_hooks/handlers.rs`](../../core/router-rs/src/hosts/cursor_hooks/handlers.rs)）。审稿链未收尾时以 **`router-rs REVIEW_GATE incomplete`** 行内 `need=`、`hint=` 排障。
- **其它**由本仓库注入的软提示（如 **`CLOSEOUT_FOLLOWUP`、`SESSION_CLOSE_STYLE`** 等）仍按该列表及各自字段形态识别；历史 `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` 行若出现在旧会话 scrape 中，**不是**当前 harness 注入。
- **`RG_FOLLOWUP`、`RG FOLLOWUP`、`RG-FOLLOWUP`**，以及**无** `router-rs ` 前缀、却仿照 `*_FOLLOWUP` 与 `missing_parts=` / `escalation=` 组合的整行，**不是**本 harness 的注入格式；常见来源为助手复述或误粘贴。真源里 **`router-rs AG_FOLLOWUP` 的 `missing_parts=`** 由 [`ship_readiness.rs`](../../core/router-rs/src/ship_readiness.rs) 拼接，并附 **`primary_fix=`**；盘上已有 `GOAL_STATE` 时 Stop **不**再读聊天 `Goal:` 标题。不会出现 `independent_subagent_or_reject_reason` 这类占位串。
- **出站剥线**：[`review_gate.rs`](../../core/router-rs/src/review_gate.rs) 写出 stdout 前对 `followup_message` / `additional_context` 调用 [`scrub_followup_fields_in_hook_output`](../../core/router-rs/src/autopilot_goal.rs)；[`merge_additional_context`](../../core/router-rs/src/hosts/cursor_hooks/handlers.rs) 在合并追加时亦对片段与整段复用 `scrub_spoof_host_followup_lines`。助手**聊天可见正文**不经该剥线，故仍可能看到仿造行——判读时 **优先** 核对 **`router-rs …` 审稿/goal 行** 与 **`.cursor/hook-state` / 磁盘门控**；**不排除** 同字段内 §4 所列其它真源短码段落。
- **Codex**：`additionalContext` 的截断与注入形态以 [`codex_hooks/mod.rs`](../../core/router-rs/src/hosts/codex_hooks/mod.rs) 及上文 **§4.2「对照」** 为准，与 Cursor 出站 **不互为替身**。
- **清门**：不得以整段会话 scrape 误认拒因；[`saw_reject_reason`](../../core/router-rs/src/hook_common.rs) 仅承认：`signal_text` 中的拒因 token、单独成行的 `rg_clear` / `/rg_clear`，以及**用户本轮**粘贴的 **goal** `ag_followup…` 前缀行。**用户粘贴 `RG_FOLLOWUP…` 不作为合法清门**（与上条「仿冒」一致）；曾依赖旧行为时请改用 `rg_clear` 或拒因 token。

## 5. 开关面

只保留真正改变行为边界的少量开关；文案分叉和投影位置分叉不再保留。

### 5.0 Review gate（操作叙述真源）

**唯一操作手册**：[`framework_operator_primer.md`](../framework_operator_primer.md)（§机读短码、Spawn-first、深度审稿 `REVIEW_GATE`、混用武装顺序、fork_context 排障）。本文件只保留实现锚点：

- **Lane 闭集**：`configs/framework/RUNTIME_REGISTRY.json` → `review_gate.deep_gate_lanes`（Cursor/Codex）；Claude 另见 `claude_reviewer_lanes`。跨宿主差异表：[`host_adapter_contract.md`](../host_adapter_contract.md) §0.1。
- **机器判定**：[`hook_common.rs`](../../core/router-rs/src/hook_common.rs) `is_deep_review_gate_lane_normalized`；`fork_context` 解析：[`review_gate_engine.rs`](../../core/router-rs/src/review_gate_engine.rs) `fork_context_from_values`。
- **状态机实现**：Cursor multiset — [`cursor_hooks/handlers.rs`](../../core/router-rs/src/hosts/cursor_hooks/handlers.rs) `ReviewGateState`；Codex phase — [`codex_hooks/mod.rs`](../../core/router-rs/src/hosts/codex_hooks/mod.rs) `CodexLifecycleContextState`；Claude — [`claude_hooks.rs`](../../core/router-rs/src/hosts/claude_hooks.rs)。
- **透镜与产出形状**：[`skills/code-review-deep/SKILL.md`](../../skills/code-review-deep/SKILL.md)（本文件不展开 checklist）。
- **应急关闭**：`ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` / `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE` / `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE`（见下表）。

**环境变量表（脚注，读表前先看）**：下表及 §4.2 中凡出现 `…_CHARS`、`…_MAX`、`…_MAX_BYTES` 或未写明「字符数」的上下文长度，**一律按 UTF-8 字节**计（实现见 [`router_env_flags.rs`](../../core/router-rs/src/router_env_flags.rs)、[`cursor_hooks/handlers.rs`](../../core/router-rs/src/hosts/cursor_hooks/handlers.rs)；变量名遗留 `_CHARS` 时不改语义）。出站裁剪超长时，前缀保留后末尾会追加**固定截断标记**（与纯 `...` 相比可区分「预算截断」与门控逻辑未满足）。子代理 **`fork_context`**：**推荐** JSON **布尔**；实现亦接受布尔字符串及 **JSON 整数** `0`/`1`（见 [`fork_context_from_values`](../../core/router-rs/src/review_gate_engine.rs)）；其它 **Number** 与**字段缺失**均不为 `false`。

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
| `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` | 开 | **默认开启**（unset 即 strict，[`router_rs_cursor_pre_goal_strict_disk_enabled`](../../core/router-rs/src/router_env_flags.rs) 为 default-true）：**禁止**仅凭磁盘 `GOAL_STATE` hydration 将 `pre_goal_review_satisfied` 置真（beforeSubmit 与 Stop 均适用）；**仅** `0`/`false`/`off`/`no` 恢复历史宽松语义；pre-goal 仍可由 subagent / `reject_reason` / nag cap 等满足 |
| `ROUTER_RS_CURSOR_REVIEW_GATE_MODE` | strict（unset） | **仅** `lite`：启用 `review_lite_pending_cycle_keys`（`id:` only）；非 `id:` 回退 strict。`strict` 显式值与 unset 等价 |
| `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` | 开 | **仅** `0`/`false`/`off`/`no`：关闭 Cursor 可数深度 lane 在 `fork_context` **缺失**时的 `false` 推断；显式 `fork_context: true` 永不算独立证据 |
| `ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX` | 32，clamp 1–256 | `review_subagent_pending_cycle_keys` multiset 上限；满则拒绝新 key 且 subagentStart 不增加 open 计数 |
| `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP` | 关 | **仅** `1`/`true`/`yes`/`on` 时：Cursor `SessionEnd` 在清当前 `session_key` 与全局 tmp 孤儿之外，对 `.cursor/hook-state/` 再做**全目录前缀清扫**（历史行为），用于单人单会话下清 session_id/cwd 漂移遗留；**默认关**以免同仓库并行 Cursor 会话的门控状态被其它会话的 SessionEnd 误删 |
| `ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS` | 关 | **仅** `1`/`true`/`yes`/`on`：5 个减法事件在未写入 `hooks.json` 时仍走完整 handler（单测/对照）；默认 no-op |
| `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` | 关 | 应急：beforeSubmit 仅 `continue:true`；Stop 仅 closeout + `SESSION_CLOSE_STYLE`（无 goal/RFV 续跑） |
| `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES` | 默认 **8** | `REVIEW_GATE` 未满足时 Stop 降频；超 cap 后短 `followup_message` + `additional_context` 细节段 |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES` | 内置数值默认 | `/implementx` pre-goal beforeSubmit 提示次数上限（[`cursor_hooks/handlers.rs`](../../core/router-rs/src/hosts/cursor_hooks/handlers.rs)） |
| `ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS` | 内置数值默认 | 仍可打开的并发 subagent 上限，`0` 关闭限制 |
| `ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS` | 内置数值默认（2h） | subagent stale 判定阈值（秒）；**仅** `0`/`false`/`off`/`no`：**关闭**自动 stale 回收（不重置 `active_subagent_count`、不 prune pending）；清门仍用 `rg_clear` / SessionEnd / `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` |
| `ROUTER_RS_CURSOR_SESSION_NAMESPACE` | unset | 同仓库并行 Cursor 会话时分流 `.cursor/hook-state` 文件名组件（[`cursor_hooks/handlers.rs`](../../core/router-rs/src/hosts/cursor_hooks/handlers.rs)） |
| `ROUTER_RS_CURSOR_WORKSPACE_ROOT` | unset | Cursor workspace/repo root 解析兜底（[`repo_root.rs`](../../core/router-rs/src/hosts/cursor_hooks/repo_root.rs)） |
| `ROUTER_RS_CURSOR_TERMINAL_KILL_MODE` | 内置默认 | 终端 kill 策略（[`cursor_hooks/handlers.rs`](../../core/router-rs/src/hosts/cursor_hooks/handlers.rs)） |
| `ROUTER_RS_CURSOR_KILL_STALE_TERMINALS` | 内置阈值默认 | 陈旧会话终端清理（[`cursor_hooks/handlers.rs`](../../core/router-rs/src/hosts/cursor_hooks/handlers.rs)） |
| `ROUTER_RS_CLOSEOUT_ENFORCEMENT` | 本地软、CI 硬 | 控制 closeout record 是否程序化硬门禁 |
| `ROUTER_RS_DEPTH_SCORE_MODE` | `legacy` | `strict` 时启用更严格 depth 第三分公式 |
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` | 640，clamp 256–8192 | Codex SessionStart `additionalContext` **字节**上限（遗留变量名；[`codex_additional_context_max_bytes`](../../core/router-rs/src/hosts/codex_hooks/mod.rs)） |
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` | unset（可选覆盖） | 若设置：**优先于** `_MAX`；二者解析均为 UTF-8 **字节**，clamp 256–8192 |
| `ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY` | **开**（unset=on） | Codex `UserPromptSubmit` / `PostToolUse` / `Stop` 在无法从 hook stdin（`session_id`/`sessionId`/`conversation_id`/`conversationId`/`thread_id`/`threadId`）或环境 `CODEX_SESSION_ID`/`CODEX_CONVERSATION_ID` 得到稳定会话键时 **block**（`SessionStart` 不受影响）；legacy `=0`/`false`/`off`/`no` 关闭硬前置并使用 **repo+cwd+payload session** 确定性 fallback（可选 `ROUTER_RS_CODEX_HOOK_STATE_SALT`；`cwd` 空 stderr 警告） |
| `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE` | 关 | **仅** `1`/`true`/`yes`/`on`：关闭 Codex `CODEX_REVIEW_GATE` 硬门控（unset 保持启用；对称 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` / `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE`） |
| `ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS` | 关 | **仅** `1`：Codex `Stop` 在 `stop_hook_active` 重放时跳过 review/closeout 门控 |
| `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE` | 关 | **仅**当值为 `1`/`true`/`yes`/`on`（大小写不敏感）时关闭 Claude Code `CLAUDE_REVIEW_GATE`（含 UserPromptSubmit review 提示）；unset、空串与其它任意值均保持启用（与 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 对称）。可选：在项目根 `.claude/router-rs-hook.env` 写 `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE=1`（由安装的 Claude hook command 包装自动加载；重装/合并 hook 后以 `core/router-rs` 的 Claude settings 投影为准） |
| `ROUTER_RS_CLAUDE_SESSION_NAMESPACE` | unset | **仅 Claude** session 状态：当 stdin 缺少会话 id、`cwd` 类字段又不足以分流时，同仓多会话可能共用 `.claude/review_gate_*.json` / `hook_state_*.json`；设非空串可为并行会话隔离状态文件名组件（语义对齐 `ROUTER_RS_CURSOR_SESSION_NAMESPACE`；见 [`claude_hooks.rs`](../../core/router-rs/src/hosts/claude_hooks.rs) `claude_session_key`） |
| `ROUTER_RS_TASK_LEDGER_FLOCK` | 开 | **仅** `0`/`false`/`off`/`no`（与 `ROUTER_RS_OPERATOR_INJECT` 同类 default-true 语义）关闭 `artifacts/current/.router-rs.task-ledger.lock` 的 `flock`；关闭后多进程并行写账本为 best-effort（见 §3.1 证据流下 Task ledger 段） |
| `ROUTER_RS_CLIPBOARD_PATH` | unset（可选） | CLI/read_clipboard：自定义剪贴板文件路径（[`runtime_ops.inc`](../../core/router-rs/src/cli/runtime_ops.inc)） |
| `ROUTER_RS_STORAGE_ROOT` | unset（可选） | `runtime_storage` 持久根重写 |
| `ROUTER_RS_BIN` | unset（可选） | host_integration：`router-rs` 可执行路径提示 |
| `ROUTER_RS_GENERATOR_TIMEOUT_SECONDS` | unset → **300s** | `generated-artifacts-status` drift-gate：单条 manifest generator 超时（秒）；`0` 仍用默认 300s |
| `ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS` | 关 | **仅** `1`/`true`/`yes`/`on`：等同 CLI `--skip-generator-run`（metadata-only，不跑 generator、不比 drift） |
| `ROUTER_RS_SHARED_TARGET` | unset（可选） | `router_self` 共享 target 路径 |
| `ROUTER_RS_UPDATE_RUN_AUTORESEARCH_CLI_TESTS` / `ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS` | unset | framework `/update` maint 流专用（[`framework_maint.rs`](../../core/router-rs/src/framework_maint.rs)） |
已退役的文案分叉、beforeSubmit 双续跑、聊天区投影切换、静默例外模式、Plan→Build goal 门控开关都不再支持。**2026-05 拔除、env 名保留但无操作**：`ROUTER_RS_GOAL_CONTINUE_HOOK`、`ROUTER_RS_RFV_LOOP_HOOK`（及兼容 `ROUTER_RS_AUTOPILOT_DRIVE_HOOK`）、`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT`、`ROUTER_RS_DEPTH_COMPLIANCE_HINT`；SessionStart `digest` 模式字符串亦不再产出 digest 正文。
