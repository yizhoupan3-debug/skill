# 框架操作者一页纸（使用者视角）

面向：**在本仓库或接入本框架的工作区里日常干活的人**。长设计与契约仍以 [AGENTS.md](../AGENTS.md)、[harness_architecture.md](harness_architecture.md)、[host_adapter_contract.md](host_adapter_contract.md) 为准；本文只解决「先读哪、宿主差在哪、卡门了怎么办」。

## 推荐阅读顺序（热路径）

1. [AGENTS.md](../AGENTS.md) — 路由、执行梯子、Closeout、跨宿主不变量  
2. [skills/SKILL_ROUTING_RUNTIME.json](../skills/SKILL_ROUTING_RUNTIME.json) — **唯一**热路由入口；命中后只打开记录里的 `skill_path`  
3. 冷元数据（按需）：[skills/SKILL_ROUTING_METADATA.json](../skills/SKILL_ROUTING_METADATA.json)、[skills/SKILL_PLUGIN_CATALOG.json](../skills/SKILL_PLUGIN_CATALOG.json)、[skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json](../skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json) — **不要**塞进模型热路径一次性读完  
4. [harness_architecture.md](harness_architecture.md) — 连续性 L1–L5、hook 出站裁剪、环境变量表  
5. [host_adapter_contract.md](host_adapter_contract.md) — 新宿主接入；**Cursor 排障**见其中 Codex/Cursor 对照表与 `fork_context` 说明  

自检命令（在仓库根，已构建 `router-rs` 时）：

```bash
cargo run --release --manifest-path scripts/router-rs/Cargo.toml -- framework doctor --repo-root "$PWD"
```

## 宿主 × 策略强度（易误解点）

| 宿主 | Stop 上「硬拦」语义 | 说明 |
|------|---------------------|------|
| Codex CLI | 可出现 `decision:block` 等硬语义 | 以 Codex hooks 实际 JSON 为准 |
| Cursor | 多为 `followup_message` / `continue` 类提示 | **不是** Codex 的同形硬拦；不要假设「Stop 一定挡住提交」 |
| Claude Code | 以 `claude_hooks` 出站字段为准 | 与 Cursor 也不完全同形 |

**Non-goal**：在 Cursor 上复刻 Codex 级硬拦属于宿主能力边界；要「真挡住」须依赖 Cursor 产品语义，而非仅改本仓库 hook 文案。

## 机读短码真源与常见误报

- **真源**：本仓库 Cursor hook 写入的机读门控短句（**非** hook `GOAL_CONTINUE` 续跑），**必须以** ASCII 前缀 **`router-rs `** 起行（例如 **`router-rs REVIEW_GATE incomplete …`**、**`router-rs AG_FOLLOWUP missing_parts=…`**）。排障以 hook 出站 JSON 中带该前缀的行为准；长设计见 [harness_architecture.md](harness_architecture.md) §4.3。
- **误报 / 仿冒**：以 **`RG_FOLLOWUP`**、**`RG FOLLOWUP`**、**`RG-FOLLOWUP`** 等开头、且带 `missing_parts=` / `escalation=` 却**没有** `router-rs ` 前缀的整行，**不是** harness 注入。常见来源是助手复述或误粘贴；其中一种长尾形态会在 `escalation=` 后接英文恐吓句（例如声称已循环多次、禁止静默继续）——仍应忽略，改查 **真实** hook 输出与 `.cursor/hook-state`。
- **对照**：真 **`router-rs AG_FOLLOWUP`** 的 `missing_parts=` 只会是 goal 门控片段（如 `goal_contract`、`checkpoint_progress`、`verification_or_blocker`）的逗号拼接，**不会出现** `independent_subagent_or_reject_reason` 这类占位串；若见该串且前缀不是 `router-rs `，按仿冒处理。
- **粘贴清门**：用户消息里单独一行粘贴 **`RG_FOLLOWUP`…** **不会**被 [`saw_reject_reason`](../scripts/router-rs/src/hook_common.rs) 当作清门（避免把模型仿造行当令牌）；请改用单独一行的 **`rg_clear`**、**[`AGENTS.md`](../AGENTS.md) 所列拒因 token**，或自然语言 `review_override` / `delegation_override`。goal 相关的 **`ag_followup…`** 粘贴兼容仍由同函数处理。

## Spawn-first 配对审稿（2026-05-21）

| 主题 | 操作 |
|------|------|
| **目标** | 少 Stop `REVIEW_GATE` nag：首轮工具**前** spawn 可数 reviewer；主线程调研须**另开** reviewer（`explore` 不算） |
| **一行 nudge** | registry `spawn_first_nudge_by_host`（`cursor` / `codex-cli` / `claude-code`，回退 `spawn_first_nudge`）；关 nudge：`ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE=0` |
| **窄范围不拦** | `review ./file`、`small_task`、不用子代理 → 不武装 gate |
| **勿做** | 为提高 spawn 率而收紧清门（`≥2` lane、lane 文件必达） |

## D10 — 子代理模型继承（2026-05-21）

| 主题 | 操作 |
|------|------|
| **目标** | 并行 `Task`/子代理与主会话**同模型**；禁止宿主默认 Sonnet/Claude 导致地区不可用 |
| **规则** | [`.cursor/rules/subagent-model-inherit.mdc`](../.cursor/rules/subagent-model-inherit.mdc)（alwaysApply） |
| **机读 nudge** | registry `subagent_model_inherit_nudge` / `subagent_model_inherit_nudge_by_host.cursor`；`spawn_first_nudge_by_host.cursor` 已合并 model 半句 |
| **关 nudge** | `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE=0` |
| **注入条件** | `beforeSubmit` 当 `goal_drive(/implementx|/verifyx)` **或** `delegation` **或** `review`；`small_task` / narrow review / `user_gate_override` **跳过** |
| **宿主范围** | **仅 Cursor**；Codex/Claude 无 `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE` |
| **Couldn't start** | 查 `agent-transcripts/.../subagents/*.jsonl` 是否 `Model not available`；见 [docs/hosts/cursor.md](hosts/cursor.md) |

## Harness 硬化要点（2026-05-20）

| 主题 | 行为 |
|------|------|
| **Registry `review_gate` lane** | 真源 `configs/framework/RUNTIME_REGISTRY.json`；[`runtime_registry/mod.rs`](../scripts/router-rs/src/runtime_registry/mod.rs) **磁盘读取**（`registry_loader.rs` shim；无 compile-time embed）。改 lane 后**无需** `cargo build`；重启 hook 子进程即可。 |
| **宿主投影 My/review 文案** | `configs/framework/host_projection_narrative.json`；`host-integration install` 渲染 Codex/Cursor/Claude 入口时读取。勿在 `host_integration.rs` 硬编码段落。 |
| **`generated-artifacts-status`** | **`framework doctor`** 与 `--skip-generator-run` / `ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS=1` → **metadata-only**（快）。**`update-one-shot`** 仍跑全量 **drift-gate**（含慢 generator）。 |
| **`ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE=1`** | 关闭审稿门控并**清除** `.cursor/hook-state` 内 review 字段；`postToolUse`/`subagent*` 不再推进 review phase。 |
| **active 无 GOAL、focus 有 GOAL** | SessionStart 有中文提示；运行 `framework task-state-resolve` 或修正 `active_task.json`。 |
| **active 有 GOAL 但不续跑、focus 在 drive** | hydration/checkpoint 已优先 focus；若指针仍分裂，doctor 报 `ACTIVE_NOT_DRIVING`；对齐 active/focus 或清空 completed 任务的 active 占位。 |
| **`ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK`** | **默认开启**（unset 即 strict）：禁止仅凭磁盘 GOAL 置 `pre_goal_review_satisfied`；宽松 legacy 设 `=0|false|off|no`。 |
| **Stop 自动 checkpoint** | **已拔除（2026-05）**：Cursor/Codex hook Stop **不写** checkpoint；显式刷新用 Desktop MCP `session_checkpoint` 或 `framework_session_artifact_write` stdio。 |
| **Cursor hooks 减法闭集** | 默认 **7** 事件；5 个已移除事件 dispatch **no-op**（[`subtraction.rs`](../scripts/router-rs/src/hosts/cursor_hooks/subtraction.rs)）。写回 `hooks.json` 即恢复 handler；`ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS=1` 仅作未注册时对照。 |
| **`router-rs schema-drift`** | `contract` / `baseline` / `check` 验收 hooks 闭集、模板 parity、REQUIREMENTS↔ROADMAP 标题；见 [`skills/verifyx/SKILL.md`](../skills/verifyx/SKILL.md) 与 [`SCHEMA_DRIFT_HEADINGS_CONTRACT.md`](../configs/framework/SCHEMA_DRIFT_HEADINGS_CONTRACT.md)。 |
| **`ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN=1`** | hook-state 持久化失败时 beforeSubmit 仍放行（应急）；默认 fail-closed。 |
| **`ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`** | Claude 专用；默认 **关闭**（不读 Cursor 同名 env）。 |
| **Registry 读盘失败** | `review_gate` lane 判定 fail-closed（不计入深度 lane）；`framework doctor` 打印 `review_gate snapshot` WARN。 |
| **D9 / fork 推断** | Cursor 默认 **开启** missing-`fork_context`→`false` 推断（`ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`，unset=on）。Claude **默认关闭**（`ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`，**不**读 Cursor/Codex 同名 env）。Codex 用 **`codex_review_independent_fork`** + **`ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`**（默认 on；**不**读 Cursor env）。 |
| **Codex stable session** | 默认 **要求** 稳定 session 键（`ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY` unset=on）；生产勿关。legacy `=0`/`false` 时 fallback 按 **repo + cwd + payload session**（可选 `ROUTER_RS_CODEX_HOOK_STATE_SALT` 加盐）；`cwd` 空会 stderr 警告。Stop 上仅 review 措辞、无 UPS 落盘证据时仍 `CODEX_REVIEW_GATE` block。 |
| **Codex `stop_hook_active`** | Codex 内部 Stop 重放默认**仍**执行 review/closeout 门控；**仅** `ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS=1` 时跳过门控。 |
| **Codex UPS re-arm** | 新一轮深度 review（`全面review` 等）会重置 `independent_review_subagent_seen` / `phase` / `subagent_start_count`（与 Claude 对称）；`ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` 或 my-light 的 UPS/PostTool 会 **清空** hook-state。 |
| **Cursor UPS re-arm** | `beforeSubmit` 在 my-light 解除 review、goal drive 抑制 review、或新一轮深度 review（`review_arms_for_gate && gate live`）时调用 `reset_review_cycle_progress(preserve_session_guards)`：**fresh cycle** 保留 `review_pending_cap_refused` 与 `active_subagent_count`；其余字段（phase / pending multiset / subagent start/stop counts / `review_followup_count`）清零；fresh cycle 再注入 spawn-first 一行指针。 |
| **Codex Stop closeout** | 完成宣称 + `ROUTER_RS_CLOSEOUT_ENFORCEMENT`（CI 默认 on）时 Stop `decision:block` + `CLOSEOUT_FOLLOWUP`（`framework_runtime::closeout_stop_followup_for_completion_text`，与 Cursor 同源 token 表）。 |
| **Review soft-nag 超 cap** | 超过 `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES` 后 `followup_message` 降为短行，细节落入 `additional_context`；**无** hook `GOAL_CONTINUE` 合并（2026-05）。 |

## 混用时的实际武装顺序（Cursor Stop）

- **Stop 优先级**（实现 [`handlers.rs`](../scripts/router-rs/src/hosts/cursor_hooks/handlers.rs) `handle_stop`）：若本轮仍武装深度 review 且子代理证据链未收尾，Stop 先给 **`router-rs REVIEW_GATE incomplete …`**；仅当 review 侧已满足后，才会轮到 **`router-rs AG_FOLLOWUP missing_parts=…`**（goal 契约 / 进展 / 验证）。**无** hook `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE`（2026-05）；宏目标续跑用 **`framework_goal_drive` stdio** + `artifacts/current/<task_id>/` 手动画板。`finalize_stop_hook_outputs` 仅可选合并 `SESSION_CLOSE_STYLE` 软提示。
- **主线程深度 review（wave-2）**：**不得**在未 spawn 可数子代理（`general-purpose` / `best-of-n-runner` / `deep-reviewer` + `fork_context=false`）时仅凭 compact findings 清 `REVIEW_GATE`；须先有可数子代理证据（`subagent_start_count` / pending multiset / qualifying stop），再与 **`Stop` tail** 含 substantive `[P0]`–`[P2]` / `Caveat:` 行配合升 phase（**裸** legacy `phase≥2` alone 不足；stale hygiene 作废 orphan start；本仓默认**无** `afterAgentResponse` hook）。
- **同一条用户消息里同时写深度 review 与 My 执行区入口**（`/implementx`、`/verifyx`）：`beforeSubmit` 里 **`review_arms_for_gate = review && !goal_drive_entrypoint`**，因此只要本回合用户文本命中 **goal drive 入口**，**不会**因 review 措辞在本回合**新武装** `review_required`。**My 默认（`my-light`）**：同轮 review + `/implementx|/verifyx` **不**注入拆分提示（静默 disarm，见 [`docs/hosts/cursor.md`](hosts/cursor.md) beforeSubmit 表）。**非 my-light**（例如磁盘 `GOAL_STATE.lifecycle_profile` 非 `my-light` 且本轮无 My 斜杠、但 hook-state 已 `goal_required`）：会注入一行 **`router-rs：本轮提交同时包含…`** 拆分提示，避免误以为 review 门控失效。若你本意是「先深度审稿再开连续执行」，请拆成两轮。**`/autopilot` 已退役**（`is_autopilot_entrypoint_prompt` 恒为 `false`）。
- **Plan**：`plan_profile: research` 与在同一计划里直接改实现互斥；与 My implement / goal drive 串联时应先调研收口再开 execution 计划或 goal，避免「口头 plan + 立刻 implement」与门控真源打架。

## 深度审稿 `REVIEW_GATE`（Cursor / Codex 可数 lane）

清门依赖宿主载荷，常见卡点：

- **`fork_context` / `forkContext`**：须能解析为逻辑 **`false`**（典型为 JSON **布尔** `false`、可走布尔字符串表中的 `"false"` / `"0"` 等，或 JSON **整数** **`0`**）。**整数 `1`** 解析为 **`true`**（非独立 fork）；其它 **Number** 与**字段缺失**均不为 `false`。仍推荐宿主使用 **JSON 布尔**。
- **Lane**：Cursor/Codex 仅 `review_gate.deep_gate_lanes`（`general-purpose`、`best-of-n-runner` 及归一化等价名；**`explore` / `review` / `reviewer` 等不计入**）。Claude Code 用 `claude_reviewer_lanes`。在 Cursor 上误用 `subagent_type: "review"` 不会清 `REVIEW_GATE`。见 `docs/host_adapter_contract.md` §0.1 差异表。  
- **Multiset 与双事件**：`review_subagent_pending_cycle_keys` 由 qualifying **`subagentStart`** / **`PostToolUse`** 入队，由 **`subagentStop`** 逐条核销至空才清门；同一 **`id:`** 若 `subagentStart` 已入队，随后同 id 的 `PostToolUse` **不重复入队**（见 `handlers.rs` 的 `push_review_pending_cycle_key`）。并行仅 `lane:` 且无稳定 id 时仍依赖 multiset 中多条相同 key。  
- **Stop 单行提示**：若见 `router-rs REVIEW_GATE incomplete` 与 `need=deep_reviewer_cycle general-purpose|best-of-n fork_context=false`，按该 `need=` 检查子代理载荷；尾缀 `hint=` 为可读排障补充，不改变 `need=` 语义。若同一门控多轮 `Stop` 仍卡，完整 `need=`/`hint=` 可能在 **`ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES`**（默认 8）之后被降级到 `additional_context`，`followup_message` 仅保留短 `mode=soft_nag` 行（见 [harness_architecture.md](harness_architecture.md) 环境变量表）。

完整排障叙述见 [host_adapter_contract.md](host_adapter_contract.md) 中 Cursor 小节与 [harness_architecture.md](harness_architecture.md) 中 Review gate 相关段落。

## Hook 减法闭集与内存（2026-05-20）

| 宿主 | 默认注册事件数 | 手册 |
|------|----------------|------|
| **Cursor** | **7** | [`docs/hosts/cursor.md`](hosts/cursor.md) |
| **Claude Code** | **4** | [`docs/hosts/claude.md`](hosts/claude.md) |
| **Codex CLI** | 见 `.codex/hooks.json`（另述） | [`docs/hosts/codex-cli.md`](hosts/codex-cli.md) |

- **Cursor 已移除**：`afterAgentResponse`（compact 清门改 **`Stop` tail**）、shell 生命周期、`afterFileEdit`、`preCompact`。
- **`postToolUse`**：仍注册（**`timeout: 20s`**，与门控事件一致；见 [`docs/hosts/cursor.md`](hosts/cursor.md)「PostToolUse timeout」）。`Read`/`Grep` 等在 router-rs **fast-path** 跳过 tracker 与 hook-state 锁；须 **`cargo build --release`** 且 launcher 命中 release（~8MB）。若 Agent 步在工具后卡住，先查 postToolUse 是否超时而非盲目加长 timeout。
- **Claude**：`PostToolUse` 仅 touched settings/framework 时才有上下文；共享 **`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0`**（`.claude/router-rs-hook.env`）。
- **恢复旧 Cursor 事件**：见 [`MIGRATION.md`](../MIGRATION.md)。

## Codex：hook 重复触发

若 **用户级** `~/.codex/hooks.json` 与 **项目** `.codex/hooks.json` 均注册 `router-rs codex hook`，同一事件可能执行多次（证据重复、状态竞态）。`router-rs framework doctor` 会对每份 hooks 文件统计并 WARN；保留一份入口即可。

## Codex：`AGENTS.md` 与二进制快照

若修改跨宿主策略 [`AGENTS.md`](../AGENTS.md) 或 Codex 差异 [`AGENTS_CODEX.md`](../AGENTS_CODEX.md) 且依赖 Codex 侧投影：策略正文在 **编译期嵌入** 的 `router-rs` 中（`AGENTS.md` + `AGENTS_CODEX.md` concat）；改文后须 rebuild + `framework sync-entrypoints`（见 [`AGENTS_CODEX.md`](../AGENTS_CODEX.md) §Codex 构建快照）。

## 出站文本被「砍一半」

Cursor 对 `additional_context` / 过长 `followup_message` 有 **UTF-8 字节上限**（变量名常含 `_CHARS`，语义为字节），超长时**保留前缀**并带截断标记；若门控句在段落后合并，可能先被裁掉——见 [harness_architecture.md](harness_architecture.md) 第 4.2 节与文中环境变量表脚注。

## 上一轮问题矩阵对照（摘要）

| 问题 | 处理 |
|------|------|
| 宿主不对称「假安全」 | 上表 + README 路径 B 声明 |
| `REVIEW_GATE` 难排障 | `need=` + `hint=` + host_adapter 链 |
| 误把 `RG_FOLLOWUP` 当真注入或当清门令牌 | 本节「机读短码真源与常见误报」「粘贴清门」+ harness §4.3 |
| review 与 My implement 同轮混写 | 本节「混用时的实际武装顺序」 |
| 真源分散 | 本文「阅读顺序」+ 不猜 slug |
| 上手重 | README 路径 A / B 分流 |
| Codex 策略漂移 | `cargo build` + `framework sync-entrypoints` + `framework doctor` 提示 |
| 环境变量命名 | harness 表前脚注 |
| 截断不可见 | 出站 `...[~trunc]` 类标记 + 文档 |

## 相关仓库入口

- 分享与安装：[README.md](../README.md)  
- 文档索引：[docs/README.md](README.md)  
