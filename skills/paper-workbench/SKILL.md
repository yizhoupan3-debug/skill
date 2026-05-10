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
  - 帮我看这篇 paper 现在能不能投
  - 能不能投
  - 投稿前把关
  - 整篇严审
  - 整篇 review
  - 科研优化
  - 收窄修改范围
  - 定点修改
  - 补丁式修改
  - 不要扩写
  - "edit_scope: surgical"
  - patch-level edit
  - hunk only
  - 不要随便动别的段
  - 顶刊顶会标准
  - 顶刊标准
  - 顶会标准
  - top-tier paper
  - CCF-A 论文
  - 检查符号
  - 符号统一
  - notation sweep
  - 砍到 8 页
  - length budget
  - 只看图表
  - figure table audit
  - paper review
  - paper review 不好用
  - paper review优化
  - paper reviewer优化
  - 论文写作不好用
  - 持续优化论文工作流
  - 外部调研 paper review
  - 允许外部调研
  - 查文献后审 paper
  - review with external research
  - 根据 reviewer comments 修改
  - 根据 reviewer comments 改论文
  - 按审稿意见改论文
  - 按 review 改论文
  - 根据 review 修改论文
  - 根据 reviewer comments 改到能投
  - 先审再改
  - review 完直接改
  - 整体推进这篇论文
  - 这篇论文
  - 这篇论文 该审
  - 这篇论文 该改
  - 该补实验
  - 先下载20篇目标期刊相近ref再写
  - 先找目标期刊ref再改论文
  - 学ref讲故事
  - 目标期刊写作套路
  - 论文故事线整体调整
  - 帮我处理这篇论文
  - 这篇稿子现在该怎么处理
  - 帮我把这篇 paper 弄到能投
  - 科研 skill 优化
  - 顶刊论文
  - 顶会论文
  - Nature/Science/Cell 标准
  - NeurIPS/ICML/ICLR 标准
  - top journal paper
  - top conference paper
  - 该删就删
  - 藏到附录
  - paper workflow
  - paper workbench
  - 改了哪里逐条列出
  - 学术用语规范
  - 术语规范
  - 不要生造概念
  - 论文用语长期规范
metadata:
  version: "1.11.0"
  platforms: [codex, cursor]
  tags: [paper, manuscript, review, revise, submission, orchestrator, top-tier]
framework_roles:
  - orchestrator
  - planner
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
risk: medium
source: local

---

# Paper Workbench

This skill is the one front door for paper work.

## 强对抗审稿默认立场（硬性）

一切审阅、返修、预判「能不能投」「顶刊是否能过」时，**不按友好读者模型**，而按**敌意审稿人 / 最坏合理解读（hostile but fair）**：专盯 **claim–evidence 缝、closest-work、复现与代码—正文对齐、推导跳步、统计与比较的公平性**。软球结论、只给情绪价值、或暗示「应该能过」而无逐条可关闭证据，视为**未执行本 skill**。

与本立场冲突的捷径（降口径逃难、rebuttal-only、代码空诺、数学直觉化、`surgical` 全局乱改等）一律以 [`references/claim-evidence-ladder.md`](references/claim-evidence-ladder.md)、[`references/edit-scope-gate.md`](references/edit-scope-gate.md) 为硬闸。

**Cursor 宿主（可选）**：在 shell/IDE 环境设 `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK=1` 时，`router-rs` 可在 **`beforeSubmit`** 合并短段 **`PAPER_ADVERSARIAL_HOOK`**（真源 `configs/framework/PAPER_ADVERSARIAL_HOOK.txt`），与本 skill 同向加压；受 `ROUTER_RS_OPERATOR_INJECT` 总闸约束。见根 `AGENTS.md` 与 `docs/harness_architecture.md`。

It exists so the user does not need to decide first whether the job is
`$paper-reviewer`, `$paper-reviser`, `$paper-writing`, or a review/revision
dimension mode.

## Progressive disclosure（渐进披露）— 减入口、减抽象

**第一性原理**：用户要的是「这篇稿子下一步怎么办」，不是背诵技能拓扑。

- **L0（默认）**：只暴露本前门与用户可理解的结果（verdict / blockers / next move / edit_scope 若即将改稿）。**不要**要求用户在 `$paper-reviewer`、`$paper-reviser`、`$paper-writing` 之间先选一个；在对话内自行路由。
- **L1**：用户已明确「只润色」「只审不改」「按 R1 改」或给出 `edit_scope` / `scope_items` 时，再收紧模式，仍不必展开全套维度名。
- **L2（排障 / 强用户）**：用户**点名**某专科 skill 或某维度（logic / figure-table / notation）时，直接尊重；文档里的 lane map 供实现方用，不是菜单。
- **L3（长程）**：多轮冻结 claim、并行 sidecar、`PAPER_GATE_PROTOCOL` 磁盘树 —— **仅当**任务真的需要跨会话状态时再物化；日常一轮交互不要默认铺协议。

**减法**：专科 skill 多 ≠ 用户入口多。`disable-model-invocation` 的 paper 专科应视为 **内部能力切片**；入口计数按 **用户可见的一个前门** 算，否则「agent 太多」会反噬可用性。

**全栈索引**（技能 × reference × L0–L3）：[`references/RESEARCH_PAPER_STACK.md`](references/RESEARCH_PAPER_STACK.md)。

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

- The user wants to advance a non-manuscript research project, topic, or experiment plan -> use the current research/project owner; this front door is manuscript-only
- The user explicitly wants only one narrow lane and names it clearly:
  - local text polish only -> use `$paper-writing`
  - literature corpus / related work only -> keep the work here as source-backed paper context until it narrows to writing or citation hygiene
  - notation consistency only -> use `notation sweep` under `$paper-reviewer`

## Edit scope gate (mandatory before any manuscript edit)

Any path that touches the manuscript (`$paper-writing`, `$paper-reviser`, or
edits executed from this front door) must first fix **`edit_scope`** using
[`references/edit-scope-gate.md`](references/edit-scope-gate.md):

- **`surgical` (精准修改)** — default when the user has **not** clearly authorized
  structural refactoring; **仅**改 `scope_items` 锚定表面；**禁止**整篇/整节回贴式替换、全局术语统一、通读顺稿、以及对未点名段落的任何修改（无论用户是否粘贴了全文）。
- **`refactor` (大面积重构)** — only with explicit user opt-in or strong refactor
  signals; allows the full honest-edit contract of `$paper-reviser`.

If the user is vague (`润色`, `改好一点`, `优化表述`) or mixed signals appear,
**ask one disambiguation question** (`surgical` vs `refactor`) before editing.

Optional machine token on its own line: `edit_scope: surgical` or
`edit_scope: refactor`。

**精准修改硬约束**：`surgical` 下必须遵循
[`references/edit-scope-gate.md`](references/edit-scope-gate.md) 中的 **硬等级**、**防扩写**、**整篇回贴禁令**、**静默全局替换禁令**、**锚定三选一**、**改动上限**、**默认交付形态（hunk/逐条）**、**改前自检**。**凡**对 `scope_items` 外字句的改动即 **越权**，须撤回或升格 `refactor` / 补列条目；**不得**用「通读」「统一文风」「对齐 mirror」当借口。

## Default front-door behavior

Default behavior is rule-based, not a user-facing mode menu:

- If the user asks a vague whole-paper question (能不能投/投稿前把关/整体推进): start with a strict verdict and top blockers, then route internally.
- If the user provides reviewer comments or accepted findings and asks to change the paper now: revise, honoring `edit_scope`.
- If the user explicitly names one dimension (claim/evidence, refs, figures, notation, language): run that slice only.
- If the user explicitly asks to learn target-journal refs first: run the ref-first workflow under this front door.
- If the user provides a bounded text block and says “只改表达不改 claim”: do local prose only after the claim boundary is frozen.

Do not make the user switch skills just because the work naturally moves from
judgment to revision.

For review-like asks, do not block on missing target venue or reference corpus:
start with a provisional bar, run external calibration when useful, and clearly
separate "known blocker" from "uncertainty that needs lookup".

## Anti-bad-output rules

- Do not start with language polish when claim/evidence, novelty, baseline, or target-venue fit is unresolved.
- Do not give a long review taxonomy before the verdict; lead with verdict, blockers, evidence, and next edit target.
- Do not say "needs more experiments" without naming the missing comparison, measurement, or failure case.
- Do not let external research become a separate literature-review task unless the paper cannot be judged without a corpus.
- When **edit_scope=refactor** (or whole-paper judgment explicitly accepts structural cuts), do not preserve weak sections by default; cut, narrow, move to appendix, or stop defending weak claims when that is the honest route.
- When **edit_scope=surgical**, do not delete, merge, or relocate sections and do not run cross-section throughline rewrites unless the user listed that work in **scope_items** (see [`references/edit-scope-gate.md`](references/edit-scope-gate.md)).
- When **edit_scope=surgical**, do not return a **whole-section or whole-document paste** as the primary deliverable if `scope_items` only names local spans—use **patches/hunks or excerpt-to-excerpt replacements** tied to `change_id` (same gate reference).
- Do not end at critique if the user asked to get the paper closer to submission; convert findings into ordered edits.
- **审稿 R&R**：若是 **repair** 类意见，关停件应优先落在 **图/表/方法/统计/附录/补充材料** 的可核验修改（或已定稿的补充实验落点），不得把「只改摘要、只加长 hedge」当主交付；见 [`references/claim-evidence-ladder.md`](references/claim-evidence-ladder.md) §「审稿意见 / R&R」。
- Do not present "top-tier" as a style problem. Treat it as a selective-venue
  acceptance problem: novelty, evidence, comparison fairness, venue fit, and
  reproducibility must survive before prose polish matters.
- Do not allow claim drift across rounds: every rewrite must stay inside the
  frozen claim ceiling unless the main decision lane explicitly reopens it.
- Do not treat **claim downgrade / 缩口径** as the default fix when blockers
  are **B 类需补**且存在合理的 **evidence-first** 路径；先列出最小补证据/补分析
  选项，再讨论降主张（见 [`references/claim-evidence-ladder.md`](references/claim-evidence-ladder.md)）。
- **代码/实现质疑**不是「措辞问题」：禁止用泛泛公开承诺、`upon request`
 、或复述「我们相信实现正确」代替 **可核验复现锚**（环境与版本、最小命令、与算法叙述对齐）；细则见阶梯文 **§代码/实现质疑**。
- **数学/推导质疑**不是「文风问题」：禁止用直觉句、Notation 洗牙或把 Wrong proof
  悄悄收成「非正式叙述」来回避；必须 **补证明 / 定理勘误 / 反例收窄 / 或为 conjecture
  并改 claim**；细则见阶梯文 **§数学/推导质疑**。
- **R&R / repair 类意见**：closure 工件优先落在 **图/方法/统计/附录** 等可核验改动，而非仅靠 abstract 层面 hedge；见 [`references/claim-evidence-ladder.md`](references/claim-evidence-ladder.md) §审稿意见 / R&R。
- Keep this front door thin: if a rule needs more than one sentence, link the
  owning reference instead of restating it here.

## Top-tier submission bar

For requests about 顶刊, 顶会, CCF-A, Nature/Science/Cell, NeurIPS/ICML/ICLR, or
similar selective venues, apply
[`references/top-tier-paper-standard.md`](references/top-tier-paper-standard.md)
before choosing the final lane.

The short rule is:

```text
top-tier readiness = target venue contract + defensible contribution +
closest-work separation + decisive evidence + reviewer attack plan + clean
manuscript surfaces
```

If any upstream scientific bar fails, the next honest move is new evidence,
claim narrowing, target retargeting, or abandonment of an overclaim. Do not hide
that failure behind better English.

## Internal lane map

- strict submission judgment -> `$paper-reviewer`
- claim / novelty / evidence pressure test -> `logic mode` under `$paper-reviewer` or `$paper-reviser`
- target-journal ref corpus and story-norm extraction -> source-backed paper context here, then `$paper-writing`
- external calibration during review -> keep the main owner here or in
  `$paper-reviewer`; keep full corpus / novelty sweeps inside this paper front door
- findings-driven manuscript changes -> `$paper-reviser` (respect **`edit_scope`**)
- local prose rewrite after scope is frozen -> `$paper-writing` (default **`surgical`** unless user escalates to **`refactor`**)
- figures / tables / captions / rendered presentation -> `figure-table mode`
- notation / abbreviations / formula references -> `notation sweep`
- page/word budget -> `length budget mode`

Use [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md) when the work needs
filesystem-backed whole-paper state, frozen gate decisions, or bounded parallel
lanes.

For target-journal ref-first writing, use
[`references/ref-first-writing-workflow.md`](references/ref-first-writing-workflow.md)
as the compact workflow contract.

For the compact lane map, use
[`references/paper-lanes.md`](references/paper-lanes.md).

For the user-phrase → lane reverse lookup (maintainer reference; not a
user-facing menu), use
[`references/user-phrases-to-lanes.md`](references/user-phrases-to-lanes.md).

For the full manuscript stack map and progressive reading order, use
[`references/RESEARCH_PAPER_STACK.md`](references/RESEARCH_PAPER_STACK.md).

## What this skill should deliver

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

## Ref-first manuscript workflow

When the user wants to learn target-journal references before writing:

1. Build the 20-paper target-journal corpus and ref-learning brief as source-backed paper context under this front door.
2. Route to `$paper-reviewer` logic mode only if the corpus exposes a claim/evidence or novelty mismatch.
3. Route to `$paper-writing` for story spine, section plan, and bounded prose rewrite.
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

与前门 **Anti-bad-output**、[`references/claim-evidence-ladder.md`](references/claim-evidence-ladder.md)、[`references/research-language-norms.md`](references/research-language-norms.md) 叠加；**优先于**「少惹事、快过关」的模型默认。

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
