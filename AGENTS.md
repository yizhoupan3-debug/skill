# Codex Agent Policy

## 权威分层

先判断问题属于哪一层，再改对应真源：

| 类别 | 权威落点 |
|------|----------|
| 跨宿主执行协议、语言约束、收口原则 | 仓库根 `AGENTS.md` |
| 连续性 harness 结构、hook 数据流、开关矩阵、为何刻意不兼容 | `docs/harness_architecture.md` |
| skill 热路由入口 | `skills/SKILL_ROUTING_RUNTIME.json` |
| skill 冷元数据 / explain / plugin catalog | `skills/SKILL_ROUTING_METADATA.json`、`skills/SKILL_PLUGIN_CATALOG.json`、`skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json` |
| framework command / host registry | `configs/framework/RUNTIME_REGISTRY.json` |
| 程序化 schema 与校验 | `configs/framework/*.json` + `scripts/router-rs/src/**` |
| hook 实际注入、拦截与投影 | 各宿主 `hooks.json` + `scripts/router-rs/src/*hooks*.rs`、`host_integration.rs` |

## 文档地图

- harness 分层与开关：`docs/harness_architecture.md`
- 宿主接入与投影：`docs/host_adapter_contract.md`
- Rust 运行时契约：`docs/rust_contracts.md`
- plugin / 路由冷元数据契约：`docs/runtime_plugin_contract.md`
- 文档索引与历史边界：`docs/README.md`

## Language

- 所有面向用户的回复必须使用简体中文。
- 代码、路径、命令、第三方原文日志不受此约束。
- 只有用户在当前轮明确要求英文回复时才切换。

## Root

- Codex 全局 skill root 通过 `CODEX_HOME` 解析；未设置时使用 `~/.codex`。
- 仓库内开发态优先使用当前仓库 `skills/` 与 `skills/SKILL_ROUTING_RUNTIME.json`。
- 不把机器绝对路径写成策略真源；跨宿主路径通过仓库根、`CODEX_HOME`、`CURSOR_HOME` 或用户主目录解析。

## Skill Routing

- 第一入口始终是当前生效 skill root 下的 `skills/SKILL_ROUTING_RUNTIME.json`。
- 该文件只承担热路由索引；命中后按 `skill_path` 读取对应 `SKILL.md`。
- 不要用 slug 猜路径；runtime 未命中且确需继续时，再查其声明的 fallback manifest。
- 不要把 explain、plugin catalog、host projection 元数据重新塞回热 runtime。

## Task Intake

- 先抽取对象、动作、约束、交付物、成功标准。
- 先判断 source / artifact / evidence gate，再选最窄 owner；最多叠加一个 overlay。
- 优先最小可验证 delta；不要为了赶进度扩大抽象、复制真源或跳过路由。
- 信息不足时先用本地证据补齐；只有关键选择有不可逆风险时才询问用户。

## Coding First Principles

- 写代码前先明确 `Goal`、`Non-goals`、`Existing owner`、`Minimal delta`、`Validation`。
- 默认做减法：先删无用逻辑，再复用已有能力，再收敛重复入口，最后才考虑新增抽象。
- 不要为“以后可能需要”新增 wrapper、fallback、兼容层、第二真源或额外状态。
- 改动必须落在最小 owner 内；若方案需要跨 owner 扩散，先说明边界和取舍。
- 完成标准是证据，不是叙述：diff、测试、日志、产物或明确 blocker。

## Continuity

- 连续性真源在 `artifacts/current/`；机器可读视图优先 `router-rs framework snapshot` / `contract-summary`。
- `SESSION_SUMMARY`、`NEXT_ACTIONS`、`EVIDENCE_INDEX`、`GOAL_STATE`、`RFV_LOOP_STATE` 属于 L2 真源。
- SessionStart 只允许注入动态活信息，不允许把 repo onboarding、工具清单或静态说明塞回上下文。

## Host Boundaries

- `AGENTS.md` 负责跨宿主不变量；实现细节、开关矩阵、注入路径只在 `docs/harness_architecture.md` 和代码里展开。
- Cursor / Codex / Claude 的 hook 可见文案以宿主实际注入为真源；不要在可见回复中自拟长篇仿机读块。
- 硬门控短码走宿主约定字段；非硬门控提示走 `additional_context` 类字段。具体事件与字段映射见 `docs/harness_architecture.md`。

## Execution Ladder

- 主线程始终负责上下文判断、阻塞项、共享决策、集成与最终验证。
- Codex CLI 及未加载 Cursor rules 的环境：默认主线程本地执行；只有用户显式要求 subagent、delegation、parallel agent work、多 agent、分路、分头、并行，或显式调用 `/autopilot` 时，才进入 bounded sidecar admission。
- review 请求是独立上下文授权：深度 / 全面 / 全仓 / 跨模块 / PR 级 review 必须先启动只读 `fork_context=false` reviewer subagent，再由主线程整合；不得用模型自写拒因跳过，只有用户明确要求不用子代理时除外。**各宿主默认可清点的深度 reviewer lane 分列**见 [`docs/harness_architecture.md`](docs/harness_architecture.md) **§5.0**（勿假设三宿主使用同一 `subagent_type` 字符串）。
- 用户请求 **review / 代码审查**（代码与改动面）且**未被更窄 owner 抢占**（例如纯截图或 UI 视觉证据、手稿/论文主线、仅 GitHub PR review comment 处置作为第一目标）时，**默认**遵循 [`skills/code-review-deep/SKILL.md`](skills/code-review-deep/SKILL.md)：**verdict-first**、**严重程度证据门槛**、从技能内透镜目录 **自选 lane** 并在已选维度内系统化穷尽；并行只读子代理按所选 lens 拆分整合。**勿**在 `AGENTS.md` 维护第二份 lens 清单。
- Cursor 工作区的 review gate / 执行偏好差异由 `.cursor/rules/*.mdc` 补充；这些文件只保留 Cursor 独有硬约束与差异。
- 适合 subagent：高噪音搜索、日志整理、独立风险审查、互不重叠的文件级实现。
- 不适合 subagent：小任务、共享上下文重、顺序依赖、写入范围重叠、验证缺失、用户要求本地处理。
- 若应启用 subagent 却未启用，内部只允许使用这些拒因：`small_task`、`shared_context_heavy`、`write_scope_overlap`、`next_step_blocked`、`verification_missing`、`token_overhead_dominates`。对用户只说业务阻塞，不展开元说明。
- `/autopilot` 进入 goal-style 连续执行：先写最小 goal 契约，再按 plan → implement → verify → repair → closeout 持续推进。

## Closeout

- 收口必须给出证据：测试、命令、diff、截图、生成产物或明确 blocker。
- **分层**：上述证据优先落在工件与记录（如 closeout record、`EVIDENCE_INDEX`、会话摘要文件、测试输出），并满足程序化门禁与诚实要求；**不等于**必须在面向用户的聊天回复里长篇罗列路径、diff 或命令全文。
- **可见收尾口吻**：结束前用几句自然话带过就好——交代清楚这轮做了什么、结果怎样、还有没有悬而未决的、你是想先收工还是需要对方接着做哪一步；少用「条目体」话术，别把路径清单、长 diff、整段命令默认贴进聊天，除非对方点名要或为排错所必需。
- 如果没有运行验证，要说明原因和剩余风险，不把未验证状态说成完成。
- `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 的软硬门禁细节见 `docs/harness_architecture.md`；本文件只保留“必须如实给证据”这个不变量。
- 发现与当前任务无关的脏工作区改动时只报告，不回滚、不顺手整理。

## Git

- 未经用户主动明确要求，不得主动创建 Git 分支或 Git worktree。
- 不要把“保持主线干净”“并行开发”“隔离风险”当作默认创建分支或 worktree 的理由。
- 只允许只读检查现有分支/worktree 状态。
- 若确实需要新分支或 worktree，先停下并询问用户。

## Codex Sync

Codex 侧可能使用编译期嵌入的 `AGENTS.md` 快照。修改本文件后，如需同步 Codex hook 投影，执行：

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"
```
