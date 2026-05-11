# 「写 plan」能力：内外部调研合成（执行交付）

本文档落实调研计划《Plan能力调研加强》中的可验收项；**不**修改 `.cursor/plans/` 下该计划文件本身。

## 1. 调研范围与架构（与计划 §1 对齐）

本仓库「写 plan」= Cursor Plan Mode / CreatePlan + [`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md) + [`.cursor/rules/cursor-plan-output.mdc`](../../.cursor/rules/cursor-plan-output.mdc) + 可选 `router-rs` 中 `retired Plan-Build goal gate` 与 `cursor_hooks` 检测。

## 1b. 与仓库调研 / review 能力联动（补档）

本合成稿初版以「契约摘录 + 宿主 URL」为主，未把 **可执行工作流** 写成与仓库技能的一一指针；以下与 [`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md) **调研范围（Research scope）与能力联动** 小节对齐，供 `plan_profile: research` / `execution` 起草时对照。

- **分层 Workflow**：默认小中型任务用轻量五行证据 + 可验收 todo；跨模块/高风险/用户要求时升级 audit plan。review plan 仅在用户明确要求、深度 review 或高风险审计时触发；是否启用 subagent 仍受 `AGENTS.md` 执行梯子约束。
- **Git 计划收口**：`execution` 末条以计划 vs 实际 + Git 状态证据为硬要求；宿主支持时可使用 [`skills/gitx/SKILL.md`](../../skills/gitx/SKILL.md) 的 **`/gitx plan`**（与 **`/gitx`** 同契约）。closeout 与 **`git diff --stat`** 习惯见同 skill **强例**及 [`plan_review_findings_round1.md`](plan_review_findings_round1.md)。
- **深度代码审**：对抗式 / 整 PR 级 review 路由 [`skills/code-review-deep/SKILL.md`](../../skills/code-review-deep/SKILL.md)（verdict、P0–P2 符号锚点与本仓库 **强例**一致）。
- **审 plan 样例 findings**：[`plan_review_findings_round1.md`](plan_review_findings_round1.md) 演示本地主线程模拟独立视角对 execution plan 的只读 findings 形态；未实际启用 subagent。
- **research→execution 第一性与继承面**（减法式交接、外部准入上限）：[`RESEARCH_plan_execution_handoff_first_principles.md`](RESEARCH_plan_execution_handoff_first_principles.md)；执行侧模板见 [`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md) **执行计划继承面（research→execution）**。

## 2. 内部：契约硬条款、hook 默认、审 plan 弱项（int-audit）

### 2.1 契约硬条款（真源摘录）

| 条款 | 要点 | 真源 |
|------|------|------|
| CreatePlan 后自检 | 每条 `todos[].content` 同条内四元组；`overview` + 末条依 `plan_profile`；正文 checkbox 与 YAML 对齐；不合规则**直接编辑** `.plan.md` | [`.cursor/rules/cursor-plan-output.mdc`](../../.cursor/rules/cursor-plan-output.mdc) |
| `plan_profile` | `execution`（缺省）末条须含计划 vs 实际 + Git 状态证据；宿主支持时可含 **`/gitx plan`**；`research` 末条**不得**强制 `/gitx plan`，须 `git status --porcelain` + 正文对照 | [`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md) CreatePlan 输出契约 |
| 宿主剥离 YAML | 未知键被剥离时须**手动**补 `plan_profile: research` | [`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md) Plan profile 节 |
| hook 不写字段 | **不要**假定 skill 路由或 hook 会改写 plan 文件 | [`.cursor/rules/cursor-plan-output.mdc`](../../.cursor/rules/cursor-plan-output.mdc) §4 |

### 2.2 Hook：Plan → Build 与 autopilot goal 对齐（默认关闭）

- 环境变量 `retired Plan-Build goal gate`：beforeSubmit 全树或用户提示中出现 `.cursor/plans/*.plan.md` 时**视同** `/autopilot` 拉起 goal 门控；**默认关闭**；不自动执行 shell（见 `router_env_flags.rs` 模块注释）。

### 2.3 审 plan 样例 findings（`plan_review_findings_round1.md`）

| ID | 主题 | 采纳状态 |
|----|------|----------|
| 1 | 末条 Git 状态证据与 closeout 中 `git diff --stat` 习惯对齐不足 | 采纳 |
| 2 | 「一轮修订」Verify 仅 `rg Finding`，不足以证明修订已写入**本计划文件** | 采纳 |
| 3 | `Blocked by` 分支示例 | defer |
| 4 | 深度 review 防空壳：Done 要求至少一条符号锚点 | 采纳 |
| 5 | `isProject` 与多工作区文档说明 | defer |

### 2.4 int-audit Verify：`rg` 证据（2026-05-11 于仓库根执行）

```text
rg -n "CreatePlan|plan_profile|ROUTER_RS_CURSOR_PLAN_BUILD" \
  skills/plan-mode/SKILL.md \
  .cursor/rules/cursor-plan-output.mdc \
  docs/plans/plan_review_findings_round1.md \
  scripts/router-rs/src/router_env_flags.rs
```

结果摘要：`plan_review_findings_round1.md` 对上述三模式无命中（该文件用语为 `gitx`、`Finding` 等）；其余三文件共 **40** 处命中（`cursor-plan-output.mdc` 6、`router_env_flags.rs` 9、`plan-mode/SKILL.md` 25）。对 findings 文件的补充检索：

```text
rg -n "CreatePlan|plan_profile|gitx|Finding" docs/plans/plan_review_findings_round1.md
```

→ **8** 处命中，覆盖 findings 全文。

## 3. 外部：宿主默认路径、Build、已知故障类（ext-cursor）

以下每条均可与公开页面独立核对。

1. **默认落盘在用户主目录**：Plan Mode 文档写明计划默认保存在用户目录；需 **「Save to workspace」** 才进入工作区以便版本管理与共享。  
   来源：[Cursor Plan Mode 官方文档](https://cursor.com/docs/agent/plan-mode)

2. **工作流**：澄清问题 → 检索代码库 → 生成实现计划 → 在聊天或 Markdown 中编辑 → 就绪后点击 Build。  
   来源：同上。

3. **内部 todo 与进度脱节、工作区同步问题**：社区报告「Internal to-do's not coherent with actual progress」「File saved to workspace is not correctly synced」等，并给出 **workdocs-first** 规则 workaround（`doc/workdocs/` 为真源、内部 todo 仅镜像）。  
   来源：[forum.cursor.com — Plan mode save to workspace workaround](https://forum.cursor.com/t/plan-mode-save-to-workspace-workaround/136968)

4. **Create Plan 工具在部分版本失败**：多用户报告「failing to create plan.md」；官方回复承认影响多用户并指向同类线程（需 Request ID / 控制台日志排查）。  
   来源：[forum.cursor.com — Plan Mode Failing in Create Plan tool](https://forum.cursor.com/t/plan-mode-failing-in-create-plan-tool/147483/2)

## 4. 问题根因 × 加强手段（synth-reco）

| 根因归类 | 典型表现 | 加强手段（对应计划 §4） | 成本 |
|----------|----------|---------------------------|------|
| 模型 / CreatePlan 习惯 | 阶段名式 todo、缺 Verify | **A** 规则 + 最小 frontmatter 模板 + 强例 | 低 |
| 宿主产品边界 | 默认不在 repo、内部 todo 漂移 | **B** Save to workspace、workdocs 镜像、文档化 env | 中 |
| 仓库契约未自动强制 | hook 不修补 YAML | **A** 人工自检；可选 **C** 校验脚本/CI | 低→高 |
| Plan→执行门控可选且默认关 | Build 不必然触发 goal | **B** 显式开启 `retired Plan-Build goal gate` | 中 |
| Verify 可伪造 / 证据不足 | `rg Finding` 不绑计划文件 diff | **A** 将 round1 建议写入 skill 强例（`git diff` 计划路径、closeout 含 `--stat`） | 低 |
| 宿主演进 / 版本 bug | plan.md 创建失败 | **D** 跟踪 Cursor 发行说明与论坛；本仓库不冒充已复现 | — |

**Open gaps**（与计划 §6 一致，未在本机复现）：当前 Cursor 版本上 Create Plan 失败率；多根工作区下 Plan 路径行为；团队是否将 `.cursor/plans/` 全量纳入 git 或仅 `docs/plans/` 摘要。

## 5. 建议优先级（执行摘要）

1. 继续以四元组 + `plan_profile` + YAML/正文对齐 + 可选只读审 plan 为主杠杆；补强 Verify 的**可复核**证据（计划文件 diff、closeout 与 Git 状态证据对齐）。  
2. 协作与审计优先：**Save to workspace** 或仓库内 `docs/plans/`（或 workdocs 约定）作为人类可读真源。  
3. 自动化门禁仅在团队共识后引入，避免误伤草稿 plan。

## 6. 调研收口（research-closeout）

- **交付物**：本文件 [`docs/plans/plan_writing_capability_research_synthesis.md`](plan_writing_capability_research_synthesis.md)；索引一行 [`docs/README.md`](../README.md)（Cursor Plan 行内增加本合成文档链接）。  
- **对照**：与调研计划正文 §1–§6 及 frontmatter todos 四项语义一致；宿主 bug 类仅作 **open gap**，不断言本仓库已复现。  
- **Verify**：已执行 `git status --porcelain`（2026-05-11）。工作区另有大量**既有**未提交/未跟踪文件，故**非**全仓干净状态；与本调研直接相关的交付为上述 `docs/plans/` 新文件与 `docs/README.md` 的索引补丁。调研 profile 下**未**要求执行 `/gitx plan`。
