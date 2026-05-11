# Continuity harness — upper-level architecture

本文件是 **宿主 hook + router-rs + 连续性工件** 的**上层设计真源**：说明分层、数据流与扩展规则，避免在代码里零散堆「又一个环境变量 / 又一个硬编码提示句」。

**与 `AGENTS.md` 的分工**：`AGENTS.md` = 跨宿主**执行与语言策略**；本文 = **控制面结构**（谁写盘、谁注入、谁算证据）。

**文档索引**：steady-state 契约导航与历史边界见 [`README.md`](README.md)（本目录）；多账本只读聚合见 [`task_state_unified_resolve.md`](task_state_unified_resolve.md)。跨 Cursor 工作区接入操作见仓库根 [`README.md`](../README.md)「其它仓库一键接入」与「建议自检命令序列」（约 L147–L192）。

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

### 2.1 Review：skill 路由、执行偏好与 REVIEW_GATE 三层

下列三层**不要混为一谈**：关路由 ≠ 关门控；编辑器侧规则也不替代磁盘相位。

1. **（a）Skill 路由与 trigger 提示**：`skills/SKILL_ROUTING_RUNTIME.json`（及 manifest）负责命中 `skill_path`、关键词与 skill 内契约；**不实现** REVIEW 门控状态机。
2. **（b）Cursor 规则与 `AGENTS.md` 执行偏好**：`.cursor/rules/*.mdc` 与 `AGENTS.md` 的 **Execution Ladder** 描述何时倾向 review/subagent；属**跨宿主执行叙事**，不等于 hook 内算法。
3. **（c）`router-rs` REVIEW_GATE 状态机（L3）**：`hook_common`、`cursor_hooks` 等解析 Cursor 事件、更新 **`.cursor/hook-state`**、在 Stop 注入 **`router-rs REVIEW_GATE`** 单行短码。路由侧与门控侧共享的结构化信号真源见 [`REVIEW_ROUTING_SIGNALS.json`](../configs/framework/REVIEW_ROUTING_SIGNALS.json) 与 [`review_routing_signals.rs`](../scripts/router-rs/src/review_routing_signals.rs)。

**本机自检清单**（可选，排障时逐项过）：

- `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE`：是否仅为调试临时关闭（门控短路；续跑仍可合并，见 §8 表）。
- `ROUTER_RS_CURSOR_HOOK_SILENT`：若开启，确认 §8 对含 `REVIEW_GATE` 等字样的 followup **保留**语义仍符合预期。
- `.cursor/hooks.json`：**已注册的事件**（如 `beforeSubmit` / `Stop` 等）是否都指向预期的 `router-rs cursor hook` 子命令。
- `.cursor/hook-state`：目录可写、无长期锁失败；异常时再对照仓库根 [`AGENTS.md`](../AGENTS.md)（不重复长政策正文）。

- **Cursor Plan 收口**：若本次改动纳入某 Cursor Plan，在仓库根执行 **`/gitx plan`** 对照计划逐项验收（以本机实际输出为准；**不要**在文档或叙述中编造执行结果）。

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
- **调研深度（外研加强）**：结构性外研输出、可复核检索轨迹、多视角真分离 — 见 `docs/references/rfv-loop/reasoning-depth-contract.md` §**提升调研深度的 harness 方向**（与 `lane-templates.md` 外研深度模式一致；**不以** L3 hook 长文案代替）。
- **硬门禁（opt-in）**：`GOAL_STATE.completion_gates`（`framework_autopilot_goal` **`complete`**）、`RFV_LOOP_STATE.close_gates`（显式 **`append_round` close** 或 **`max_rounds` 耗尽**自动 closed）在开启时读取 **`resolve_task_view` / `DepthCompliance`** 校验；默认关闭、与 advisory rollup 分工见 [`reasoning-depth-contract.md`](references/rfv-loop/reasoning-depth-contract.md) §**可程序化硬门禁**。

---

## 5. 扩展规则（避免继续「加抽象」失控）

1. **新宿主行为** → 先标清属于 L3 哪条管道（PostTool / Stop / refresh），再实现；禁止在 L4 bash 里复制 L3 逻辑。
2. **新 env 开关** → 仅在 **跨用户可见噪音 / 合规** 需要时添加；**优先**收束到 `router_env_flags` + 文档表。**例外**：少量宿主专用或窄作用域旋钮（例如 **Codex** `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX`）目前可在对应模块按需读 `std::env::var`；跨宿主或通用开关仍应集中到 `router_env_flags`。**禁止**在随机模块零散增加裸 env 读出而不登记文档。
3. **新验证启发式** → 必须 **可测**（单测含命令样例）；宁可 **少而准**，用 `hook-evidence-append` 补长尾。
4. **新 operator 文案** → 默认进 **L5 文档**；注入宿主时以 **`configs/framework/HARNESS_OPERATOR_NUDGES.json`** 为真源（`router-rs` 启动时合并内置默认值）。Schema 不匹配会**回退到内置默认**（不再做部分合并）。**关闭全部此类注入**：`ROUTER_RS_HARNESS_OPERATOR_NUDGES=0`（与其它 `ROUTER_RS_*` 软关断语义一致）。Schema 说明见同目录 `HARNESS_OPERATOR_NUDGES_SCHEMA.json`。
5. **同时关掉所有续跑/nudge + 可选论文对抗 hook** → `ROUTER_RS_OPERATOR_INJECT=0`（聚合关断；P1-E）。等价于同时设 `ROUTER_RS_HARNESS_OPERATOR_NUDGES=0` + `ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0` + `ROUTER_RS_RFV_LOOP_HOOK=0`，并在你已启用时还关掉 Cursor **`beforeSubmit`** 的 **`PAPER_ADVERSARIAL_HOOK`**（见 `configs/framework/PAPER_ADVERSARIAL_HOOK.txt`），单变量更易调试。

---

## 6. 与仓库文件的映射

| 概念 | 主要落地 |
|------|----------|
| 宿主适配契约 | [`host_adapter_contract.md`](host_adapter_contract.md)（portable core、事件→CLI、新宿主 checklist；**闭集宿主**以 `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported` 为准） |
| 新宿主接入（快速路径 / 工程顺序） | [`host_adapter_contract.md`](host_adapter_contract.md) 文首 **快速路径**；工程勾选表 [§3.1](host_adapter_contract.md#31-可复制执行清单工程顺序) |
| L4 | `.cursor/hooks.json`（每条命令直指 `router-rs cursor hook`，无 shell shim）、Codex `hooks.json` |
| L3 | `scripts/router-rs/src/{cursor_hooks,codex_hooks,hook_common,review_gate,framework_host_targets,framework_runtime,rfv_loop,autopilot_goal,task_state,task_state_aggregate,task_command,task_write_lock,harness_operator_nudges}.rs` |
| L2 | `artifacts/current/`、`configs/framework/*SCHEMA*` |
| L5 | `skills/**/SKILL.md`、`docs/rfv_loop_harness.md`、`docs/references/rfv-loop/*`（含 [`math-reasoning-harness.md`](references/rfv-loop/math-reasoning-harness.md)；非热 skill 路由） |
| **Skill 路由中的宿主 id（`host_support.platforms`）** | `skills/<slug>/SKILL.md`（frontmatter）→ `cargo run … skill-compiler-rs --apply` → `skills/SKILL_ROUTING_RUNTIME.json`；值域与 `RUNTIME_REGISTRY.host_targets.supported` 对齐（见 [`host_adapter_contract.md`](host_adapter_contract.md) 维护表末行） |
| 弱模型 / 上下文预算与注入审计 | [`plans/RESEARCH_harness_weak_model_top_tier.md`](plans/RESEARCH_harness_weak_model_top_tier.md)（调研合成），[`plans/context_token_audit_deep_dive.md`](plans/context_token_audit_deep_dive.md)（Codex cap / Cursor `merge_additional_context` 路径） |

---

## 7. 刻意不做的事

- 不在本文定义具体模型名、定价或 Cursor Auto 路由（属产品侧，易变）。
- 不把 **closeout 硬门禁** 规则重复写全（真源仍在 `closeout_enforcement` + schema）。

维护：当新增一类 hook 行为或全局开关时，**至少更新本节 §5 与 §6 表格中的一行**；若牵涉新宿主或可移植边界，同步 [`host_adapter_contract.md`](host_adapter_contract.md)，避免「只有代码没有地图」。

**读模型**：多账本统一只读聚合见 [`task_state_unified_resolve.md`](task_state_unified_resolve.md)（`router-rs` `task_state` / `framework task-state-resolve`；阶段 3 另见 `TASK_STATE.json` 与 `framework task-state-aggregate-sync`）。完整文档目录见 [`README.md`](README.md)。

---

## 8. 开关取舍矩阵（深度注入相关）

每个开关「关」时影响的注入面不同；下表给出对照，避免误以为关一个等于关全部。

| 环境变量 | 默认 | 关闭后影响（其余面不变） |
|---------|------|------------------------|
| `ROUTER_RS_OPERATOR_INJECT` | 开 | **聚合关断**：推理 nudge + AUTOPILOT_DRIVE + RFV_LOOP **及**（若启用）Cursor beforeSubmit **`PAPER_ADVERSARIAL_HOOK`** 全部消失 |
| `ROUTER_RS_HARNESS_OPERATOR_NUDGES` | 开 | 仅去掉 `HARNESS_OPERATOR_NUDGES.json` 注入的 operator 文案（含 **推理深度** 三键与可选 **`math_reasoning_harness_line`** / **`retrieval_trace_harness_line`**）；RFV/AUTOPILOT 续跑骨架仍在。**不**影响：continuity digest 主线 `prompt` 里的 **`深度信号: dN/3`**（`depth_compliance` rollup）与 GOAL 段落内硬编码的 **深度自检** 行（见 `framework_runtime::continuity_digest`） |
| *（脚注 #3）* | — | **`深度信号` 行**与 digest 内 **`depth_compliance_refresh_hint`** 走 `task_state` rollup，**不受**本行开关关断；若将来要「一条 env 关掉 digest 内全部深度提示」，属 **breaking 产品决策**，须另开开关或版本化 digest 契约（见 `docs/plans/RESEARCH_harness_depth_longrun_math.md` Open #3）。 |
| `ROUTER_RS_AUTOPILOT_DRIVE_HOOK` | 开 | 整个 **AUTOPILOT_DRIVE** 续跑块（含其内的 nudge 句）消失 |
| `ROUTER_RS_RFV_LOOP_HOOK` | 开 | 整个 **RFV_LOOP_CONTINUE** 续跑块（含其内的 nudge 句）消失 |
| `ROUTER_RS_GOAL_PROMPT_VERBOSE` | 关（默认紧凑） | 仅切换 verbose/compact 模板；与「是否注入」无关 |
| `ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT` | 关（**opt-in**） | 仅 Cursor **`beforeSubmit`** 合并 **AUTOPILOT_DRIVE** 续跑块（**Stop** 仍由 `ROUTER_RS_AUTOPILOT_DRIVE_HOOK`） |
| `ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT` | 关（**opt-in**） | 仅 Cursor **`beforeSubmit`** 合并 **RFV_LOOP_CONTINUE**（**Stop** 仍由 `ROUTER_RS_RFV_LOOP_HOOK`） |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED` | 关（**opt-in**） | `/autopilot` **pre-goal** beforeSubmit 注入与计数放行；不影响磁盘 `GOAL_STATE` 门控 |
| `ROUTER_RS_CURSOR_HOOK_CHAT_FOLLOWUP` | 关 | 改写入 `additional_context` vs `followup_message` |
| `ROUTER_RS_CURSOR_HOOK_SILENT` | 关 | 输出层整段剥离（含 nudge）；**例外**：含 `CLOSEOUT_FOLLOWUP` / `AG_FOLLOWUP` / `REVIEW_GATE` / `PAPER_ADVERSARIAL_HOOK` / `pre-goal 提示已达上限` / `hook-state 锁不可用` 字样的 followup 会**保留**，避免静默丢失硬阻塞与合规提示|
| `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` | 关 | 仅短路 review/delegation 门控；**续跑仍合并** |
| `ROUTER_RS_REVIEW_GATE_SUPPRESS_ON_MANUSCRIPT_CONTEXT` | 关（**opt-in**） | 为 on 时，`hook_common::is_review_prompt` 在命中 review 正则且路由侧 **`has_paper_context`** 为真、且无强代码/PR 锚点时返回 **false**，减轻手稿话术误触 **`REVIEW_GATE`**；**不**影响 `skills/SKILL_ROUTING_RUNTIME.json` 路由；**不受** **`ROUTER_RS_OPERATOR_INJECT`** 总闸约束 |
| `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK` | 关（**opt-in**：须显式 `1`/`true`/`yes`/`on`） | Cursor **`beforeSubmit`**：论文类用户提示合并 **`PAPER_ADVERSARIAL_HOOK`** 短段（强对抗审稿禁令摘要）；文案真源 **`configs/framework/PAPER_ADVERSARIAL_HOOK.txt`**；受 **`ROUTER_RS_OPERATOR_INJECT`** 总闸约束 |
| `ROUTER_RS_DEPTH_SCORE_MODE` | `legacy`（未设置与其它取值同 legacy） | 设为 **`strict`** 时，`DepthCompliance.depth_score` 的第三分在「checkpoint / 对抗轮」之外还把 **falsification_tests 计数** 与（任务 `external_research_strict` 时）**strict 外研通过轮次**计入；用于 digest / gate 与同一 rollup 对齐 |
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` | 约 **640**（ clamp **256–8192**） | **Codex `SessionStart` / `UserPromptSubmit` `additionalContext`** 字符上限；超长时在 **换行边界**截断（见 `codex_hooks::truncate_codex_additional_context`）。宿主窄域读取，未放入 `router_env_flags` |

实现入口：**跨宿主 / 高频** router 行为的开关主路径在 [`router_env_flags`](../scripts/router-rs/src/router_env_flags.rs)；少数宿主窄域例外（例如上表 **`ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX`**）可由对应宿主模块解析 `std::env::var`。**`REVIEW_ROUTING_SIGNALS.json`** 中单条坏 regex 在加载时 **跳过该条**（其余 pattern 仍编译），仅当全部无效时才回落内置 literals（见 `review_routing_signals::compile_review_gate_regexes`）。新增通用开关请优先加到 `router_env_flags`（或确认为窄域例外后在本节与对应模块注释登记）。

---

## 9. 推理深度跨账本校验（review P0/P1）

| 信号 | 来源 | 消费方（程序化） |
|------|------|-----------------|
| `verify_result ∈ {PASS,FAIL,SKIPPED,UNKNOWN}` | `RFV_LOOP_STATE.rounds[]`（`append_round` 强校验枚举） | `DepthCompliance`（rolled-up counts） |
| `evidence_refs` / `cross_check` | RFV 写入 round 时自动 cross-link `EVIDENCE_INDEX` | `DepthCompliance.rfv_pass_without_evidence_count` |
| `claimed_passed_without_evidence` | `closeout_enforcement` R7（record 内自检） | `enforce_closeout_for_session_payload` 阻断 |
| `claimed_passed_without_evidence_index_rows` | `closeout_enforcement` R8（context-aware；读 EVIDENCE_INDEX） | 同上 |
| `goal_verify_or_block_seen` | `cursor_hooks::hydrate_goal_gate_from_disk`（已收紧：纯 has_goal_text 不够）| Stop AG_FOLLOWUP 决策 |
| `depth_score ∈ {0..3}` | `task_state::DepthCompliance`（`ROUTER_RS_DEPTH_SCORE_MODE=strict` 时第三分公式见上表） | **`task_state` / `ResolvedTaskView` 的 `depth_compliance`**；**digest `prompt` 一行 `深度信号`**（经 Codex SessionStart）；**`framework statusline` 段 `depth=dN`**（`PASS` 无对照证据时后缀 `!`）；**可选**与 `GOAL_STATE.completion_gates` / `RFV_LOOP_STATE.close_gates` 的 `min_depth_score` 对齐 |
| `completion_gates` / `close_gates` | 账本字段 + `autopilot_goal` / `rfv_loop` | 默认 off；开启后为 **硬门禁**（失败 Err、不落盘终态 / 不写收口轮）；RFV 侧含 **`max_rounds` 耗尽** 自动 closed 与显式 close 两条收口预览路径 |

详细深度契约（语义层）见 [`reasoning-depth-contract.md`](references/rfv-loop/reasoning-depth-contract.md)。
