---
name: paper-workbench
description: |
  Unified front door for paper work. Use when the user has a manuscript-level
  task and should not have to choose between review, revision, logic, figures,
  or prose lanes first. Good for requests like "帮我看这篇 paper 现在能不能投",
  "根据 reviewer comments 改到能投", "先审再改", "整体推进这篇论文", or
  "这篇稿子现在该怎么处理". Also use when manuscript preparation should start
  from target-journal refs, e.g. "先下载20篇目标期刊相近ref再写" or "学ref讲故事".
  Also use for feedback/repair asks like "paper review不好用，彻底优化",
  "论文写作不好用，持续优化", or "允许外部调研". This skill picks the right paper
  lane first, allows external literature / venue lookup when useful, and keeps
  the workflow continuous without making the user switch skills. Use top-tier
  journal / top-conference standards when the user says 顶刊, 顶会, CCF-A,
  Nature/Science/Cell, NeurIPS/ICML/ICLR, or wants the paper pushed toward a
  genuinely selective venue rather than merely polished.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: preferred
user-invocable: true
disable-model-invocation: false
trigger_hints:
  - 帮我审这篇 paper
  - 帮我审这篇论文
  - 投稿前把关
  - 整篇严审 / 全文审核
  - 穷举审 / 逐句审
  - R&R / rebuttal
  - cover letter
  - abstract 改写
  - introduction 重写
  - 顶刊 / 顶会 / CCF-A
  - Nature / Science / Cell / NeurIPS / ICML / ICLR
  - top-tier 论文
  - revision modes
  - claim 漂移
metadata:
  version: "1.16.0"
  platforms: [supported]
  tags: [paper, manuscript, review, revise, submission, orchestrator, top-tier]
framework_roles: [orchestrator, planner, verifier]
framework_contracts: {emits_findings: true, consumes_findings: true, emits_execution_items: true, consumes_execution_items: false, emits_verification_results: true}
risk: medium
source: local

---

# Paper Workbench

This skill is the one front door for paper work.

## 强对抗审稿默认立场（硬性）

一切审阅、返修、预判「能不能投」「顶刊是否能过」时，**不按友好读者模型**，而按**敌意审稿人 / 最坏合理解读（hostile but fair）**：专盯 **claim–evidence 缝、closest-work、复现与代码—正文对齐、推导跳步、统计与比较的公平性**。软球结论、只给情绪价值、或暗示「应该能过」而无逐条可关闭证据，视为**未执行本 skill**。

与本立场冲突的捷径（降口径逃难、rebuttal-only、代码空诺、数学直觉化、`surgical` 全局乱改等）一律以 [`references/claim-evidence-ladder.md`](references/claim-evidence-ladder.md)、[`references/edit-scope-gate.md`](references/edit-scope-gate.md) 为硬闸。

启用外研时，审稿/校准产出须满足 [`docs/references/rfv-loop/reasoning-depth-contract.md`](../../docs/references/rfv-loop/reasoning-depth-contract.md) §A–B 的 **`Claims`**、**Contradiction sweep**、**Unknowns** 与可追溯 **retrieval_trace**（不能仅靠「读起来专业」的综述）；门面仍由本会话收口，细节上复用 **`@lane:reviewer`** 的 External lane shape 约定。

**宿主 hook（L4 短码）**：`router-rs` 在 **Cursor `beforeSubmit`** 与 **Claude Code / Antigravity CLI `UserPromptSubmit`** 命中写作/润色语境时合并 **`PAPER_PROSE_QUALITY_HOOK`**（真源 `configs/framework/PAPER_PROSE_QUALITY_HOOK.txt`，**默认开**）；手稿审稿/改稿语境可另合并 **`PAPER_ADVERSARIAL_HOOK`**（opt-in）。受 `ROUTER_RS_OPERATOR_INJECT` 总闸约束。Prose 子开关：`ROUTER_RS_CURSOR_PAPER_PROSE_HOOK` / `ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK` / `ROUTER_RS_ANTIGRAVITY_CLI_PAPER_PROSE_HOOK`（unset=开，`0`=关）。Adversarial：`ROUTER_RS_*_PAPER_ADVERSARIAL_HOOK=1` 启用。见 [`references/prose-chain-contract.md`](references/prose-chain-contract.md) §L4。

It exists so the user does not need to decide first whether the job is
`@lane:reviewer`, `@lane:writer`, or a review/revision dimension mode.

## Progressive disclosure（渐进披露）— 减入口、减抽象

**第一性原理**：用户要的是「这篇稿子下一步怎么办」，不是背诵技能拓扑。

- **L0（默认）**：只暴露本前门与用户可理解的结果（verdict / blockers / next move / edit_scope 若即将改稿）。**不要**要求用户在 `@lane:reviewer`、`@lane:writer` 之间先选一个；在对话内自行路由。
- **L1**：用户已明确「只润色」「只审不改」「按 R1 改」或给出 `edit_scope` / `scope_items` 时，再收紧模式，仍不必展开全套维度名。
- **L2（排障 / 强用户）**：用户**点名**某专科 skill 或某维度（logic / figure-table / notation）时，直接尊重；文档里的 lane map 供实现方用，不是菜单。
- **L3（长程）**：多轮冻结 claim、并行 sidecar、`PAPER_GATE_PROTOCOL` 磁盘树 —— **仅当**任务真的需要跨会话状态时再物化；日常一轮交互不要默认铺协议。

**减法**：专科 skill 多 ≠ 用户入口多。`disable-model-invocation` 的 paper 专科应视为 **内部能力切片**；入口计数按 **用户可见的一个前门** 算，否则「agent 太多」会反噬可用性。

**全栈索引**（技能 × reference × L0–L3）：[`references/RESEARCH_PAPER_STACK.md`](references/RESEARCH_PAPER_STACK.md)。

**宿主与专科契约（与 stack 对齐）**：`$paper-workbench` 在所有宿主上均为用户可 invocation 的前门；`@lane:writer` / `@lane:reviewer` 保持 `disable-model-invocation`，表示由本前门在**任一支宿主**上**内联**加载的专科 lane，而非与用户入口并列的第二扇门。权威表述见 [`references/RESEARCH_PAPER_STACK.md`](references/RESEARCH_PAPER_STACK.md) §宿主与专科入口。

## Use this when

- The user has a whole-paper task and the first move is still part of the job
- The user wants the paper judged, then possibly revised, in one continuous flow
- The user wants reviewer comments executed without manually picking the next lane
- The user wants to prepare or rewrite a manuscript by first learning target-journal reference papers
- The user says `先审再改`, `改到能投`, `整体推进这篇论文`, or similarly workflow-shaped asks
- The task may need claim narrowing, appendix routing, figure/table cleanup, or local prose polish after the main decision is clear
- The user complains that paper review, revision, or writing skills are poor and wants the manuscript workflow tightened
- External calibration can change the verdict, baseline expectations, novelty bar, or target-journal fit
- The user wants 顶刊/顶会/CCF-A/top-tier readiness, or wants the workflow to
  produce papers that can survive selective venues rather than local polish

## Do not use

- The user wants to advance a non-manuscript research project, topic, or experiment plan -> use `$research-execution`; this front door is manuscript-only
- The user explicitly wants only one narrow lane and names it clearly:
  * local text polish only -> stay on **`$paper-workbench`** prose intake (inline `@lane:writer` after Claim card / `edit_scope`; do not treat `@lane:writer` as a parallel user entry)
  * literature corpus / related work only -> keep the work here as source-backed paper context until it narrows to writing or citation hygiene
  * notation consistency only -> use `notation sweep` under `@lane:reviewer`

## Edit scope gate (mandatory before any manuscript edit)

Any path that touches the manuscript (`@lane:writer` or
edits executed from this front door) must first fix **`edit_scope`** using
[`references/edit-scope-gate.md`](references/edit-scope-gate.md):

- **`surgical` (精准修改)** — default when the user has **not** clearly authorized
  structural refactoring; **仅**改 `scope_items` 锚定表面；**禁止**整篇/整节回贴式替换、全局术语统一、通读顺稿、以及对未点名段落的任何修改（无论用户是否粘贴了全文）。
- **`refactor` (大面积重构)** — only with explicit user opt-in or strong refactor
  signals; allows the full honest-edit contract of inline revision.

If the user is vague (`润色`, `改好一点`, `优化表述`) or mixed signals appear,
**ask one disambiguation question** (`surgical` vs `refactor`) before editing.

Optional machine token on its own line: `edit_scope: surgical` or
`edit_scope: refactor`。

**精准修改硬约束**：`surgical` 下必须遵循
[`references/edit-scope-gate.md`](references/edit-scope-gate.md) 中的 **硬等级**、**防扩写**、**整篇回贴禁令**、**静默全局替换禁令**、**锚定三选一**、**改动上限**、**默认交付形态（hunk/逐条）**、**改前自检**。**凡**对 `scope_items` 外字句的改动即 **越权**，须撤回或升格 `refactor` / 补列条目；**不得**用「通读」「统一文风」「对齐 mirror」当借口。

## Audit depth routing (`audit_depth`)

Resolve **`audit_depth`** before inline `@lane:reviewer` (machine token on its own line
overrides heuristics: `audit_depth: exhaustive` | `audit_depth: compact`).

| `audit_depth` | Default when | Inline reviewer behavior | User-visible output |
|---------------|--------------|--------------------------|---------------------|
| **exhaustive** | 整篇严审 / 能不能投 / 投稿前把关 / 全文审核 / 帮我审这篇 / strict reviewer / 顶刊审稿 / 穷举 / 逐句 / 逐公式 | Load [`paper-exhaustive-audit.md`](references/paper-exhaustive-audit.md); **pass through** depth — reviewer must not override | Verdict first, then **full** `findings_by_dimension` + `warning_items`; output **must** include `audit_depth: exhaustive` |
| **compact** | 快速看一下 / 只审 claim / 只审图表 / 只审语言 / 单维度 + narrow scope | Compressed 8-step reviewer workflow | Verdict + top blockers summary |

**Pass-through rule**: when workbench invokes `@lane:reviewer`, the resolved
`audit_depth` is authoritative for that turn; reviewer must not re-default to compact.

Optional sidecars during exhaustive review:

- PDF / rendered figures → `$visual-review`
- Formal theorem blocks → read-only `$math-derivation` witness
- Rubric / Bonus text present → [`rubric-audit-bridge.md`](references/rubric-audit-bridge.md)

## Default front-door behavior

Default behavior is rule-based, not a user-facing mode menu:

- If the user asks a vague whole-paper question (能不能投/投稿前把关/整体推进): resolve **`audit_depth: exhaustive`**, strict verdict, then **full dimension findings** (not top-N truncation), then route internally.
- If the user asks a **quick** or **single-dimension** review: resolve **`audit_depth: compact`**, verdict + top blockers.
- If the user provides reviewer comments or accepted findings and asks to change the paper now: revise, honoring `edit_scope`.
- If the user explicitly names one dimension (claim/evidence, refs, figures, notation, language): run that slice only.
- If the user explicitly asks to learn target-journal refs first: run the ref-first workflow under this front door.
- If the user provides a bounded text block and says “只改表达不改 claim”: do local prose only after the claim boundary is frozen.

Do not make the user switch skills just because the work naturally moves from
judgment to revision.

For review-like asks, do not block on missing target venue or reference corpus:
start with a provisional bar, run external calibration when useful, and clearly
separate "known blocker" from "uncertainty that needs lookup".

## Prose quality intake（自动触发，勿等用户声明）

**硬规则**：只要将触达手稿正文句子（含用户只贴一段、说「改这段/不通顺/帮我看看文字」、或粘贴 LaTeX/摘要/引言），**立即**走 prose chain——**不得**等用户写 `language_register` / `writing_mode` / `prose_qc`。

自动执行：

1. **推断** `language_register`（见 [`references/prose-quality-gate.md`](references/prose-quality-gate.md) §Language register）
2. **默认** `edit_scope: surgical` + 从用户粘贴/点名推断 `scope_items`（模糊时**一问** surgical vs refactor，**不问** register）
3. **默认** `writing_mode: ladder-full` + 极简 Claim card（四槽可短，不可省略）
4. 若 claim/evidence 明显未冻结且用户要「能不能投」→ 先 reviewer；**纯改文字**则 Claim card 后直写

转发 `@lane:writer` 时**自带**上述默认值，用户无 token 也须完整交付 `tone_audit` + `prose_qc` + Stage B（或 Stage A-only 若 ladder_blocked）。

**全链路真源**：[`references/prose-chain-contract.md`](references/prose-chain-contract.md)（路由 → intake → reviewer language findings → reviser → inline writing → 可选 `PROSE_QC_LOG`）。多轮改稿建议维护 [`references/templates/PROSE_QC_LOG.template.md`](references/templates/PROSE_QC_LOG.template.md)。

## Prose quality chain（默认开启）

凡触达正文句子的路径（含 inline `@lane:writer`）默认走 prose chain，**不得**跳过 `language_register` 或 `prose_qc`：

| 步骤 | 动作 |
| --- | --- |
| 1 | NL/用户入口 → **本前门**（不直跳 `paper-writing`） |
| 2 | Intake：`language_register` + `edit_scope` + Claim card |
| 3 | 若 claim 未冻结或用户要「能不能投」→ inline `@lane:reviewer`（language findings 含 `prose_repair_class`） |
| 4 | 若有 R&R / 结构改动 → inline revision（本前门 edit_scope gate 内） |
| 5 | Inline `@lane:writer`：`ladder-full` → `tone_audit` + `prose_qc` → Stage B |
| 6 | 收口：可选 append `paper_story/PROSE_QC_LOG.md` |

宿主短码：**`PAPER_PROSE_QUALITY_HOOK` 默认开**（`ROUTER_RS_CURSOR_PAPER_PROSE_HOOK=0` 关闭），见 prose-chain-contract §L4。

## Anti-bad-output rules

[详细规则](references/anti-bad-output-rules.md)

## Top-tier submission bar

[Top-tier bar 详](references/top-tier-bar-summary.md)

## Internal lane map

[Internal lane map (maintainer-only)](references/internal-lane-map.md)

## What this skill should deliver

本前门转发或收口 **`@lane:writer`** 的改稿时，**统一输出顺序**须先回声门控与叙事契约，再贴正文块：**`edit_scope` → `scope_items`/`non_goals` 或 `refactor_intent`/`risk_note` → Claim card（四槽）→ `language_register` →（可选 Stage A 提纲）→ `tone_audit` → `prose_qc` → prose/hunks → `change_id` 账本（`surgical`）或 `sections_touched` + `claim_ledger_touch_statement`/`claim_ledger_delta`（`refactor`）**；细则见 [`references/prose-quality-gate.md`](references/prose-quality-gate.md) 与 [`references/edit-scope-gate.md`](references/edit-scope-gate.md)。

Keep the user-facing output simple:

1. what mode the paper is in now
2. the real blockers or active edit target
3. the next honest move

When the ask is whole-paper or workflow repair, the minimum useful decision card is:

```text
mode:
verdict_or_blocker:
active_lane:
next_edit:
external_calibration_needed:
top_tier_bar:
claim_lock_status:
```

Behind the scenes, this skill may switch lanes. The user should not need to.

For multi-turn work, the front door should maintain a compact claim ledger and
evidence anchors as stable artifacts:

- `paper_story/CLAIM_LEDGER.md`
- `paper_story/EVIDENCE_ANCHOR_MAP.md`

These artifacts are required before repeated local polishing passes.

## Verification and closeout

Before claiming a revision is done, verify that all `edit_scope` items are addressed and the claim ledger reflects actual changes. The `verifyx` closeout gate checks for evidence rows and successful verification commands.

## Ref-first manuscript workflow

When the user wants to learn target-journal references before writing:

1. Build the 20-paper target-journal corpus and ref-learning brief as source-backed paper context under this front door.
2. Route to `@lane:reviewer` logic mode only if the corpus exposes a claim/evidence or novelty mismatch.
3. Route to `@lane:writer` for story spine, section plan, and bounded prose rewrite.
4. Keep `$citation-management` for final citation truth and `.bib` hygiene, not for the initial story-learning pass.

The handoff artifact should be simple:

```text
target venue -> 20-ref corpus -> venue story norm -> our paper's story spine -> sections to rewrite
```

In filesystem-backed work, the stable artifacts are:

- `paper_ref/ref_learning_brief.md`
- `paper_story/STORY_CARD.md`
- `paper_story/SECTION_REWRITE_PLAN.md`
- rewritten manuscript sections or patch notes

## 审稿意见 / R&R：禁止逃避（硬约束）

与前门 **Anti-bad-output**、[`references/claim-evidence-ladder.md`](references/claim-evidence-ladder.md) §审稿意见 / R&R、[`references/research-language-norms.md`](references/research-language-norms.md) 叠加；**优先于**「少惹事、快过关」的模型默认。可核验关停与 repair 优先级以 **claim-evidence-ladder** 为单真源，本节只强调审稿语境下易逃逸的禁令。

- **禁止「降 claim / 缩口径」当主手逃避**：在仍属 B 类可闭合、且存在合理 **evidence-first** 路径时，不得把本轮主策略做成「改弱提法 + 加长 limitation」却对证据结构不动（见阶梯文与后门 **Hard rules** 已有条目；本条是审稿场景的显式复述）。
- **禁止「防御口径」顶替改稿**：不得用连环 hedge、冗长免责声明、叠叠乐的 `but/not/rather than`（辩论腔 prose）填满回复或正文，**代替**审稿人点名的对照/消融/协议澄清/图表修正/披露与复现条目。
- **禁止 rebuttal-only**：意见客观要求手稿、图表、方法、统计或结构化补充材料变更时，**不得**只交 response letter；须并排交付可追溯的 **手稿改动（或等价 hunk/diff）** 与「意见 → 改动」映射。
- **逐条关停**：每条审稿意见须有 **point_id → (manuscript_delta | 已落地的补证与分析 | `cannot_fix_because`）**之一；不得以「我们已经温和表述」「理解审稿人关切」等话述冒充关闭。
- **默认正面硬修**：可先判可行性与优先级，但一旦进入改稿链路，应以 **repair**（补证、重写、重画、补强比较公平性）为第一默认，而非嘴上认错、手稿不动。
- **代码/实现类意见（硬）**：审稿人追问复现性、复杂度、对齐伪代码 vs 源码、默认值/随机种子、潜在 bug——须交付 **可查证的复现与对齐物**（如版本化的 artifact、环境与入口命令、方法与正文/框图一致的对照），或 **修正文中的错误陈述**并说明影响。**禁止**用「将开源」「已向期刊说明」一类**不可立即核对**的承诺当关停件；若暂不发布，须提供 **minimal reproduction bundle**（或等价：独立伪代码补丁 + synthetic sanity + 审稿人可操作的最小脚本）并接受 `cannot_fix_because` 须极严格。
- **数学/推导类意见（硬）**：质疑证明步骤、条件、常量/阶、可测性与交换极限等——须 **手写可检查的补证或勘误**（附录引理链、条件修正、反例后范围收窄），或显式把错误结论改为 **较弱但可证** 的表述并登记 claim。**禁止**仅做「更谦虚的 English」或把定理悄悄改成 prose 直觉而不声明 **推理变更**。
- **双高危默认归类**：未见用户显式「只改文风」豁免时，将 **code-skeptic** 与 **math-skeptic** 类意见默认标为 **`repair` 主轴**（narrow 仅能附 `narrowing_is_primary_because` 走阶梯）。

## Hard rules

- Do not apply manuscript edits without a resolved **`edit_scope`** (`surgical`
  vs `refactor`; see [`references/edit-scope-gate.md`](references/edit-scope-gate.md))
- Do not start with prose polish when the real problem is claim or evidence
- Do not let ref learning turn into sentence copying or citation padding
- Do not force the user to choose reviewer vs reviser before the route is clear
- Do not lose specialist rigor just because the front door is unified
- Do not turn a normal paper review into a process-heavy gate report; lead with
  verdict, blockers, external calibration, and next honest move
- If the shortest honest path is cut, narrow, hide in appendix, or stop defending, say so plainly
- Do not ship or polish prose that violates
  [`references/research-language-norms.md`](references/research-language-norms.md)
  unless the user explicitly waived that scope for the task
- Do not close a revision round with **only** softer claims when findings say
  the honest primary path is **new evidence or analysis**; align with
  [`references/claim-evidence-ladder.md`](references/claim-evidence-ladder.md)

## Verification skill integration

When lanes require structured verification, load the corresponding skill:
- `@lane:reviewer` prose quality checks → [`../prose-verification/SKILL.md`](../prose-verification/SKILL.md)
- `@lane:reviewer` structure/logic checks → [`../structure-verification/SKILL.md`](../structure-verification/SKILL.md)
- Literature/citation integrity checks → [`../literature-verification/SKILL.md`](../literature-verification/SKILL.md)
- Statistical methodology checks → [`../statistical-verification/SKILL.md`](../statistical-verification/SKILL.md)

## Exit Criteria

- verdict 已输出（accept / revise / reject + claim-evidence ladder 完整）
- edit_scope 已门控（仅限 declared scope 内的文件）
- 用户已确认下一步动作（提交修改 / 补充实验 / 放弃）
