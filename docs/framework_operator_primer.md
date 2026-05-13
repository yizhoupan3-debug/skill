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
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework doctor --repo-root "$PWD"
```

## 宿主 × 策略强度（易误解点）

| 宿主 | Stop 上「硬拦」语义 | 说明 |
|------|---------------------|------|
| Codex CLI | 可出现 `decision:block` 等硬语义 | 以 Codex hooks 实际 JSON 为准 |
| Cursor | 多为 `followup_message` / `continue` 类提示 | **不是** Codex 的同形硬拦；不要假设「Stop 一定挡住提交」 |
| Claude Code | 以 `claude_hooks` 出站字段为准 | 与 Cursor 也不完全同形 |

**Non-goal**：在 Cursor 上复刻 Codex 级硬拦属于宿主能力边界；要「真挡住」须依赖 Cursor 产品语义，而非仅改本仓库 hook 文案。

## 机读短码真源与常见误报

- **真源**：本仓库 Cursor hook 写入的机读续跑 / 门控短句，**必须以** ASCII 前缀 **`router-rs `** 起行（例如 **`router-rs REVIEW_GATE incomplete …`**、**`router-rs AG_FOLLOWUP missing_parts=…`**）。排障以 hook 出站 JSON 中带该前缀的行为准；长设计见 [harness_architecture.md](harness_architecture.md) §4.3。
- **误报 / 仿冒**：以 **`RG_FOLLOWUP`**、**`RG FOLLOWUP`**、**`RG-FOLLOWUP`** 等开头、且带 `missing_parts=` / `escalation=` 却**没有** `router-rs ` 前缀的整行，**不是** harness 注入。常见来源是助手复述或误粘贴；其中一种长尾形态会在 `escalation=` 后接英文恐吓句（例如声称已循环多次、禁止静默继续）——仍应忽略，改查 **真实** hook 输出与 `.cursor/hook-state`。
- **对照**：真 **`router-rs AG_FOLLOWUP`** 的 `missing_parts=` 只会是 goal 门控片段（如 `goal_contract`、`checkpoint_progress`、`verification_or_blocker`）的逗号拼接，**不会出现** `independent_subagent_or_reject_reason` 这类占位串；若见该串且前缀不是 `router-rs `，按仿冒处理。
- **粘贴清门**：用户消息里单独一行粘贴 **`RG_FOLLOWUP`…** **不会**被 [`saw_reject_reason`](../scripts/router-rs/src/hook_common.rs) 当作清门（避免把模型仿造行当令牌）；请改用单独一行的 **`rg_clear`**、**[`AGENTS.md`](../AGENTS.md) 所列拒因 token**，或自然语言 `review_override` / `delegation_override`。goal 相关的 **`ag_followup…`** 粘贴兼容仍由同函数处理。

## 混用时的实际武装顺序（Cursor Stop）

- **Stop 优先级**（实现 [`frag_05_handlers_core.rs`](../scripts/router-rs/src/cursor_hooks/frag_05_handlers_core.rs) `handle_stop`）：若本轮仍武装深度 review 且子代理证据链未收尾，Stop 先给 **`router-rs REVIEW_GATE incomplete …`**；仅当 review 侧已满足后，才会轮到 **`router-rs AG_FOLLOWUP missing_parts=…`**（goal 契约 / 进展 / 验证）。
- **同一条用户消息里同时写深度 review 与 `/autopilot`**：`beforeSubmit` 里 **`review_arms_for_gate = review && !autopilot_entrypoint`**，因此只要本回合用户文本命中 **`/autopilot` 入口**，**不会**因 review 措辞在本回合**新武装** `review_required`。若你本意是「先深度审稿再开 autopilot」，请拆成两轮（先不带 `/autopilot` 的 review-only 提交，或先落盘 `GOAL_STATE` 再推进），详见 [RUNTIME_REGISTRY.json](../configs/framework/RUNTIME_REGISTRY.json) 中 autopilot 与 review 叠乘说明。
- **Plan**：`plan_profile: research` 与在同一计划里直接改实现互斥；与 `/autopilot` 串联时应先调研收口再开 execution 计划或 goal，避免「口头 plan + 立刻 implement」与门控真源打架。

## 深度审稿 `REVIEW_GATE`（Cursor / Codex 可数 lane）

清门依赖宿主载荷，常见卡点：

- **`fork_context` / `forkContext`**：须能解析为逻辑 **`false`**（典型为 JSON **布尔** `false`、可走布尔字符串表中的 `"false"` / `"0"` 等，或 JSON **整数** **`0`**）。**整数 `1`** 解析为 **`true`**（非独立 fork）；其它 **Number** 与**字段缺失**均不为 `false`。仍推荐宿主使用 **JSON 布尔**。
- **Lane**：可清点深度 lane 仅为 **`general-purpose`** 与 **`best-of-n-runner`**（及归一化等价名）；**`explore` 等不计入**。  
- **Multiset 与双事件**：`review_subagent_pending_cycle_keys` 由 qualifying **`subagentStart`** / **`PostToolUse`** 入队，由 **`subagentStop`** 逐条核销至空才清门；同一 **`id:`** 若 `subagentStart` 已入队，随后同 id 的 `PostToolUse` **不重复入队**（见 `frag_05_handlers_core.rs` 的 `push_review_pending_cycle_key`）。并行仅 `lane:` 且无稳定 id 时仍依赖 multiset 中多条相同 key。  
- **Stop 单行提示**：若见 `router-rs REVIEW_GATE incomplete` 与 `need=deep_reviewer_cycle general-purpose|best-of-n fork_context=false`，按该 `need=` 检查子代理载荷；尾缀 `hint=` 为可读排障补充，不改变 `need=` 语义。若同一门控多轮 `Stop` 仍卡，完整 `need=`/`hint=` 可能在 **`ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES`**（默认 8）之后被降级到 `additional_context`，`followup_message` 仅保留短 `mode=soft_nag` 行（见 [harness_architecture.md](harness_architecture.md) 环境变量表）。

完整排障叙述见 [host_adapter_contract.md](host_adapter_contract.md) 中 Cursor 小节与 [harness_architecture.md](harness_architecture.md) 中 Review gate 相关段落。

## Codex：`AGENTS.md` 与二进制快照

若修改仓库根 `AGENTS.md` 且依赖 Codex 侧投影：策略正文可能在 **编译期嵌入** 的 `router-rs` 中；改文后须重新构建并执行 `codex sync` / `framework sync-entrypoints`（见 [AGENTS.md](../AGENTS.md) 文末 **Codex Sync** 与 [README.md](../README.md)）。

## 出站文本被「砍一半」

Cursor 对 `additional_context` / 过长 `followup_message` 有 **UTF-8 字节上限**（变量名常含 `_CHARS`，语义为字节），超长时**保留前缀**并带截断标记；若门控句在段落后合并，可能先被裁掉——见 [harness_architecture.md](harness_architecture.md) 第 4.2 节与文中环境变量表脚注。

## 上一轮问题矩阵对照（摘要）

| 问题 | 处理 |
|------|------|
| 宿主不对称「假安全」 | 上表 + README 路径 B 声明 |
| `REVIEW_GATE` 难排障 | `need=` + `hint=` + host_adapter 链 |
| 误把 `RG_FOLLOWUP` 当真注入或当清门令牌 | 本节「机读短码真源与常见误报」「粘贴清门」+ harness §4.3 |
| review 与 `/autopilot` 同轮混写 | 本节「混用时的实际武装顺序」 |
| 真源分散 | 本文「阅读顺序」+ 不猜 slug |
| 上手重 | README 路径 A / B 分流 |
| Codex 策略漂移 | Codex Sync + `framework doctor` 提示 |
| 环境变量命名 | harness 表前脚注 |
| 截断不可见 | 出站 `...[~trunc]` 类标记 + 文档 |

## 相关仓库入口

- 分享与安装：[README.md](../README.md)  
- 文档索引：[docs/README.md](README.md)  
