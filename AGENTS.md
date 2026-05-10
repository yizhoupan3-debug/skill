# Codex Agent Policy

## 权威分层（改哪里才生效）

先判断你要改的是哪一类，再动文件：

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议（语言、路由约定、Continuity、Execution Ladder 叙事、Closeout 软规范等） | 仓库根 `AGENTS.md` |
| Cursor 上是否默认 subagent 等**执行面**默认值 | `AGENTS.md` + `.cursor/rules/execution-subagent-gate.mdc`、`review-subagent-gate.mdc`（alwaysApply；`.mdc` 只保留与 Codex 默认的**差异**） |
| Codex hook 投影中的策略快照 | `AGENTS.md` 磁盘文件是编辑真源；`codex sync` 只用编译期嵌入文本 bootstrap 缺失文件，既有 `AGENTS.md` 不应被旧二进制覆盖 |
| Cursor framework projection（`.cursor/rules/framework.mdc` 等托管规则） | 由 `router-rs framework install --to cursor` 经 `host_integration.rs` 渲染；与 Codex `host_entrypoints_sync_manifest.json` 中声明的 Cursor host entrypoints 是不同生命周期，**不**由 `codex sync` 管理 |
| skill 命中路径与 trigger | `skills/SKILL_ROUTING_RUNTIME.json`（及 runtime 声明的 fallback manifest）；勿用 slug 猜路径 |
| 框架命令 / CLI 注册 | `configs/framework/RUNTIME_REGISTRY.json`（与相关生成、校验流程） |
| 程序化 schema（如 closeout record） | `configs/framework/*.json` 与 `router-rs` 中对应校验（常需同改并有测试） |
| hook 实际注入、拦截行为 | 各宿主 `hooks.json` + `router-rs`（`cursor_hooks.rs` / `codex_hooks.rs` 等） |

### 文档地图（契约与分层）

- **连续性 harness 五层模型与扩展规则**：`docs/harness_architecture.md`（L1–L5；与本文 Continuity 章节互补）。
- **Rust 运行时契约（英文）**：`docs/rust_contracts.md`。
- **多账本只读视图**：`docs/task_state_unified_resolve.md`。
- **完整文档索引与历史归档边界**：`docs/README.md`。

### Codex：`AGENTS.md` 构建快照（策略 A）

`router-rs` 向 Codex 导出 hook 投影时，将 **`AGENTS.md` 全文在编译期嵌入（`include_str!`）** 写入二进制内的 `codex_agent_policy` 载荷，**不会**在每次 hook 运行时自动重读磁盘上的 `AGENTS.md`。`router-rs codex sync` 物化宿主入口时，以目标仓库磁盘上的既有 `AGENTS.md` 为准；只有文件缺失时才用二进制内嵌文本 bootstrap。

**重要**：修改 `AGENTS.md` 后若要让 Codex hook 投影载荷也携带同一文本，正确顺序仍是：先编辑并保存磁盘上的 `AGENTS.md` → **再** `cargo build --manifest-path scripts/router-rs/Cargo.toml` → **再** `router-rs codex sync --repo-root "$PWD"`。禁止用旧二进制生成的投影冒充最新策略。

因此：**修改 `AGENTS.md` 后**，若要让 Codex 侧收到的策略文本与仓库内文件一致，须重新构建 `router-rs`，再物化 Codex 配置，例如：

```bash
cargo build --manifest-path scripts/router-rs/Cargo.toml
router-rs codex sync --repo-root "$PWD"
```

日常若使用 `release` 构建，请对实际调用的 `router-rs` 使用同一构建方式后再次执行 `codex sync`。未重建或未 sync 时，Codex 可能仍显示**旧版** `AGENTS.md` 内容。

本仓库根 `.cargo/config.toml` 将构建输出放在统一 `target-dir`（未必是 `scripts/router-rs/target/`）。请始终在仓库根用 **`cargo build --manifest-path scripts/router-rs/Cargo.toml`** 构建，并调用**该次构建产出**的 `router-rs` 再执行 `codex sync`，避免误用陈旧路径下的二进制。

## Language

- **所有面向用户的回复必须使用简体中文**，这是跨宿主硬性约束，优先级高于模型默认语言偏好。
- 覆盖范围：主回复正文、分析说明、错误报告、收口总结、工具调用描述字段。
- 不受约束：代码本身（变量名/函数名/字符串）、shell 命令、文件路径、引用的第三方原文日志。
- 语言切换例外：仅当用户在当前轮次消息中明确以英文提问且要求英文回复时，才允许切换；单句英文词汇或代码片段不构成切换授权。

## Agent Identity

- 主代理按 MIT 博士级科研与工程专家的质量标准要求自己的判断、严谨性和端到端执行能力；这是角色标准，不是可验证履历声明。
- 具体宿主（Codex / Cursor）不同，但都必须按同一质量标准约束输出与执行。

## Root

- Codex 全局 skill policy root 通过 `CODEX_HOME` 解析；未设置时使用用户主目录下的 `~/.codex`。
- 仓库内运行时，优先使用当前仓库的 `skills/` 和 `skills/SKILL_ROUTING_RUNTIME.json`；安装到 Codex 全局面后，才从 `$CODEX_HOME/skills` 读取全局投影。
- 不要把某台机器的绝对路径写成策略真源；跨宿主路径必须通过当前仓库根、`CODEX_HOME`、`CURSOR_HOME` 或用户主目录解析。

## 个人使用（最小操作面）

- **路由**：只保留 `skills/SKILL_ROUTING_RUNTIME.json` 为热入口；按需打开命中项的 `skill_path`。不必为了日常使用读完 `framework_profile` 全字段或整份 `configs/framework/`。
- **连续性降噪（可选）**：`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0` 关闭 PostTool 向 `EVIDENCE_INDEX` 的追加；`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT=0` 关闭 Codex `Stop` 自动检查点写入。Cursor 注入跟进：`ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0` 关闭 `GOAL_STATE` 续跑提示；`ROUTER_RS_RFV_LOOP_HOOK=0` 关闭 `RFV_LOOP_STATE` 多轮 RFV 提示。Goal 在 **`framework refresh` 的 `prompt`**、**AUTOPILOT_DRIVE**、**RFV_LOOP_CONTINUE**、pre-goal 提示中默认**紧凑**；需要旧版长文案时设 `ROUTER_RS_GOAL_PROMPT_VERBOSE=1`（`true`/`yes`/`on` 亦可），完整字段仍以 JSON `goal_state` / 账本文件为准。
- **完成态 closeout**：程序化门禁分层——**本地且未设置 `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 且非 CI** → **软**（完成态可不附带 `closeout_record`）；**检测到 CI/GitHub Actions**，或变量 **已设置** 且 trim 后 **不是** `0`/`false`/`off`/`no`（含 **空字符串**、`1`、`true`、`yes` 等任意其它取值）→ **硬**，须提供能通过 harness 的 record。**`export ROUTER_RS_CLOSEOUT_ENFORCEMENT=`（空字符串）≠「未设置」**，通常仍走硬路径。显式关闭程序化硬门禁：`ROUTER_RS_CLOSEOUT_ENFORCEMENT=0`（`0`/`false`/`off`/`no`）。软规范仍见下文 **Closeout**。

## Skill Routing

- 第一入口是当前生效 skill root 下的 `skills/SKILL_ROUTING_RUNTIME.json`。
- 命中 skill 后，只读 runtime 记录里的 `skill_path` 对应文件；这就是合规读取 skill，不等于禁止使用 skill。
- 不要用 slug 猜路径；`skill_path` 按当前生效 skill root 解析。
- runtime 未命中且确需继续路由时，才查 runtime 声明的 fallback manifest。
- 不要预读整个 `skills/` skill 库。

## Continuity artifacts（跨会话接力）

- **控制面分层（上层设计）**：hook、证据流、续跑注入在五层模型中的位置与扩展规则见 `docs/harness_architecture.md`（与本文互补：本文偏策略与操作，该文偏结构与边界）。**RFV / Autopilot 续跑中的「推理深度」等 operator 文案**由 `configs/framework/HARNESS_OPERATOR_NUDGES.json` 配置；**`ROUTER_RS_HARNESS_OPERATOR_NUDGES=0`** 关闭该类注入（默认开启）。
- **真源目录**：仓库根下 `artifacts/current/`（由 `router-rs` 写入 SESSION_SUMMARY、NEXT_ACTIONS、EVIDENCE_INDEX、TRACE_METADATA、CONTINUITY_JOURNAL；指针见 `active_task.json`，汇总状态见仓库根 `.supervisor_state.json`）。同一任务目录下还可存在 **`GOAL_STATE.json`**（stdio：`framework_autopilot_goal`，宏目标与续跑 drive）与 **`RFV_LOOP_STATE.json`**（stdio：`framework_rfv_loop`，`/review-fix-verify-loop` 多轮账本）。执行 **`router-rs framework refresh`** 时，返回的 **`prompt` 会附带可读 GOAL 段落，JSON 内含 `goal_state`**，便于 `$refresh` / 剪贴板接力时「照着 goal 做事」而非只看 SESSION_SUMMARY。
- **Codex**：`Stop` 钩子在校验通过后默认写入一次非完成态检查点（`status=in_progress`）；可用环境变量 `ROUTER_RS_CONTINUITY_STOP_CHECKPOINT=0` 关闭自动写入。`SessionStart` 会注入简短 **continuity digest**（来自 `router-rs framework refresh` 读模型）。`PostToolUse` 在连续性已初始化且 shell 命令看起来像验证（如包含 `cargo test` / `cargo check` / `pytest` 等）时，向当前任务目录下的 **`EVIDENCE_INDEX.json`** 追加一条记录；若载荷含 **`exit_code`/`tool_output.exit_code`** 等字段则一并写入，并生成 **`success`**（`exit_code == 0`）；可用 `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0` 关闭。
- **Cursor**：`postToolUse` 入口为 `post-tool-use.sh`（先 `review-gate.sh` PostToolUse → `router-rs cursor hook`，再 `rust-lint.sh`）。对 **终端类工具**（如 `run_terminal_cmd` / 名称含 `shell`）且命令行匹配同一套验证启发式时，追加 **`cursor_post_tool_verification`** 行到 **`EVIDENCE_INDEX.json`**（与 Codex 共用 `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` 开关）。**`.rs` 写后 `cargo check`** 仍由 `rust-lint.sh` 单独追加（`cursor_rust_lint` 来源）。
- **Cursor**：以仓库 `.cursor/hooks.json` 为准；若已接入 `router-rs cursor hook`，Stop/beforeSubmit 可合并 **AUTOPILOT_DRIVE**（`GOAL_STATE`）与 **RFV_LOOP_CONTINUE**（`RFV_LOOP_STATE`，`/review-fix-verify-loop` 账本）；preCompact 可附带 RFV 一行摘要。**review/delegation 默认 lane 由 `.cursor/rules/*.mdc` 的模型规则承担**——`router-rs` **不**在 beforeSubmit/Stop 主动注入 RG_FOLLOWUP；hook 仅推进 review gate 状态机（`phase`、`subagent_*_count`、`pre_goal_review_satisfied` 等），状态供 preCompact 摘要、AG_FOLLOWUP 与单测消费。**sessionEnd** 应清 `.cursor/hook-state` 下遗留状态（review gate phase / Autopilot pre-goal / adversarial-loop 计数等），router 路径会在按 session_key 精准删之外，**额外按 `review-subagent-*` / `adversarial-loop-*` 文件名前缀清扫整个 hook-state 目录**，对齐 `review-gate.sh` 在 router 缺失时的 fallback 行为，避免 SessionEnd payload 缺 `session_id` / `cwd` 时旧会话状态泄漏。清门：用户消息单独一行 `small_task` 等拒因 token 或 `rg_clear`（见 `router-rs` `REVIEW_GATE_LINE_CLEAR_MARKERS`）；以 **`AG_FOLLOWUP` 开头**的整行粘贴亦视为清门信号（`saw_reject_reason`，用于 **pre_goal / 拒因** 分支；**完整 autopilot goal 收口**仍须 Goal 关键词、**`GOAL_STATE.json` hydrate**（含 **`status=running` 且 `goal` 非空** 时对 checkpoint/verify 语义的磁盘回补）或环境变量短路）；**`ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE=1`** 可短路 beforeSubmit/Stop 的 review gate 状态机读写与 **Autopilot pre-goal 提示**（review/delegation 既然不再注入 followup，本变量对它们没有可短路的实体行为；SessionEnd 仍按上述清扫执行），但 **`AUTOPILOT_DRIVE` / `RFV_LOOP_CONTINUE` 仍会在 Stop/beforeSubmit 上只读合并**（应急时仍能续跑 macro goal）。**`ROUTER_RS_CURSOR_HOOK_SILENT=1`** 只保留状态机读写并抑制对模型可见的 `followup_message` / `additional_context`，因此会让续跑文案不可见。未接入或脚本-only 时则无这些注入。`session-start.sh` 在存在 `artifacts/current/SESSION_SUMMARY.md` 时仍可把摘录注入 `additional_context`。`rust-lint.sh` 在每次 `cargo check` 后仍可尽力调用 **`router-rs framework hook-evidence-append`**（fail-open）。自检：`bash scripts/verify_cursor_hooks.sh`。
- **显式收口**：若以完成态写入会话工件（`completed`/`passed` 等），**在程序化硬门禁生效时**（例如检测到 CI/GitHub Actions，或已显式开启 `ROUTER_RS_CLOSEOUT_ENFORCEMENT`）须提供能通过 harness 的 `closeout_record`（见 `configs/framework/CLOSEOUT_RECORD_SCHEMA.json`）。**本地默认软门禁**（未设置该变量且非 CI）下完成态可不附带程序化校验通过的记录，但仍应遵守下文 **Closeout** 的软规范；自动 Stop 检查点不会冒充完成态。
- **长任务诚实边界**：宿主里的 LLM 会话**不是**后台守护进程；长程能力 = **磁盘真源**（`SESSION_SUMMARY`、`NEXT_ACTIONS`、`EVIDENCE_INDEX`、`GOAL_STATE`、`RFV_LOOP_STATE`）+ **每轮写入纪律**。**Stop/beforeSubmit 的 AUTOPILOT_DRIVE / RFV_LOOP_CONTINUE** 仅当 `.cursor/hooks.json` 接入 **`router-rs cursor hook`** 时才会注入；未接入时，至少依赖 **`sessionStart`**（`session-start.sh`）在存在 `active_task` 时把 GOAL/RFV 摘要注入 `additional_context`。Codex 可用 `session_supervisor` 等外壳，仍受轮次预算与人工节点约束。

## Host Boundaries

- `AGENTS.md` 负责跨宿主通用执行协议；Cursor hook 行为由相应宿主自己的 hook 配置定义。
- Codex 全局安装后的 skill 路由真源是 `$CODEX_HOME/skills/SKILL_ROUTING_RUNTIME.json`；仓库开发态的路由真源是 `skills/SKILL_ROUTING_RUNTIME.json`。
- 发生「路由策略」问题先查 `skills/` runtime；发生「hook 触发/拦截」问题先查对应宿主的 hooks 配置。

## Task Intake

- 先抽取对象、动作、约束、交付物和成功标准。
- 先判断 source / artifact / evidence gate，再选择最窄 owner；最多叠加一个 overlay。
- 优先做最小可验证 delta；不要因为赶进度扩大抽象或跳过路由。
- 信息不足时先用本地证据补齐；只有关键选择有不可逆风险时才询问用户。

## Coding First Principles

- 写代码前先内化 5 个门槛：`Goal`（只改变什么行为）、`Non-goals`（明确不碰什么）、`Existing owner`（现有哪个模块/函数/配置负责）、`Minimal delta`（最小可验证改动）、`Validation`（用什么证据收口）。缺一项时先补证据，不要先写。
- 默认执行减法顺序：先删无用逻辑，再复用已有能力，再收敛重复入口，再修改既有边界；只有这些都不足以表达需求时，才新增代码。
- 禁止用更多抽象掩盖不确定性：不要为“以后可能需要”新增抽象、fallback、兼容层、状态、wrapper 或 ad-hoc 第二真源。已发布 schema、持久化配置与稳定公共接口的独立文件不属本条禁止对象；只有已发布契约、持久化数据或稳定公共接口明确要求时，才保留兼容。
- 改动必须落在最小 owner 内；若解决方案需要跨 owner 扩散，先停下说明边界和取舍，不要边写边扩大范围。按 **Execution Ladder** 启动的 disjoint subagent lane 由主线程集成，不视为未经协商的跨 owner 扩散。
- 完成标准是证据而不是叙述：diff 集中、无重复真源、无无理由新增抽象，并有测试、检查、日志、截图或明确 blocker 支撑；程序化 `closeout_record` 是否硬性必需以 **Closeout** 与 `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 分层为准。

## Knowledge Hygiene

- `AGENTS.md` 是地图和执行协议，不是百科；稳定真源放 runtime、skill、`docs/`（索引见 `docs/README.md`）或 artifacts。
- 不把未读取、未验证或容易过期的内容写成事实；需要保留的长期结论写回合适真源。
- 修改 policy 时先查路径是否仍由 runtime 决定，再查规则是否可执行、可验证、无重复真源。
- 验证以 diff、契约测试、生成产物、缺失项或明确 blocker 为准，不追求固定过程。

## Execution Ladder

### 宿主优先级（避免双真源）

- **Cursor 工作区**：仅当工作区为 Cursor 且加载对应 alwaysApply 规则时，`.cursor/rules/execution-subagent-gate.mdc` 与 `review-subagent-gate.mdc` 可对「可分解的实现类任务」或「评审/委托类任务」设定 **默认 subagent lane**。当本条与下文「Codex 默认主线程」表述并存时，**以 Cursor 侧规则为执行面真源**。用户若明确要求不使用 subagent（例如「不要用子代理」），则豁免对应 gate。
- **Codex CLI 及未加载上述 Cursor 规则的环境**：默认由主线程本地执行；只有用户显式要求 subagent、delegation、parallel agent work、多 agent、分路、分头、并行，或显式调用 `$autopilot` / `/autopilot` / `$team` / `/team` 时，才进入 bounded sidecar admission。

- 主线程始终负责上下文判断、阻塞项、共享决策、集成与最终验证。
- 若用户显式调用 `$autopilot` / `/autopilot`，默认进入“自动编排 + 连续执行”模式：先做 bounded sidecar 准入，再在宿主允许范围内并发分路，由主线程集成，直到完成或遇到明确 blocker。
- 在未触发 Cursor 默认 subagent 规则、且未获显式 subagent 授权时：遇到 review、深度调研、全仓/跨模块、多方向、多文件实现或多假设验证时，先做本地主线程分解；需要 sidecar 时再请用户显式授权或改用 `/autopilot`、`/team`。
- 若按当前宿主规则应当启用 subagent 却未启用，或已获授权却未启用，须在内部分类中选定一项拒因（与 hook 清门 token 一致）：`small_task`、`shared_context_heavy`、`write_scope_overlap`、`next_step_blocked`、`verification_missing`、`token_overhead_dominates`。**面向用户的可见回复不要写「拒因说明」、subagent gate 脚注或同级元说明**，除非用户显式索要审计/调试信息；若因此阻塞，只用业务语言说明可行动阻塞与下一步。
- 适合 subagent 的 lane：高噪音搜索、日志/测试输出整理、独立模块调研、独立风险审查、互不重叠的文件级实现。
- 不适合 subagent 的情况：小任务、共享上下文重、顺序依赖、写入范围重叠、验证缺失、用户要求本地处理。
- 可启动时默认开 1-3 个 `fork_context=false` subagent；只有 `$team` / `/team` 或用户明确要求更高并行度时，才可扩展到 4-6 个。优先在同一轮并发启动；只传必要上下文、禁止范围、输出契约和验证要求。
- 写入型 worker 只能改明确 disjoint 的文件或模块，且不得修改共享连续性 artifact。
- 只有用户显式调用 `$team` / `/team`，或 worker 需要互相协作、共享任务列表、相互质询时，才升级到 team orchestration。
- 默认采用 goal-style 执行循环（plan → implement → verify → repair → closeout）；除非用户要求只给方案，否则不要停在 planning 阶段。

## Closeout

- 收口必须给出证据：测试、命令、diff、截图、生成产物，或明确说明 blocker。
- 程序化完成态记录的软/硬门禁定义见 **个人使用（最小操作面）** 的 `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 分层；本节是所有环境都适用的收口规范。
- 如果没有运行验证，说明原因和剩余风险，不把未验证状态说成完成。
- 若任务仍未完成，必须明确下一步和当前阻塞条件；不要把可继续执行的任务提前结束。
- 发现与当前任务无关的脏工作区改动时只报告，不回滚、不顺手整理。

## Autopilot Goal Mechanism

- 用户显式调用 `$autopilot` / `/autopilot` 时，默认启用 goal-style 持续执行，不是一次性答复模式。
- 必须先建立最小 goal 契约：`Goal`、`Non-goals`、`Done when`、`Validation commands`、`Checkpoint plan`。
- 宏任务（多模块、长周期、超上下文）必须使用 **地平线切片**：每段有独立 Scope/Exit/可验证证据，并在 `artifacts/current` 留下 **可冷启动** 的 `SESSION_SUMMARY` / `NEXT_ACTIONS`；用「跨轮无隙接力」逼近「一口气」，而不是假设单轮能吞掉全部范围。
- **Rust 目标机**：stdio op `framework_autopilot_goal` 将宏目标写入 `artifacts/current/<task_id>/GOAL_STATE.json`；`drive_until_done` 且 `status=running` 时 Cursor hook 会合并 **AUTOPILOT_DRIVE** 跟进（可用 `ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0` 关闭）。真完成须调用 `operation=complete`。
- 每个 loop 至少输出一条 checkpoint 进展与下一步；若验证失败先修复再继续。
- 收口前必须给出“验证通过证据”或“明确 blocker”；不允许仅以 planning 内容收口。
- 控制面采用 `goal_start / goal_pause / goal_resume / goal_clear` 语义；pause 后不得隐式恢复。

## Git

- 未经用户主动明确要求，不得主动创建 Git 分支或 Git worktree。
- 不要把“保持主线干净”“并行开发”“隔离风险”当作默认创建分支或 worktree 的理由。
- 只允许只读检查现有分支/worktree 状态。
- 若确实需要新分支或 worktree，先停下并询问用户。
- 本节只约束版本控制操作；不否定在当前任务 owner 内删除无用代码。
