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
- **RFV / Autopilot 注入文案（推理深度等 operator nudge）**：`configs/framework/HARNESS_OPERATOR_NUDGES.json`；`**ROUTER_RS_HARNESS_OPERATOR_NUDGES=0`** 关闭该类注入。
- **Rust 运行时契约（英文）**：`docs/rust_contracts.md`。
- **多账本只读视图**：`docs/task_state_unified_resolve.md`。
- **完整文档索引与历史归档边界**：`docs/README.md`。

### Codex：`AGENTS.md` 构建快照（策略 A）

Codex 侧可能使用 **编译期嵌入**的 `AGENTS.md` 文本（不会每次 hook 运行都读磁盘）。因此修改本文件后，如需让 Codex hook 投影同步更新，按下面顺序做即可：

```bash
cargo build --manifest-path scripts/router-rs/Cargo.toml
router-rs codex sync --repo-root "$PWD"
```

（更细实现与构建输出路径细节不在本文件展开；以 `docs/` 与代码为准。）

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
- **连续性降噪（可选）**：`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0` 关闭 PostTool 向 `EVIDENCE_INDEX` 的追加；`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT=0` 关闭 Codex `Stop` 自动检查点写入。Cursor 注入跟进：`ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0` 关闭 `GOAL_STATE` 续跑提示；`ROUTER_RS_RFV_LOOP_HOOK=0` 关闭 `RFV_LOOP_STATE` 多轮 RFV 提示。**论文强对抗审稿（可选）**：`ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK=1`/`true`/`yes`/`on` 时，在满足启发式的用户 **`beforeSubmit`** 上合并一段 **`PAPER_ADVERSARIAL_HOOK`**（hostile / worst-case 审稿姿态 + **软逃逸禁令**：禁止仅降口径 / 堆 limitation / rebuttal-only / 代码空诺 / 数学直觉化；文案真源 `configs/framework/PAPER_ADVERSARIAL_HOOK.txt`，由 `include_str!` 在编译期嵌入同一份内容作为缺失文件回落）；总闸与同表其它 operator 注入一致：`ROUTER_RS_OPERATOR_INJECT=0` 时也关闭该段。Goal 在 **Codex SessionStart continuity digest**、**AUTOPILOT_DRIVE**、**RFV_LOOP_CONTINUE**、pre-goal 提示中默认**紧凑**；需要旧版长文案时设 `ROUTER_RS_GOAL_PROMPT_VERBOSE=1`（`true`/`yes`/`on` 亦可），完整字段仍以磁盘 `GOAL_STATE.json` / 账本文件为准。
- **Cursor SessionEnd 终端回收（可选）**：默认在对话结束时仅向本会话 shell 账本登记的 Cursor terminal 发终止信号；`ROUTER_RS_CURSOR_KILL_STALE_TERMINALS=0`/`false`/`off`/`no` 关闭该步骤；若需恢复旧行为（按仓库 cwd 扫描**所有**仍 active 的集成终端）设 `ROUTER_RS_CURSOR_TERMINAL_KILL_MODE=legacy`（或 `all`/`repo`/`repo-wide`）。
- **完成态 closeout**：程序化门禁分层——**本地且未设置 `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 且非 CI** → **软**（完成态可不附带 `closeout_record`）；**检测到 CI/GitHub Actions**，或变量 **已设置** 且 trim 后 **不是** `0`/`false`/`off`/`no`（含 **空字符串**、`1`、`true`、`yes` 等任意其它取值）→ **硬**，须提供能通过 harness 的 record。**`export ROUTER_RS_CLOSEOUT_ENFORCEMENT=`（空字符串）≠「未设置」**，通常仍走硬路径。显式关闭程序化硬门禁：`ROUTER_RS_CLOSEOUT_ENFORCEMENT=0`（`0`/`false`/`off`/`no`）。软规范仍见下文 **Closeout**。
- **Autopilot pre-goal（Cursor，opt-in）**：默认关闭；需要 beforeSubmit 侧「独立 fork pre-goal」提示与计数放行时设 **`ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED=1`**。若开启后仍卡：**单独一行** `small_task` 可清门；自动放行次数由 **`ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES`** 控制（ unset 默认 **8**；设为 **`0`**/`false`/`off`/`no` 关闭自动放行）。Stop 上 goal 收口仍可能给出 **`router-rs AG_FOLLOWUP`**（与 pre-goal 注入独立）。

## Skill Routing

- 第一入口是当前生效 skill root 下的 `skills/SKILL_ROUTING_RUNTIME.json`。
- 命中 skill 后，只读 runtime 记录里的 `skill_path` 对应文件；这就是合规读取 skill，不等于禁止使用 skill。
- 不要用 slug 猜路径；`skill_path` 按当前生效 skill root 解析。
- runtime 未命中且确需继续路由时，才查 runtime 声明的 fallback manifest。
- 不要预读整个 `skills/` skill 库。

## Continuity artifacts（跨会话接力）

- **真源与结构**：连续性分层与边界以 `docs/harness_architecture.md` 为准；本节只保留高频使用要点，避免把实现细节写成第二真源。
- **真源目录**：`artifacts/current/`（SESSION_SUMMARY / NEXT_ACTIONS / EVIDENCE_INDEX / TRACE_METADATA / CONTINUITY_JOURNAL；指针 `active_task.json`，汇总 `.supervisor_state.json`）。
- **Goal / RFV**：同一 task 下可能出现 `GOAL_STATE.json` 与 `RFV_LOOP_STATE.json`；机器可读视图优先用 `router-rs framework snapshot` / `contract-summary`。
- **证据追加（可关）**：验证类命令会被追加到 `EVIDENCE_INDEX.json`（关：`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0`）。
- **续跑注入（可关）**：**Stop** 等路径：`ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0`、`ROUTER_RS_RFV_LOOP_HOOK=0`。Cursor **`beforeSubmit`** 默认**不**合并 **AUTOPILOT_DRIVE** / **RFV_LOOP_CONTINUE**；显式合并：`ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT=1`、`ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT=1`。
- **降噪与应急**：`ROUTER_RS_CURSOR_HOOK_SILENT=1` 可压制非必要提示（但合规/硬阻塞类提示仍应可见）；完整开关矩阵与语义以 `docs/harness_architecture.md` 为准。

## Host Boundaries

- `AGENTS.md` 负责跨宿主通用执行协议；Cursor hook 行为由相应宿主自己的 hook 配置定义。
- **Cursor（`router-rs cursor hook`）机读续跑/门控短码的真源示例**：`**AG_FOLLOWUP**`（Stop 上对未满足 autopilot goal 时由宿主注入 **`router-rs AG_FOLLOWUP`** 起头的单行短码）、`**REVIEW_GATE**`（Stop 上对未满足 review 子代理证据链时 **`router-rs REVIEW_GATE`**）、`**AUTOPILOT_DRIVE**`、`**RFV_LOOP_CONTINUE**`、`**CLOSEOUT_FOLLOWUP**` 等（以实际 `followup_message` / `additional_context` 为准）。**不要**在可见回复中自拟多段仿宿主的长篇机读排版；若某段看起来像 hook 却从未由宿主注入 **`router-rs …`** 起头的单行，应视为**非真源**。（已废止的双字母+FOLLOWUP 前缀与自拟「键值对式」仿真机读同样不是宿主注入。）确需清门仍只用 **单独一行**拒因 token（见 **Execution Ladder**）。
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

- **Cursor 工作区**：仅当工作区为 Cursor 且加载对应 alwaysApply 规则时，`.cursor/rules/execution-subagent-gate.mdc` 与 `review-subagent-gate.mdc` 提供执行面叙事（实现侧多为**建议**默认；**review** 侧对齐宿主 **`router-rs REVIEW_GATE`** 硬路径）。当本条与下文「Codex 默认主线程」表述并存时，**以 Cursor 侧规则为执行面真源**。用户若明确要求不使用 subagent（例如「不要用子代理」），则豁免对应 gate。
- **Cursor hook 可执行硬点（review）**：独立上下文 subagent 证据链由 **`router-rs`** 对 **review** 类请求校验（见 `.cursor/hook-state` phase 与 Stop 单行短码）；并行实现、`/autopilot`、RFV 等不与该硬路径共用 pre-goal 子代理门槛，其它开关见 **`ROUTER_RS_*`** 与 `docs/harness_architecture.md`。
- **Codex CLI 及未加载上述 Cursor 规则的环境**：默认由主线程本地执行；只有用户显式要求 subagent、delegation、parallel agent work、多 agent、分路、分头、并行，或显式调用 `/autopilot` / `/team` 时，才进入 bounded sidecar admission。

- 主线程始终负责上下文判断、阻塞项、共享决策、集成与最终验证。
- 若用户显式调用 `/autopilot`，默认进入“自动编排 + 连续执行”模式：先做 bounded sidecar 准入，再在宿主允许范围内并发分路，由主线程集成，直到完成或遇到明确 blocker。
- 在未触发 Cursor 默认 subagent 规则、且未获显式 subagent 授权时：遇到 review、深度调研、全仓/跨模块、多方向、多文件实现或多假设验证时，先做本地主线程分解；需要 sidecar 时再请用户显式授权或改用 `/autopilot`、`/team`。
- 若按当前宿主规则应当启用 subagent 却未启用，或已获授权却未启用，须在内部分类中选定一项拒因（与 hook 清门 token 一致）：`small_task`、`shared_context_heavy`、`write_scope_overlap`、`next_step_blocked`、`verification_missing`、`token_overhead_dominates`。**面向用户的可见回复不要写「拒因说明」、subagent gate 脚注或同级元说明**，除非用户显式索要审计/调试信息；若因此阻塞，只用业务语言说明可行动阻塞与下一步。**禁止**自拟多段仿 hook 的长篇机读块，或把 **`AG_FOLLOWUP`** 与长篇叙事粘在一起冒充宿主注入；真实 hook 文案由宿主注入。确需清门时：**单独一行**写一个 token，**不要**加标题或项目符号扩写。
- **对话体验**：默认不要在对话结尾复盘“开了哪些 lane / 起了哪些 subagent / 子任务怎么跑的”等过程性细节；面向用户的输出以结果、证据与下一步为主。仅当用户显式要求审计/调试时，才展开过程。
- 适合 subagent 的 lane：高噪音搜索、日志/测试输出整理、独立模块调研、独立风险审查、互不重叠的文件级实现。
- 不适合 subagent 的情况：小任务、共享上下文重、顺序依赖、写入范围重叠、验证缺失、用户要求本地处理。
- 只要任务**可拆成 2 个及以上相对独立的子问题**（例如：代码定位/方案对比/风险审查/测试验证/文档对齐彼此不互相阻塞），就应**默认并行拆 lane**，而不是先串行把所有信息凑齐。
- 可启动时默认开 **3-5** 个 `fork_context=false` subagent（优先覆盖：搜索定位、实现、验证/测试、风险审查；按需裁剪）；当子任务更多且依然互不重叠时，可扩展到 **6-8** 个（不再要求必须 `/team` 或用户显式点名），但仍需满足：写入范围 disjoint、共享上下文不爆炸、主线程能集成收口。
- 优先在同一轮并发启动；只传必要上下文、禁止范围、输出契约和验证要求；发现依赖关系后再把剩余工作回收成串行，不要反过来一开始就全部串行化。
- 写入型 worker 只能改明确 disjoint 的文件或模块，且不得修改共享连续性 artifact。
- 只有用户显式调用 `/team`，或 worker 需要互相协作、共享任务列表、相互质询时，才升级到 team orchestration。
- 默认采用 goal-style 执行循环（plan → implement → verify → repair → closeout）；除非用户要求只给方案，否则不要停在 planning 阶段。

## Closeout

- 收口必须给出证据：测试、命令、diff、截图、生成产物，或明确说明 blocker。
- 程序化完成态记录的软/硬门禁定义见 **个人使用（最小操作面）** 的 `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 分层；本节是所有环境都适用的收口规范。
- 如果没有运行验证，说明原因和剩余风险，不把未验证状态说成完成。
- 若任务仍未完成，必须明确下一步和当前阻塞条件；不要把可继续执行的任务提前结束。
- 发现与当前任务无关的脏工作区改动时只报告，不回滚、不顺手整理。

## Autopilot Goal Mechanism

- 用户显式调用 `/autopilot` 时，默认启用 goal-style 持续执行，不是一次性答复模式。
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
