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
- 允许内容：active task 连续性摘要、`GOAL_STATE` 单行头、`RFV_LOOP_STATE` 单行头、必要时的一行 repo 指针。
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

## 5. 开关面

只保留真正改变行为边界的少量开关；文案分叉和投影位置分叉不再保留。

| 环境变量 | 默认 | 作用 |
|---------|------|------|
| `ROUTER_RS_OPERATOR_INJECT` | 开 | 总闸：关闭 advisory 注入；不影响硬门控短码 |
| `ROUTER_RS_HARNESS_OPERATOR_NUDGES` | 开 | 仅关闭 operator nudge 文案；不改 gate 逻辑 |
| `ROUTER_RS_AUTOPILOT_DRIVE_HOOK` | 开 | 关闭 Stop 等必要事件上的 `AUTOPILOT_DRIVE` advisory |
| `ROUTER_RS_RFV_LOOP_HOOK` | 开 | 关闭必要事件上的 `RFV_LOOP_CONTINUE` advisory |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED` | 关 | 显式开启 Cursor `/autopilot` pre-goal beforeSubmit 提示 |
| `ROUTER_RS_REVIEW_GATE_SUPPRESS_ON_MANUSCRIPT_CONTEXT` | 关 | 手稿语境下抑制 review gate 误触发 |
| `ROUTER_RS_CLOSEOUT_ENFORCEMENT` | 本地软、CI 硬 | 控制 closeout record 是否程序化硬门禁 |
| `ROUTER_RS_DEPTH_SCORE_MODE` | `legacy` | `strict` 时启用更严格 depth 第三分公式 |
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` | 640，clamp 256–8192 | 仅 Codex SessionStart 的字符预算 |

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
| L2 continuity | `artifacts/current/`、`configs/framework/*SCHEMA*` |
| Skill 热路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| Skill 冷元数据 | `skills/SKILL_PLUGIN_CATALOG.json`、`skills/SKILL_ROUTING_METADATA.json`、`skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json` |
| Host registry | `configs/framework/RUNTIME_REGISTRY.json` |

## 9. 刻意不做的事

- 不在 SessionStart 注入 repo onboarding。
- 不保留旧 runtime shape 兼容层。
- 不在 `AGENTS.md`、Cursor rules、docs、hook 文案里重复展开同一套长叙事。
- 不为了“也许以后需要”保留 verbose 模式、双通道切换或多事件重复续跑注入。
