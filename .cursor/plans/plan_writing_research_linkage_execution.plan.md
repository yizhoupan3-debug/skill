---
name: Plan调研联动与范围开关
overview: 本文件为执行计划（plan_profile: execution / 缺省）。允许按下方 todos 修改 skills/plan-mode/SKILL.md、docs/plans/plan_writing_capability_research_synthesis.md、docs/plans/plan_todo_checklist.md；按需最小补丁 docs/README.md。末条以计划 vs 实际 + Git 状态证据收口，宿主支持时可使用 /gitx plan。目标：把《plan能力调研加强》合成稿 §2–§5 与 plan_review_findings_round1 采纳项**逐项**写回 skill/清单/合成文档；补齐调研范围开关（默认仅仓库内；用户要「外部」则内外并行）及与本地审 plan / 深度 review / Git 状态证据收口的显式联动。不修改附件 plan能力调研加强_982eda11.plan.md；本轮不实现 router-rs 对 .plan.md 的机器校验（Non-goals）。
todos:
  - id: trace-matrix
    content: 在 docs/plans/plan_writing_capability_research_synthesis.md 增加「调研→执行映射」表：合成稿 §2.1 每行条款、§2.3 findings 1/2/4、§3 四条外部结论、§4 矩阵 A–D 行 → 对应本 execution 的 todo id 或明示 defer | Done: 表至少 12 行且每行含「合成出处 + 执行落点路径」；defer 行含原因 | Verify: rg -n \"调研→执行映射|§2\\.1|§4\" docs/plans/plan_writing_capability_research_synthesis.md
    status: pending
  - id: skill-research-scope
    content: 在 skills/plan-mode/SKILL.md 新增「调研范围（Research scope）」：overview 模板句①仅仓库内只读 ②仓库内+外部只读并行（外部开启不得省略内部 todo）；与 Plan profile research 的 overview 模板并列说明可粘贴 | Done: 两模板句全文可检索；写明外部仅 WebSearch/WebFetch/MCP 只读、须在 §证据与范围 写 URL+日期 | Verify: rg -n \"调研范围|仅仓库内|外部只读\" skills/plan-mode/SKILL.md
    status: pending
  - id: skill-linkage-table
    content: 在同文件增「能力与工件联动」表或同级小节：行含 本地代码调研 rg+Read、router-rs framework snapshot（可选一句指向 RUNTIME_REGISTRY/harness doc）、Workflow 第3步 findings 建议落盘 docs/plans/*findings*.md、code-review-deep 触发画像、execution 下游 Git 状态证据（宿主支持时 /gitx plan）、research 末条 git status；列 profile 与最小证据 | Done: code-review-deep 与 gitx 各至少一链 | Verify: rg -n \"code-review-deep|gitx|findings|framework snapshot|Git 状态证据\" skills/plan-mode/SKILL.md
    status: pending
  - id: skill-strong-examples
    content: 在 skills/plan-mode/SKILL.md「弱例与强例」或紧接段增补 round1 采纳：① execution 末条/closeout 须含 git diff --stat 或声明无代码 diff（对齐 skills/gitx/SKILL.md Substantive diff 习惯）；② 审 plan 修订轮 Verify 须含 git diff .cursor/plans/<本文件>.plan.md | head -n 40 或 closeout 内嵌摘要；③ 深度 review Done 须含至少一符号锚点 + Verify rg 命中 | Done: 三条各有一句可复制到 .plan.md 的示例 | Verify: rg -n \"git diff --stat|head -n 40|符号锚点\" skills/plan-mode/SKILL.md
    status: pending
  - id: checklist-readme
    content: 更新 docs/plans/plan_todo_checklist.md：research 勾选增加「overview 调研范围句」「外部开启时内外并行」「审 plan 修订可复核 git diff 计划路径」「execution closeout 与 --stat」；docs/README.md 若 Cursor Plan 行无本 execution 计划链则增链 .cursor/plans/plan_writing_research_linkage_execution.plan.md | Done: checklist 新增行可 rg 命中 | Verify: rg -n \"调研范围|git diff|plan_writing_research_linkage\" docs/plans/plan_todo_checklist.md docs/README.md
    status: pending
  - id: continuity-external-doc
    content: 在 skills/plan-mode/SKILL.md Continuity 小节增 2–4 句：Cursor 默认计划目录与用户主目录、Save to workspace、forum  workaround 链到 plan_writing_capability_research_synthesis §3（不重复长贴） | Done: 含官方 Plan Mode URL 或相对链至合成稿 | Verify: rg -n \"Save to workspace|cursor.com/docs/agent/plan-mode|plan_writing_capability\" skills/plan-mode/SKILL.md
    status: pending
  - id: gitx-closeout
    content: 对照本 plan 正文与已实现 diff，记录 Git 状态证据 @ 仓库根 | Done: 上列 todos 均有对应文件变更或正文 defer；未改 plan能力调研加强_982eda11 | Verify: git status --short --branch && git diff --stat；宿主支持时可执行 /gitx plan（与 /gitx 同契约，见 skills/gitx/SKILL.md）
    status: pending
plan_profile: execution
isProject: true
---

# Plan 调研联动与范围开关（执行细化版）

本文件取代仅含高层 bullet 的旧版思路，把 **已完成的调研合成** 逐项「翻译」为可改文件、可复制模板与可 `rg` 验收句。

## 1. 调研结论 → 执行落点（总表）

以下每条必须在 **§2 映射表**（由 todo `trace-matrix` 写入 `plan_writing_capability_research_synthesis.md`）中展开为表格行；此处为执行侧摘要。

| 调研出处 | 结论摘要 | 执行落点（文件 / 小节） | Todo id |
|----------|----------|-------------------------|---------|
| 合成 §2.1 行1 | CreatePlan 后四元组 + 自检 | 已在 cursor-plan-output；skill 交叉引用即可 | `skill-linkage-table` 可选 |
| 合成 §2.1 行2 | research/execution 末条分岔 | plan-mode CreatePlan 契约已有；联动表指向末条 | `skill-linkage-table` |
| 合成 §2.1 行3 | 宿主剥离 YAML → 手补 plan_profile | plan-mode Plan profile 节加一句与「调研范围」并置 | `skill-research-scope` |
| 合成 §2.1 行4 | hook 不写字段 | plan-mode Continuity 或联动表重申 | `skill-linkage-table` |
| 合成 §2.2 | ROUTER_RS_CURSOR_PLAN_BUILD 默认关 | plan-mode Continuity 已有；联动表列「Build→goal」+ `router_env_flags.rs` | `skill-linkage-table` |
| 合成 §2.3 F1 | closeout 与 `git diff --stat` | plan-mode **强例** + checklist | `skill-strong-examples`, `checklist-readme` |
| 合成 §2.3 F2 | 修订轮须绑计划文件 diff | plan-mode **强例** + checklist | `skill-strong-examples`, `checklist-readme` |
| 合成 §2.3 F4 | 深度 review 符号锚点 | plan-mode **强例**；链 `code-review-deep` | `skill-strong-examples`, `skill-linkage-table` |
| 合成 §3.1–3.2 | 默认主目录、Save to workspace | plan-mode Continuity + 链合成稿 §3 | `continuity-external-doc` |
| 合成 §3.3–3.4 | 内部 todo 漂移、CreatePlan 失败 | Continuity 短提醒 + open gap；不冒充复现 | `continuity-external-doc` |
| 合成 §4 行 A | 模板 + 强例 | plan-mode 弱例/强例 + research 范围 | `skill-strong-examples`, `skill-research-scope` |
| 合成 §4 行 B | workdocs / env 文档化 | Continuity 指合成稿 §3 与 AGENTS 已有 env | `continuity-external-doc` |
| 合成 §4 行 C | 程序化校验 | **defer**：本 execution Non-goals | trace-matrix 表中标 defer |
| 合成 §4 行 D | 宿主演进 | open gap 一句 | trace-matrix |
| 合成 §5 | 优先级三句 | 映射表「验收顺序」列或 synthesis §5 下加「执行顺序」 | `trace-matrix` |
| 用户新增 | 默认仅内部；说「外部」则内外并行 | overview 模板 + research todo 写法 | `skill-research-scope`, `checklist-readme` |
| 用户新增 | 与本地 review 能力联动不足 | 联动表 + synthesis 映射 | `skill-linkage-table`, `trace-matrix` |

## 2. `skills/plan-mode/SKILL.md` 预期补丁结构（写稿时按此顺序）

1. **`plan_profile: research` 附近或 Workflow 前**：插入 **调研范围** — 两套 `overview` 可加贴句（与现有「零实现面改动」模板合成同段或下一段）。
2. **Workflow 第 1、3 步之后**：插入 **能力与工件联动** 表（或 `###` 小节），显式出现：`docs/plans/*findings*.md`、`skills/code-review-deep/SKILL.md`、`skills/gitx/SKILL.md`、`/gitx plan`、`router-rs … PLAN_BUILD…`（一句，链 `AGENTS.md`）。
3. **弱例与强例**：追加 3 条 **强例**（对应 round1 F1/F2/F4），每条格式与现有「强：」一致，便于复制到 `.plan.md` 的 `todos[].content` Verify 段。
4. **Continuity**：补 Cursor 落盘默认 + Save to workspace + 指向 `plan_writing_capability_research_synthesis.md` §3 作为外部/宿主痛点索引。

## 3. `docs/plans/plan_writing_capability_research_synthesis.md` 预期补丁

- 新节 **「调研结论 → 本仓库执行映射」**：Markdown 表，列至少：`合成稿锚点 | 执行动作 | 目标路径 | 状态(done/defer)`。
- 不删除既有 §1–§6；新节建议插在 §1 与 §2 之间或 §6 之后（以阅读流自然为准）。

## 4. `docs/plans/plan_todo_checklist.md` 预期补丁

在 `plan_profile: research` / CreatePlan 相关勾选块增加（措辞可微调，语义须保留）：

- `overview` 含 **调研范围** 句（仅内部 / 内部+外部）；外部类须存在至少一条 todo 的 Verify 引用外部 URL 或 `WebFetch` 结果核对方式。
- **审 plan 修订**：Verify 含 `git diff` 针对**本计划**路径或 closeout 内嵌 diff。
- **execution 收口**：closeout 或末条关联文档含 `git diff --stat` 或显式「无代码 diff」。

## 5. Non-goals（本 execution 明确不做）

- 不新增 `router-rs` 子命令或 CI 硬校验 `.plan.md` frontmatter。
- 不修改 `plan能力调研加强_982eda11.plan.md`。
- Finding 3/5（Blocked by 示例、isProject）仅可在映射表标 **defer**，不强制本 PR 写样例计划。

## 6. 整体验收（除末条外）

```bash
rg -n "调研范围|调研→执行映射|git diff --stat|head -n 40|code-review-deep" \
  skills/plan-mode/SKILL.md \
  docs/plans/plan_writing_capability_research_synthesis.md \
  docs/plans/plan_todo_checklist.md
```

## 7. 末条收口

由宿主执行 **`/gitx plan`**（与 **`/gitx`** 同契约，见 [`skills/gitx/SKILL.md`](../../skills/gitx/SKILL.md)）：对照本文件 frontmatter `todos` 与正文 §1–§5，逐项勾选完成或 defer 原因，并完成 Git 流程。
