# 优秀计划实践对标（OpenAI / Gemini / SWE-agent）

本文档对照外部仓库的常见「可执行计划」约定，供本仓库策划与 `skills/plan-mode/SKILL.md` 迭代时参考；不替代仓库根 `AGENTS.md` 的跨宿主真源。

## OpenAI PLANS.md 可借鉴要点

以下是对 [openai-agents-js `PLANS.md`](https://raw.githubusercontent.com/openai/openai-agents-js/main/PLANS.md) 中 **NON-NEGOTIABLE REQUIREMENTS** 的意译摘要（非逐字翻译）：

1. **完全自包含**：当前版本的 ExecPlan 须包含一名完全不了解本仓库的新手端到端成功所需的全部知识与步骤；读者仅有工作区与该计划文件，无「记得上次计划」之类外部记忆。
2. **活文档**：随进展、新发现与设计定稿持续修订计划，且每次修订后仍须保持完全自包含；并强制维护 **`Progress`**、**`Surprises & Discoveries`**、**`Decision Log`**、**`Outcomes & Retrospective`** 等节，使后人仅凭计划即可接续。
3. **可演示的验收**：目标不仅是「满足某条定义」的代码改动，而是可观察、可复现的行为结果；验收用语应像「启动后访问某 URL 得到某 HTTP 状态与正文」这类人类可验证表述，而非仅内部类型/结构名。

与 **`skills/plan-mode/SKILL.md`**（相对仓库根）的对照：OpenAI 强调 ExecPlan 在信息密度上**自给自足、禁止依赖外链补知识**；本仓库 plan-mode 更强调 **Cursor CreatePlan 的 YAML/正文对齐、每条 todo 四元组、末条固定 `/gitx plan` 对照 Git**，并把跨会话与 hook 边界交给 `docs/harness_architecture.md` 与 `AGENTS.md`，而非在单份计划里复制全书式运行时说明——属于 **「计划自包含」与「仓库单一真源」之间的取舍**：可借鉴其「新手可读 + 活文档 + 行为验收」的 bar，但不必把 `AGENTS.md` 级策略整段抄进每一份 `.plan.md`。

## Gemini CLI Plan Mode 对照

Plan Mode 的 **Tool Restrictions** 将允许工具约束为以只读探索为主（文档列举含 `read_file`、`list_directory`、`glob`、`grep_search`、只读 MCP、研究子代理等），对应「实施前先安全调研」的 read-only 面。（[raw `plan-mode.md`](https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/cli/plan-mode.md)）
在正式 Markdown 计划起草前，文档要求先讨论发现并 **reach an informal agreement on the approach**，可用 `ask_user` 呈现选项，且写明 **Gemini CLI will stop and wait for your confirmation** 再继续。
**Planning (Write)** 下 **`write_file`** 与 **`replace`** **only allowed for `.md` files** in **`~/.gemini/tmp/.../plans/`**（上游原文目录占位串略有空格排版）或 **custom plans directory**；其它路径/类型的写入不在该模式默认白名单内。

## SWE-agent trajectories（可选参考）

[SWE-agent 的 `trajectories` 目录](https://github.com/princeton-nlp/SWE-agent/tree/main/trajectories) 存放可审计的 **计划—行动—环境反馈** 轨迹，便于对照「代理实际做了什么、环境如何响应」；若你需要对外复现或事后复盘，可将此类「可回放记录」视为对 Markdown 计划正文的补充证据链，而非替代本仓库的 Git 与 harness 工件约定。

## 计划收口

| Todo id | 状态 | 说明 |
|-----------|------|------|
| `read-openai-plans` | 完成 | 上文「OpenAI PLANS.md 可借鉴要点」已摘录并对比 `skills/plan-mode/SKILL.md`。 |
| `read-gemini-plan-mode` | 完成 | 上文「Gemini CLI Plan Mode 对照」三行对齐上游 `plan-mode.md` 工具限制与流程。 |
| `optional-swe-trajectories` | 完成 | 上文 SWE-agent 段落 + 官方 trajectories 链接。 |
| `gitx-plan-closeout` | 完成 | 本表 + 下方 `git status` 快照；宿主侧请用户在 Cursor 按 `skills/gitx/SKILL.md` 执行 **`/gitx plan`**（与 **`/gitx`** 同契约）；本代理无法代用户触发 slash 命令。 |

**`git status --short --branch`（在 `/Users/joe/Documents/skill` 执行）**：

```text
## cursor/paper-adversarial-skills
 M AGENTS.md
 M docs/README.md
 M docs/harness_architecture.md
 M docs/host_adapter_contract.md
 M docs/references/rfv-loop/reasoning-depth-contract.md
 M docs/rfv_loop_harness.md
 M scripts/router-rs/src/autopilot_goal.rs
 M scripts/router-rs/src/closeout_enforcement.rs
 M scripts/router-rs/src/main_tests.rs
 M scripts/router-rs/src/rfv_loop.rs
 M scripts/router-rs/src/route/routing.rs
 M scripts/router-rs/src/route/scoring.rs
 M scripts/router-rs/src/route/signals.rs
 M scripts/router-rs/src/router_env_flags.rs
 M scripts/router-rs/src/task_state.rs
 M scripts/skill-compiler-rs/src/main.rs
 M skills/SKILL_LOADOUTS.json
 M skills/SKILL_MANIFEST.json
 M skills/SKILL_ROUTING_METADATA.json
 M skills/SKILL_ROUTING_RUNTIME.json
 M skills/citation-management/SKILL.md
 M skills/experiment-reproducibility/SKILL.md
 M skills/paper-reviser/SKILL.md
 M skills/paper-workbench/SKILL.md
 M skills/paper-workbench/references/RESEARCH_PAPER_STACK.md
 M skills/paper-workbench/references/edit-scope-gate.md
 M skills/paper-writing/SKILL.md
 M skills/paper-writing/references/storytelling-patterns.md
 M skills/plan-mode/SKILL.md
 M skills/statistical-analysis/SKILL.md
 M tests/routing_eval_cases.json
?? .cursor/plans/
?? .cursor/rules/cursor-plan-output.mdc
?? docs/plans/REVIEW_plan_review_adoption.md
?? docs/plans/benchmark_plan_practices.md
?? docs/plans/context_token_audit_build_report.md
?? docs/plans/plan_review_adoption.md
?? docs/plans/plan_review_closeout.md
?? docs/plans/plan_review_findings_round1.md
?? docs/plans/plan_todo_checklist.md
?? skills/citation-management/references/integrity-redlines.md
?? skills/experiment-reproducibility/references/research-record-minimum.md
?? skills/paper-writing/references/claim-spine-and-section-contract.md
?? skills/statistical-analysis/references/
```

**复制粘贴注意**：上表为撰写时的快照；若你本地随后有新提交，应以你机器上再次运行同一命令的输出为准。

**Copy vs defer（取舍备忘）**

- **Copy（写入计划/对标文）**：外部 ExecPlan 的「新手可读、活文档四节、行为级验收」等**原则**可吸收进 `docs/plans/` 与本 skill 的叙述，避免团队口头漂移。
- **Defer（不复制进每份计划）**：`AGENTS.md`、`docs/harness_architecture.md`、路由 runtime 等仍保持**单一真源**；计划正文用链接/指针引用，而不是 fork 第二套完整 policy，以免双真源与过期分叉。
