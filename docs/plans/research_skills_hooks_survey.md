# 科研相关 Skill、Hook 与真源调研笔记

只读调研；未改路由 JSON / hook 行为。证据均来自本仓库已打开文件中的路径与符号。

## 1. Skill 表（科研向 + 手稿邻接 artifact）

| slug | layer | gate | session_start | skill_path | hot（25）vs manifest-only |
|------|-------|------|----------------|------------|---------------------------|
| `citation-management` | L1 | `none` | `preferred` | `skills/citation-management/SKILL.md` | **hot**（`skills/SKILL_ROUTING_RUNTIME.json` `records`） |
| `loop` | L1 | `none` | `preferred` | `skills/loop/SKILL.md` | **hot** |
| `paper-workbench` | L2 | `none` | `preferred` | `skills/paper-workbench/SKILL.md` | **hot** |
| `paper-writing` | L2 | `none` | `preferred` | `skills/paper-writing/SKILL.md` | **hot** |
| `experiment-reproducibility` | L3 | `none` | `preferred` | `skills/experiment-reproducibility/SKILL.md` | **hot** |
| `statistical-analysis` | L4 | `none` | `preferred` | `skills/statistical-analysis/SKILL.md` | **hot** |
| `paper-reviewer` | L2（registry） | `none`（registry） | manifest 行见 `SKILL_MANIFEST.json` | `skills/paper-reviewer/SKILL.md` | **manifest-only（热路由外）**：`SKILL_ROUTING_RUNTIME.json` 的 `scope.hot_skill_count` 为 25，本 slug **不在** `records`；`SKILL_ROUTING_RUNTIME_EXPLAIN.json` 的 `excluded.paper-reviewer` 写明 `explicit_opt_in_or_manifest_fallback` |
| `paper-reviser` | L2（registry） | `none`（registry） | 同上 | `skills/paper-reviser/SKILL.md` | **manifest-only**：同上 `excluded.paper-reviser` |
| `pdf` | L3 | `artifact` | `required` | `skills/pdf/SKILL.md` | **hot**（artifact gate） |
| `doc` | L3 | `artifact` | `required` | `skills/doc/SKILL.md` | **hot** |
| `visual-review` | L3 | `evidence` | `required` | `skills/visual-review/SKILL.md` | **hot** |
| `plan-mode` | L1 | `none` | `preferred` | `skills/plan-mode/SKILL.md` | **hot**（策划闸门；与手稿 workflow 可叠加） |

**manifest fallback 行为证据**：`scripts/router-rs/src/main_tests.rs` 中 `manifest_fallback_plain_paper_reviewer_token_targets_specialist_slug` 对任务 `用 paper-reviewer 逻辑模式审一下 claim evidence` 断言 `selected_skill == "paper-reviewer"`，说明 **显式 token / 别名路径** 可走 manifest 命中专科，而不必在 hot `records` 内。

## 2. 论文栈内部路由（前门 vs `$paper-*` lane）

- **默认用户入口**：`$paper-workbench`；专科 `$paper-reviewer` / `$paper-reviser` / `$paper-writing` 为 **内联 lane**，`disable-model-invocation: true` 语义下 **不与前门并列**（见 `skills/paper-workbench/references/RESEARCH_PAPER_STACK.md` §「宿主与专科入口」）。
- **edit_scope**：改稿前须定 `surgical`（默认）或 `refactor`；`$paper-reviser` 全合同仅在 `refactor` 等授权下展开（`skills/paper-workbench/references/edit-scope-gate.md`）。
- **输出顺序**（经前门转发改稿时）：`edit_scope` → `scope_items`/`non_goals` 或 `refactor_intent`/`risk_note` → Claim card → `tone_audit` 或用语层说明 → 正文/hunks → 账本字段；见 `skills/paper-workbench/SKILL.md` 正文「统一输出顺序」与 `skills/paper-writing/SKILL.md` 的 **Output Defaults** 互引。
- **RFV vs 手稿门控**：`RESEARCH_PAPER_STACK.md` 横切规则写明 **RFV（代码）与 PAPER_GATE（手稿）PASS 语义勿混**。

## 3. router-rs「论文语境」信号与打分

- **`paper_skill_requires_context`**：`slug` 为 `paper-workbench` \| `paper-reviewer` \| `paper-reviser` \| `paper-writing` 时要求论文语境（`scripts/router-rs/src/route/signals.rs`）。
- **`has_paper_context`**：子串/短语集合含 `paper`、`manuscript`、`论文`、`摘要`、`审稿意见`、`reviewer comments`、`rebuttal`、`appendix`、`claim` 等（同文件）。
- **抑制逻辑**：若需上下文但未命中 `has_paper_context`，`score` 置 0，理由 `"Suppressed: paper skills require explicit paper or manuscript context."`（`scripts/router-rs/src/route/scoring.rs`）。
- **PR 与论文分离**：`gh-address-comments` 在「有论文语境但无 GitHub/PR 语境」时抑制，理由含 `paper review or revision requests without explicit GitHub/PR`（同文件）。
- **路由夹具**：`tests/routing_eval_cases.json` 中 `route-009-paper-writing`、`route-011-phd-proposal-paper-workbench`、`route-012-prereg-reproducibility`；另有 `broad-architecture-review-delegation-gate-case` 将 `paper-reviser`/`paper-reviewer` 列入 `forbidden_owners` 以约束「全面 review 架构」类任务不误入手稿专科。

## 4. `REVIEW_GATE` 与「审论文」话术

- **正则真源**：`configs/framework/REVIEW_ROUTING_SIGNALS.json` → 编译期嵌入 `scripts/router-rs/src/review_routing_signals.rs`（`EMBEDDED_JSON` + `compile_review_gate_regexes` 容错说明见 `docs/harness_architecture.md` §8 末段）。
- **Cursor 门控短码**：`scripts/router-rs/src/cursor_hooks.rs` 出现字面 `router-rs REVIEW_GATE`（与 `AGENTS.md` Host Boundaries 一致）；**非** skill 路由状态机。
- **与论文的交叉风险**：`review_gate_regexes` 含 `(?i)(深度|全面|…)\s*review` 等；若用户中英混写 **「深度 review」** 且语义指向手稿，仍可能落入 **代码向 review 门控** 的 regex 集合（与 `paper-workbench` 的「整篇 review」类 **路由** 触发词是不同子系统）。并行 review 启发式 `has_parallel_review_candidate_context` 需 **review 标记 + breadth 标记 + scope 标记** 同时满足（`signals.rs`），纯「帮我审稿意见改稿」更常走论文信号链而非该三元组，但 **边界话术** 仍建议以宿主实际注入的 **`router-rs REVIEW_GATE`** 行为为准，勿与 `skills/SKILL_ROUTING_RUNTIME.json` 命中混谈。

## 5. Hook / 环境变量子集（科研工作流相关）

| 变量 | 默认/语义要点 | 与论文/审稿关系 |
|------|----------------|-----------------|
| `ROUTER_RS_OPERATOR_INJECT` | 默认开；`router_rs_operator_inject_globally_enabled()`（`scripts/router-rs/src/router_env_flags.rs`） | **总闸 off** 时关闭 nudge + AUTOPILOT_DRIVE + RFV_LOOP **及**（若启用）`PAPER_ADVERSARIAL_HOOK`（`docs/harness_architecture.md` §8 表；`AGENTS.md` 个人使用节同述） |
| `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK` | **opt-in**（须 `1`/`true`/`yes`/`on`） | `paper_adversarial_hook.rs`：`cursor_paper_adversarial_hook_requested()` = 总闸 **且** `router_rs_env_enabled_default_false(ENV_HOOK)`；`beforeSubmit` 合并前缀行 `**PAPER_ADVERSARIAL_HOOK**` |
| `ROUTER_RS_HARNESS_OPERATOR_NUDGES` | 关断仅影响 `HARNESS_OPERATOR_NUDGES.json` 注入 | 与 `PAPER_ADVERSARIAL_HOOK` 独立（§8 表） |
| `ROUTER_RS_AUTOPILOT_DRIVE_HOOK` / `ROUTER_RS_RFV_LOOP_HOOK` | 控制 Stop 续跑块 | 论文 workflow 可与 goal/RFV 并行，但语义层手稿门控仍见 `RESEARCH_PAPER_STACK` RFV vs PAPER_GATE |
| `retired beforeSubmit AUTOPILOT_DRIVE opt-in` / `retired beforeSubmit RFV opt-in` | opt-in | 仅 Cursor `beforeSubmit` 合并对应续跑 |
| `retired silent-mode branch` | 整段剥离例外 | 含 `REVIEW_GATE` / `PAPER_ADVERSARIAL_HOOK` 等字样的 followup **保留**（`docs/harness_architecture.md` §8） |
| `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` | 短路 review 门控 | 注释写明续跑仍合并（同表） |

**文案真源**：`configs/framework/PAPER_ADVERSARIAL_HOOK.txt`；缺失时 `include_str!` 内置同文（`paper_adversarial_hook.rs` 注释）。

## 6. 真源索引（速查）

| 关注点 | 文件 |
|--------|------|
| 热路由 + fallback 声明 | `skills/SKILL_ROUTING_RUNTIME.json`（`scope.fallback_manifest`、`hot_skill_count`） |
| 全量 skill 行 | `skills/SKILL_MANIFEST.json`、`skills/SKILL_ROUTING_REGISTRY.md` |
| hot 外 slug 解释 | `skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json` → `excluded` |
| Review regex / 并行候选 markers | `configs/framework/REVIEW_ROUTING_SIGNALS.json`、`review_routing_signals.rs` |
| 论文语境与意图切片 | `scripts/router-rs/src/route/signals.rs` |
| 候选打分与抑制 | `scripts/router-rs/src/route/scoring.rs` |
| 路由回归夹具 | `tests/routing_eval_cases.json`、`main_tests.rs` |
| Cursor hook 合并点 | `scripts/router-rs/src/cursor_hooks.rs`（`maybe_merge_paper_adversarial_before_submit` 调用） |
| 五层 harness + 开关矩阵 | `docs/harness_architecture.md` §1–2.1、§8 |
| 宿主安装与手稿栈 | `docs/host_adapter_contract.md`、`configs/framework/RUNTIME_REGISTRY.json` |
| 跨工作区 Cursor hooks 模板 | `configs/framework/cursor-hooks.workspace-template.json`（`_doc` 写明 symlink `configs/` 与 JSON/txt 同轨） |

## 7. 混淆与风险（≤1 页）

1. **skill 命中 ≠ `REVIEW_GATE`**：前者由 `SKILL_ROUTING_RUNTIME.json` + `route/*`；后者为 Cursor Stop/beforeSubmit 路径上 **子代理证据链** 门控（`cursor_hooks.rs` / `.cursor/hook-state`），短码 **`router-rs REVIEW_GATE`**。
2. **`paper-reviewer` / `paper-reviser` 不在 hot 25**：日常首轮路由更常落到 `paper-workbench` / `paper-writing` 等 hot owner；**显式** `paper-reviewer` 字样或 manifest fallback 才稳定专科（见 `main_tests.rs`）。
3. **`ROUTER_RS_OPERATOR_INJECT=0`**：一键关闭多类注入含 **对抗论文 hook**；调试时易误判为「单关 PAPER 变量无效」。
4. **未 symlink `configs/framework`**：hook 仍可能跑，但 **改 `PAPER_ADVERSARIAL_HOOK.txt` / JSON 不一定** 与框架仓库磁盘同轨；`README.md`（`--with-configs`）与 `cursor-hooks.workspace-template.json` `_doc` 一致叙述。
5. **`review` 一词多义**：PR/code review 路由与手稿「审稿」在英文 `review` 上共享表面形式；`scoring.rs` 对 `gh-address-comments` 的抑制与 `REVIEW_ROUTING_SIGNALS.json` 的 breadth/scope 组合用于降低误触，但无法数学上消除所有歧义。
6. **`loop` skill vs `RFV_LOOP_STATE`**：`skills/loop/SKILL.md` 写明长轮次以 **`framework_rfv_loop`** 写磁盘状态为主，**不在热 skill 路由** 的等价路径上混用旧 Cursor 文件名。

## 8. 相邻 skill 与叠加矩阵（简）

| 场景 | 主 owner | 常见叠加 |
|------|----------|----------|
| 手稿总控 / 可投性 | `paper-workbench` | `citation-management`（`.bib`/DOI）；`statistical-analysis`（方法/功效）；`experiment-reproducibility`（预注册/偏离）；成稿版式 `pdf`/`visual-review` |
| 仅引用格式 | `citation-management` | 手稿叙事仍归 `paper-workbench`（`citation-management/SKILL.md` When to Use） |
| 统计深度 | `statistical-analysis` | `$paper-reviewer` 逻辑模式互指（`statistical-analysis/SKILL.md`） |
| 可复现流程 | `experiment-reproducibility` | 与 `RESEARCH_PAPER_STACK.md` 科研纪录章节互链 |
| 多轮对抗审查（代码/结构化） | `loop` / `rfv_loop` harness | 与手稿 **PAPER_GATE** 协议分列（见 stack 横切） |
| 推理深度 rollup / 硬门禁 | `reasoning-depth-contract.md` | `completion_gates` / `close_gates` 默认 off，与 digest 提示分工 |

## 9. 可选后续立项

- **手稿语境 vs `REVIEW_GATE`**：可选 opt-in 环境变量 **`ROUTER_RS_REVIEW_GATE_SUPPRESS_ON_MANUSCRIPT_CONTEXT`**（默认 off）在 `hook_common::is_review_prompt` 中复用路由侧 `has_paper_context`；详见 [`docs/harness_architecture.md`](../harness_architecture.md) §8 表与 `scripts/router-rs/src/hook_common.rs`。开启前需接受「纯英文手稿用语且无意图锚点」仍可能走代码向 regex 的残余歧义。
- 将 `paper-reviewer`/`paper-reviser` 是否纳入 hot 的 **路由命中率** 做离线统计（仅观测，不改默认 25 热集前需评审）。
- **Symlink / 跨工作区操作索引（已落地）**：不必重复写长步骤——见 [`docs/host_adapter_contract.md`](../host_adapter_contract.md) **§0** 末行「跨 Cursor 工作区接入」指针；操作核对清单为 [`cursor_cross_workspace_operator_checklist.md`](cursor_cross_workspace_operator_checklist.md)；模板 `_doc` 与 `configs/` 同轨说明见 [`configs/framework/cursor-hooks.workspace-template.json`](../../configs/framework/cursor-hooks.workspace-template.json) 顶部 `_doc`。

## 10. 验证与 Git 收口

**已执行测试**：

```bash
cargo test --manifest-path /Users/joe/Documents/skill/scripts/router-rs/Cargo.toml routing_eval -- --nocapture
cargo test --manifest-path /Users/joe/Documents/skill/scripts/router-rs/Cargo.toml manifest_fallback_plain_paper -- --nocapture
```

结果：**均通过**（各 2 与 1 个测试匹配子串，其余 filtered）。

**工作区**：调研时 `git status` 显示大量与本调研无关的已修改文件（用户分支 `cursor/paper-adversarial-skills`）；本交付 **仅新增** `docs/plans/research_skills_hooks_survey.md`。

**宿主收口**：若用户将本文件纳入提交，请在 **Cursor 宿主** 执行 **`/gitx plan`** 对照计划验收（与 `AGENTS.md` / plan-mode 契约一致）；本子代理未执行该宿主命令。
