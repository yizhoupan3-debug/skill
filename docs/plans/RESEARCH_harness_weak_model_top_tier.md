# RESEARCH：Harness 内外核查 — 弱模型体验、「顶级」判据与合成交付

> 执行日期：2026-05-12。对齐 Cursor Plan「Harness 全面核查调研计划（内外部）」；**未修改** 附带的 `.plan.md` 文件。本文件为 **调研期** 合成交付（`plan_profile: research` 等价语义）；实现面改动见另开 execution 计划。

---

## 1. Executive verdict（两问直答）

| 问题 | 结论（可证伪） |
|------|----------------|
| **是否「顶级」？** | 在 **「可审计的软件 harness：磁盘状态机 + exit code 证据链 + 多宿主 hook + opt-in 硬门禁」** 这一细分赛道，本仓库处于 **第一梯队工程实现**（与纯 IDE 规则栈相比多一层 L3/L2 真源与测试覆盖）。若把「顶级」定义为 **「替代更大参数模型在开放域推理上的上限」**，则 **不适用** — 本架构在文档中已明确深度来自 **分工 + verify + 落盘**，而非单模型 CoT（见 `docs/references/rfv-loop/reasoning-depth-contract.md` 原则段）。 |
| **弱模型能否切实增强到强模型体验？** | **部分成立**：在 **可靠性、防偷懒完成叙事、长任务切片续跑、强制验证与证据索引** 上，harness 可把弱模型的产出质量 **向「有纪律的强模型工作流」拉近**；在 **单次上下文内复杂推理、稳定长链工具调用、细粒度数学证明** 等仍受 **基座能力天花板** 约束，harness **不能**等价替换强模型，除非叠加 **独立强 verifier / 子代理**（与 `AGENTS.md` Execution Ladder 一致）。 |

---

## 2. 继承基线摘录（≥8 条，带锚点）

来源：`docs/plans/RESEARCH_harness_depth_longrun_math.md`（下称 **DEPTH**）、`docs/plans/context_token_audit_deep_dive.md`（下称 **TOKEN**）。

| # | 主题 | 锚点 |
|---|------|------|
| 1 | Open #1 `close_gates` vs `max_rounds`：**已在契约与实现对齐（Option B）**；DEPTH 正文 Open 表仍写「矛盾」属**历史措辞**，当前 `reasoning-depth-contract.md` L30 与 `rfv_loop.rs` rustdoc L365–366 一致。 | `docs/references/rfv-loop/reasoning-depth-contract.md` §可程序化硬门禁 RFV 行；`scripts/router-rs/src/rfv_loop.rs` `RfvCloseGates` / `append_round_close_gates_enforced_on_max_rounds_cap` |
| 2 | PostTool 数理子串扩展：**已做**；残余为长尾 `python path/verify.py` 无关键字则可能漏记。 | DEPTH §3.2 |
| 3 | digest「深度信号」与 `ROUTER_RS_HARNESS_OPERATOR_NUDGES` **闸断不对称**：关 nudge 仍见 rollup 行；产品 Breaking 另议。 | DEPTH §1 双真源 #3；`docs/harness_architecture.md` §8 表脚注 |
| 4 | Codex SessionStart **640**（可调 `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` clamp 256–8192）截断；**优先级固化测试仍推迟（Open #6）**。 | DEPTH Open #6；`scripts/router-rs/src/codex_hooks.rs` `truncate_codex_additional_context` / `codex_additional_context_max_chars` |
| 5 | Cursor `merge_additional_context`：**无总字符 cap**，长会话理论上线性增长；依赖 SILENT、段落 strip、前缀替换。 | TOKEN §2 表；`scripts/router-rs/src/cursor_hooks.rs` `merge_additional_context` |
| 6 | digest `max_lines` 经 `clamp(2,4)` 与 Codex `build_framework_continuity_digest_prompt(..., 4)` 共限。 | TOKEN §1；`scripts/router-rs/src/framework_runtime/continuity_digest.rs` `capped_max_lines` |
| 7 | 多宿主 PostTool 与 `cross_link` 时间戳公平性：**仍 Open #7**（`no_evidence_window` 审计标签偏多风险）。 | DEPTH §6 |
| 8 | `external_research_strict` 迁移显眼度、R9 closeout-depth 对齐推迟、contradiction_sweep 单列计数推迟等。 | DEPTH Open #4–#5、#8；`reasoning-depth-contract.md` R9 段 |
| 9 | EVIDENCE 条数上限 **120**、command 预览 **2000** UTF-8 字符截断。 | TOKEN §1 缺口表 → `framework_runtime/mod.rs` 常量（TOKEN 已引） |

---

## 3. 弱上下文模型专段（内部代码对照结论）

**失效模式（弱模型更痛）**

1. **Codex**：合并后整段 `additionalContext` 被 **硬截断**（换行优先 + `...`），弱模型可能 **读不到** 续跑/Goal 尾部 → 丢步骤或重复问状态。证据：`codex_hooks.rs` `truncate_codex_additional_context`。
2. **Cursor**：**无合并总 cap**，弱模型反受 **噪声膨胀** 影响（长会话多路径 `merge_additional_context`）。证据：`cursor_hooks.rs` L1435–1446。
3. **闸断认知负荷**：`ROUTER_RS_OPERATOR_INJECT=0` 关续跑，但 digest 仍可有 **深度 rollup 行**（与 harness §8 一致），弱模型易 **误判「已关深度」**。证据：`docs/harness_architecture.md` §8 脚注。
4. **verbose 续跑**：`ROUTER_RS_GOAL_PROMPT_VERBOSE` 放大 Goal/RFV followup 与 digest Goal 段；弱模型 token 预算更紧。证据：TOKEN §1 表。
5. **误用 Tier0**：若 agent `read_file` 整份 `EVIDENCE_INDEX`，与 hook「摘要注入」设计相悖，弱模型更易上下文爆炸。证据：TOKEN §1「账本 → 门控读盘」脚注。

**≥5 条可操作缓解（配置/流程，非本调研代码改动）**

1. Codex：按需调高 `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX`（仍受 clamp），并接受 **线性成本**。
2. 关冗长注入：`ROUTER_RS_GOAL_PROMPT_VERBOSE=0`（默认紧凑）、关 `ROUTER_RS_OPERATOR_INJECT` 或分项关 RFV/AUTOPILOT drive。
3. Cursor 长会话：启用 `ROUTER_RS_CURSOR_HOOK_SILENT=1` 前确认 **不**误剥 `REVIEW_GATE` / `AG_FOLLOWUP` 等（见 harness §8 例外）。
4. 验证命令：避免「隐形」脚本路径；对长尾 verify 使用 **显式** `hook-evidence-append` 或命令串包含已覆盖子串（DEPTH §3.2）。
5. 弱模型主线程：缩小 **单轮目标**、多写 `GOAL_STATE` checkpoint，依赖 **地平线切片** 而非单轮吞上下文（`AGENTS.md` Autopilot 宏任务段）。

**本段相对 TOKEN/DEPTH 的新增代码核对**：已读 `merge_additional_context`、`truncate_codex_additional_context`、`continuity_digest.rs` `max_lines.clamp(2, 4)`；结论与 TOKEN **一致**，无「仅文档已知」反转。

---

## 4. 契约 ↔ 实现一致表（≤15 行）

| 契约陈述（`reasoning-depth-contract.md`） | Rust 真源 | 与 DEPTH 执行决议 |
|------------------------------------------|-----------|-------------------|
| `completion_gates` 在 `complete` 路径校验，失败不写 `completed` | `autopilot_goal.rs` + `task_state::validate_goal_completion_gates` | 一致；单测 `goal_complete_*` |
| 字段 `enabled` / `min_depth_score` / `require_successful_evidence_row` / `min_goal_checkpoints` / `block_on_rfv_pass_without_evidence` | `GoalCompletionGates` + `parse_goal_completion_gates` | 与契约表一致 |
| `close_gates` 预览态 + 显式 close | `rfv_loop.rs` `enforce_rfv_close_gates` | 一致 |
| `max_rounds` 耗尽自动 `closed` 亦跑 `close_gates` | `rfv_loop.rs` 双路径调用 + 单测 `append_round_close_gates_enforced_on_max_rounds_cap` | DEPTH Option B **已落地**；Open 表 #1 措辞滞后 |
| `close_gates` 字段含 `require_external_research_object_when_strict_on_close` | `RfvCloseGates` | 与契约一致 |
| `DepthCompliance` 由 `depth_compliance_aggregate` rollup | `task_state.rs` | 与契约「同一条聚合链」一致 |
| R9 closeout-depth 推迟 | `closeout_enforcement.rs` 注释（未再读全文） | 与契约 R9 段一致 |

---

## 5. 外部 Bibliography（≥4；含「对本 harness 的启示」）

| # | 来源（URL） | 3–5 句摘要 | 对本 harness 的启示 |
|---|-------------|------------|----------------------|
| 1 | https://arxiv.org/abs/2406.06592（Lightman 等，**数学推理上的自动化过程监督**） | 用过程级（步级）信号改进推理；强调 **中间步骤可检验** 而非仅最终答案。 | 与 **RFV `append_round` + verify_result** 同哲学；本 harness 用 **shell exit code + EVIDENCE** 替代 PRM 训练回路，属 **运行时过程监督** 工程化。 |
| 2 | https://arxiv.org/abs/2408.03314（**Scaling test-time compute** 类工作） | 推理时可分配额外算力（采样/搜索/验证）；小模型 + 推理时策略可追赶更大参数。 | 对齐 **「弱 + 多轮 + verify」** 叙事；本仓库的 **多轮 RFV/Goal + 硬门禁 opt-in** 属于 **结构化 test-time 资源** 的一种，但 **不**包含 best-of-N 全空间搜索。 |
| 3 | https://arxiv.org/abs/2510.08049（**Process Reward Models 综述**） | 系统整理从 outcome 到 process supervision 的谱系与开放问题。 | 便于对外对标 **「步级信用分配」**；本 harness 的 **缺口** 在于未训练专用 PRM，而依赖 **显式命令与账本 schema**。 |
| 4 | https://arxiv.org/abs/2305.11738（**CRITIC**，工具交互修正） | 模型生成 → 工具验证 → 修正循环。 | 与 **PostTool → EVIDENCE_INDEX**、RFV verify 阶段 **同源不同构**；本仓库更窄、更可审计，工具多样性靠用户/技能定义而非论文内建集。 |

**Verify（计划要求）**：`curl -sI https://arxiv.org/abs/2406.06592` → HTTP/2 200（已在本机执行）。

---

## 6. 能力矩阵（≥6 行 × 3 列：维度 | 内部证据 | 外部对标 / 缺口）

| 维度 | 内部证据（路径级） | 外部对标 / 缺口 |
|------|-------------------|-----------------|
| **可审计深度** | `reasoning-depth-contract.md`；`EVIDENCE_INDEX` + `append_round`；`task_state::DepthCompliance` | vs PRM/OmegaPRM：无学习式步级奖励，**强在可执行审计** |
| **长时运行** | `RFV_LOOP_STATE`、`max_rounds`、`GOAL_STATE` checkpoint；DEPTH 矩阵「无周级调度器」 | vs Voyager：用 **git + artifacts** 非环境模拟器 |
| **数理 / STEM** | `math-reasoning-harness.md`；PostTool 启发式扩展；DEPTH「裸 python 漏记」 | vs LeanDojo：**不**做 ATP；鼓励外部 checker |
| **Token / 上下文** | `codex_hooks` 截断；`cursor_hooks` 无总 cap；TOKEN 全文 | vs 产品内 token 计数：**本仓库未内置**宿主 token 计（TOKEN Non-goals） |
| **多宿主公平** | DEPTH §6 PostTool 形状；`cross_link` 时间戳敏感 | Open #7：**部分风险** |
| **硬门禁** | `completion_gates` / `close_gates` opt-in；默认 advisory | 与工业界「默认软、关键路径硬」相近 |

---

## 7. P0 / P1 缺口自检（计划要求）

| 级别 | 项 | 说明 |
|------|-----|------|
| **P0（本调研不新增；继承已知）** | Cursor 上下文 **无总 cap** | 极端长会话下与弱模型叠加 → **可靠性风险**；缓解见 §3。 |
| **P1** | Open #6 SessionStart 截断 **优先级无固化测试** | 回归盲区；弱模型 + Codex 小预算下更明显。 |
| **P1** | Open #7 多宿主 PostTool → `cross_link` 公平性 | 审计标签噪声；不阻断 append 但增监督成本。 |
| **P1** | nudge 与 digest 深度行 **闸断不对称** | 文档依赖；易误配置。 |

无「本调研新发现的未知 P0」：主要风险已在 TOKEN/DEPTH 登记。

---

## 8. 非目标与未测假设

**非目标（本执行遵守）**

- 未改 `scripts/router-rs`、hooks、schema、测试、CI、锁文件。
- 未做宿主侧 A/B 或用户Study；**无**弱模型 vs 强模型 **同任务对照实验数据**。

**未测假设**

- 不同弱模型在 **相同 harness 配置** 下的 **任务完成率曲线** 未测。
- `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` 调高后对 **Codex 首包延迟/配额** 的影响未测。

---

## 9. 计划 todos 执行对照（自检）

| Plan todo id | 状态 | 证据 |
|---------------|------|------|
| inherit-baseline | Done | 本文件 §2 |
| internal-weak-context | Done | §3 + `codex_hooks.rs` / `cursor_hooks.rs` / `continuity_digest.rs` |
| internal-depth-gates | Done | §4 |
| external-scaffold | Done | §5 + `curl -sI` |
| synthesis-verdict | Done | §1、§6、§7 |
| research-closeout | 见下节 | `git status --porcelain` |

---

## 10. Git 收口说明（research-closeout）

本调研 **新增** 本 tracked 路径：`docs/plans/RESEARCH_harness_weak_model_top_tier.md`。若工作区在调研前已有其它未提交改动，**`git status --porcelain` 不会仅含 `.plan.md`** — 以执行时仓库真实状态为准；本文件不声称清空既有脏文件。

**建议只读验证命令**（与 DEPTH §10 风格一致）：

```bash
rg -n "completion_gates|close_gates|DepthCompliance" scripts/router-rs/src/autopilot_goal.rs scripts/router-rs/src/rfv_loop.rs scripts/router-rs/src/task_state.rs | head -n 30
rg -n "truncate|merge_additional_context|SessionStart|max_lines" scripts/router-rs/src/codex_hooks.rs scripts/router-rs/src/cursor_hooks.rs scripts/router-rs/src/framework_runtime/continuity_digest.rs | head -n 40
```

---

## 11. 生态位（轻量，1 段）

通用 **IDE 规则 + 无状态 system prompt** 解决「写作风格与检查清单」；本仓库 **router-rs + `artifacts/current` + hook 相位** 额外解决 **「跨轮状态、证据记账、可选硬门禁、多宿主对齐」**。差距不在「会不会写漂亮话」，而在 **是否把完成定义绑到 L1 exit code 与 L2 账本** — 这正是弱模型可借力的 **外部脚手架**。
