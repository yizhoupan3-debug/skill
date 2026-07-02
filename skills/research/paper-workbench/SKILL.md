---

description: 论文全流程前门：自动路由 reviewer/writer lane，一站式审稿、返修、投稿。按顶刊/顶会标准把关，支持 rebuttal、R&R、cover letter 全流程。
framework_contracts:
  consumes_execution_items: false
  consumes_findings: true
  emits_execution_items: true
  emits_findings: true
  emits_verification_results: true
framework_roles:
- orchestrator
- planner
- verifier
metadata:
  platforms:
  - supported
  tags:
  - paper
  - manuscript
  - review
  - revise
  - submission
  - orchestrator
  - top-tier
  version: '1.16.0'
name: paper-workbench
scene: research
risk: medium
routing_gate: none
routing_layer: L3
routing_owner: owner
routing_priority: P2
session_start: preferred
source: project
trigger_hints:
- 改稿
- CCF-A 论文
- SCI润色
- paper review
- top-tier paper
- 严审
- 先审再改
- 全文审核
- 写 rebuttal
- 写论文
- 只改摘要
- 大面积重构
- 学术润色
- 帮我审这篇 paper
- 帮我审这篇论文
- 投稿前把关
- 按审稿意见改论文
- 整体推进这篇论文
- 根据 reviewer comments 改论文
- 精准修改
- 缩口径
- 能不能投
- 英文论文润色
- 论文写作
- 论文润色
- 顶会标准
- 顶会标准改稿
- 顶刊标准
- 顶刊标准改稿
- paper-workbench
- $paper-writing
- paper writing
- prose ladder
- section rewrite
- story card
- 润色落笔
- 论文改稿
- $paper-workbench
---
## Quick Ref
- **Purpose**: 论文全流程前门——自动路由 reviewer/writer lane，一站式审稿、返修、投稿。支持 loop mode（自动多轮对抗审稿直到收敛）
- **Entry**: 由 `$research` 统一科研前门路由到此，也可直接调用
- **Key Rules**: 默认 hostile-but-fair 审稿立场；edit_scope 门控（surgical/refactor）；审稿意见逐条关停；prose chain 自动触发；禁降 claim 逃避；**loop mode 收敛硬约束**（min_rounds=5, consecutive_stable=2）
- **Trigger**: "帮我审这篇 paper"、"改到能投"、"R&R"、"顶刊"、"先审再改"、"整体推进这篇论文"、"改稿周期"、"revision loop"
<!-- full content below; load on demand -->

# Paper Workbench

This skill is the one front door for paper work.

## 强对抗审稿默认立场（硬性）

一切审阅、返修、预判「能不能投」「顶刊是否能过」时，**不按友好读者模型**，而按**敌意审稿人 / 最坏合理解读（hostile but fair）**：专盯 **claim–evidence 缝、closest-work、复现与代码—正文对齐、推导跳步、统计与比较的公平性**。软球结论、只给情绪价值、或暗示「应该能过」而无逐条可关闭证据，视为**未执行本 skill**。

与本立场冲突的捷径（降口径逃难、rebuttal-only、代码空诺、数学直觉化、`surgical` 全局乱改等）一律以 [`references/claim-evidence-ladder.md`](references/claim-evidence-ladder.md)、[`references/edit-scope-gate.md`](references/edit-scope-gate.md) 为硬闸。

启用外研时，审稿/校准产出须满足 [`docs/architecture.md`](../../docs/routing/architecture.md) §A–B 的 **`Claims`**、**Contradiction sweep**、**Unknowns** 与可追溯 **retrieval_trace**（不能仅靠「读起来专业」的综述）；门面仍由本会话收口，细节上复用 **`@lane:reviewer`** 的 External lane shape 约定。

**宿主 hook（L4 短码）**：`router-rs` 在宿主提交命中写作/润色语境时合并 **`PAPER_PROSE_QUALITY_HOOK`**（真源 `configs/framework/PAPER_PROSE_QUALITY_HOOK.txt`，**默认开**）；手稿审稿/改稿语境可另合并 **`PAPER_ADVERSARIAL_HOOK`**（opt-in）。受 `ROUTER_RS_OPERATOR_INJECT` 总闸约束。Prose 子开关：`ROUTER_RS_PAPER_PROSE_HOOK`（unset=开，`0`=关）。Adversarial：`ROUTER_RS_PAPER_ADVERSARIAL_HOOK=1` 启用。见 [`references/prose-chain-contract.md`](references/prose-chain-contract.md) §L4。

It exists so the user does not need to decide first whether the job is
`@lane:reviewer`, `@lane:writer`, or a review/revision dimension mode.

## Progressive disclosure（渐进披露）— 减入口、减抽象

**第一性原理**：用户要的是「这篇稿子下一步怎么办」，不是背诵技能拓扑。

- **L0（默认）**：只暴露本前门与用户可理解的结果（verdict / blockers / next move / edit_scope 若即将改稿）。**不要**要求用户在 `@lane:reviewer`、`@lane:writer` 之间先选一个；在对话内自行路由。
- **L1**：用户已明确「只润色」「只审不改」「按 R1 改」或给出 `edit_scope` / `scope_items` 时，再收紧模式，仍不必展开全套维度名。
- **L2（排障 / 强用户）**：用户**点名**某专科 skill 或某维度（logic / figure-table / notation）时，直接尊重；文档里的 lane map 供实现方用，不是菜单。
- **L3（长程）**：多轮冻结 claim、并行 sidecar、[`references/paper-gate-protocol.md`](references/paper-gate-protocol.md) 磁盘树 —— **仅当**任务真的需要跨会话状态时再物化；日常一轮交互不要默认铺协议。

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
- The user says `改稿周期`, `revision loop`, `自动改到能投`, `对抗审稿循环`, or wants
  automated multi-round adversarial review until convergence → **loop mode**

## Do not use

- The user wants to advance a non-manuscript research project, topic, or experiment plan -> use `$research` (execution lane); this front door is manuscript-only
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

只要将触达手稿正文句子，**立即**走 prose chain——**不得**等用户写 `language_register` / `writing_mode` / `prose_qc`。

自动执行：
1. **推断** `language_register`（见 prose-quality-gate.md §Language register）
2. **默认** `edit_scope: surgical` + 从用户粘贴/点名推断 `scope_items`
3. **默认** `writing_mode: ladder-full` + 极简 Claim card
4. 若 claim/evidence 未冻结且用户要「能不能投」→ 先 reviewer；**纯改文字**则 Claim card 后直写

全链路真源：[`references/prose-chain-contract.md`](references/prose-chain-contract.md)。

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

**`PAPER_PROSE_QUALITY_HOOK` 默认开**，见 prose-chain-contract §L4。

## Anti-bad-output rules

[详细规则](references/anti-bad-output-rules.md)

## Top-tier submission bar

[Top-tier bar 详](references/top-tier-bar-summary.md)

## Internal lane map

[Internal lane map (maintainer-only)](references/internal-lane-map.md)

## What this skill should deliver

本前门转发或收口 **`@lane:writer`** 的改稿时，统一输出顺序须先回声门控与叙事契约，再贴正文块。
详细规格见 [`references/prose-quality-gate.md`](references/prose-quality-gate.md) 与 [`references/edit-scope-gate.md`](references/edit-scope-gate.md)。

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

## Verification and closeout

Before claiming a revision is done, verify that all `edit_scope` items are addressed and the claim ledger reflects actual changes. Ensure evidence rows and verification commands confirm the revision.

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

与前门 **Anti-bad-output** 叠加。详见 [`references/review-hard-rules.md`](references/review-hard-rules.md)。
**核心原则**：禁止降 claim 逃避、禁止防御口径顶替改稿、禁止 rebuttal-only、逐条关停、默认正面硬修。
代码与数学类意见默认归入 `repair` 主轴。

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
- `@lane:reviewer` prose quality checks → [`../../../quality-gates/prose-verification/SKILL.md`](../../../quality-gates/prose-verification/SKILL.md)
- `@lane:reviewer` structure/logic checks → [`../../../quality-gates/structure-verification/SKILL.md`](../../../quality-gates/structure-verification/SKILL.md)
- Literature/citation integrity checks → [`../../../quality-gates/literature-verification/SKILL.md`](../../../quality-gates/literature-verification/SKILL.md)
- Statistical methodology checks → [`../../../quality-gates/statistical-verification/SKILL.md`](../../../quality-gates/statistical-verification/SKILL.md)

## Upstream skill integration: good-story

When the user provides **raw results, figures, data summaries, or early drafts** before the manuscript's story is settled, call `$good-story` first to produce a Story Card before entering the review/revision pipeline.

**When to route upstream**:
- User has results/tables/figures but no clear manuscript direction → `$good-story` for story diagnosis first
- User has a draft but the story is weak or unfocused → `$good-story` for story spine diagnosis, then feed the Story Card into `@lane:reviewer`
- User asks for "story" or "narrative" → route to `$good-story` as the owner, not paper-workbench

**Consuming the Story Card**:
The Story Card (output of `$good-story`) provides structured input for paper-workbench:
- `Best story` / `Why this story works` → shapes the reviewer's claim-evidence evaluation
- `Evidence map` (claim → evidence → boundary) → seeds `@lane:reviewer` evidence-chain checks
- `Weak points` → pre-populates reviewer's finding list for verification
- `Figure order` → guides `@lane:reviewer` figure/table audit depth

**Boundary**:
- `$good-story` replaces paper-workbench when the task is "find the story" not "review/rewrite an existing draft"
- If a draft exists AND story is unclear, run `$good-story` first, then paper-workbench on the rewritten manuscript
- Do not route to `$good-story` when: the manuscript is already structured and the user only wants review/rewrite without restructuring the narrative

**Downstream**:
- `$good-story` does not require paper-workbench; it can work standalone for users who only need story diagnosis
- paper-workbench remains the front door for manuscript submission/review/revision; `$good-story` is a pre-processing step

## Research Harness MCP tools（Rust 加速路径）

论文审稿/返修流程可通过 `research-harness` crate 的 MCP tools 加速。
这些 tools 在 `host-projection` 的 `mcp_stdio_harness` 中注册，可直接调用：

| MCP Tool | 用途 | 输入 | 输出 |
|----------|------|------|------|
| `research_review_dimensions` | 获取审稿维度 prompt + checklist | `round: u64` | 该轮维度的审稿 prompt 和 checklist |
| `research_aigc_check` | AIGC 检测 | `text: string` | 0-100 AI 概率评分 + 信号列表 |
| `research_aigc_humanize (暂未实现)` | AIGC 降重（句法改写/词汇替换） | `text: string` | 重写后的文本 + 策略列表 |
| `research_review_loop` | 对抗审稿循环管理 | `operation: start/submit_round/status` | 收敛状态、下一轮维度 |
| `research_claim_drift` | 声明漂移检测 | `original_claims: [], current_claims: []` | drift 分析结果列表 |

**使用时机**：
- **审稿维度 prompt**：loop mode 每轮 reviewer spawn 时，用 `research_review_dimensions(round)` 获取精确的审稿 prompt 和 checklist，替代手动构建
- **对抗审稿循环**：用 `research_review_loop` 管理多轮审稿循环（启动、提交审稿轮次、查询收敛状态），避免手动跟踪循环进度
- **声明漂移检测**：改稿前后用 `research_claim_drift` 检测主张是否悄悄漂移，确保 claim 在 revision 链中保持一致
- **AIGC 检测**：投稿前检测手稿 AI 概率，或在 prose chain 中作为 QC 步骤
- **AIGC 降重**：对高 AI 概率段落执行句法改写，降低 AIGC 检测风险

## AIGC 检测与降重（可选步骤）

在投稿前或 prose chain 中，可对论文正文执行 AIGC 检测和降重：

### 检测流程

```
1. 将论文正文按段落/句子分割
2. 对每个片段调用 research_aigc_check → 获取 0-100 评分
3. 评分阈值：
   - 0-30: 安全（低 AI 概率）
   - 30-60: 注意（中等 AI 概率，建议人工复查）
   - 60-100: 高风险（高 AI 概率，强烈建议降重）
4. 对高风险片段执行降重
```

### 降重策略

调用 `research_aigc_humanize (暂未实现)` 对高风险文本执行以下策略：
- **词汇替换**：AI 高频词汇 → 学术常用替代（Moreover → Additionally, Leverage → Use）
- **句法改写**：主动/被动变换、从句重组
- **句式多样化**：注入长短句交替，打破 AI 文本的均匀节奏
- **保持学术语气**：不降低学术规范性

## Loop mode（自动多轮对抗审稿 — 直到收敛）

### 路由规则

| 用户意图 | 模式 | 行为 |
|---------|------|------|
| 默认（不指定维度）：帮我审 / 改到能投 / 投稿前把关 / 整体推进 | **loop mode** | 自动多轮对抗审稿 → 修复 → 再审，直到收敛 |
| 指定单一维度：只审语言 / 只审逻辑 / 只审图表 | **single-dimension** | 单轮审查该维度，不循环 |
| 明确说"loop" + 维度：只审语言但 loop / 改稿周期 | **loop mode** | 进入 loop，该维度作为第一轮，后续轮次渐进扩展 |
| 明确说"loop" + 无维度：改稿周期 / revision loop | **loop mode** | 从头开始完整 loop |

### Loop mode 核心流程

```
Step 1: 启动对抗审稿循环
  research_review_loop(operation=start,
    max_rounds=10,
    min_rounds=5,
    consecutive_stable_required=2)

Step 2: 多轮循环（一次只 spawn 一个 reviewer subagent）
  FOR round = 1 TO 10:

    # 2a: Spawn reviewer subagent
    spawn_agent(
      role="hostile {target_venue} reviewer",
      prompt="""
        本轮审查维度：{dimension_for_round(round)}
        渐进披露：每轮一个新维度，不暴露总轮数。
        严格按 severity-spec 分级（P0/A/B/Warning/C）。
        只报告确实找到的、有具体位置的问题。
        输出 JSON：{findings: [...], verdict, dimension_covered}
      """
    )

    # 2b: 主会话修复（surgical edit_scope）
    基于 findings 执行改稿

    # 2c: 提交轮次审稿发现
    research_review_loop(operation=submit_round,
      round=round,
      findings=[...adversarial_findings],
      max_rounds=10)

    # 2d: 收敛检查
    IF 无新 A/B 级 findings AND round >= min_rounds → stable_count++
    ELSE → stable_count = 0
    IF stable_count >= consecutive_stable_required → BREAK（收敛）

Step 3: 关闭 — 检查收尾就绪并写入 Closeout Record
  closeout_gate(task_id=<task_id>)
  closeout_record_write(task_id=<task_id>,
    summary="Paper revision converged after N rounds",
    verification_status="passed",
    changed_files=[...],
    blockers=[],
    risks=[])

Step 4: 输出收敛报告（verdict + 每轮 findings 摘要 + 改动统计）
```

### 渐进披露维度（7 维度循环）

| 轮次 | 审查维度 | 子维度 |
|------|---------|--------|
| R1 | **逻辑与证据** | claim ceiling, evidence coverage, ablation isolation, comparison fairness |
| R2 | **最近工作与新颖性** | closest prior work gaps, novelty positioning, venue calibration |
| R3 | **数学与符号** | equation closure, symbol uniqueness, derivation gaps, overmath |
| R4 | **图表与可读性** | figure rendering, caption self-containment, table density |
| R5 | **语言与防御性** | terminology density, defensive tone, EN slop / ZH 套话 |
| R6 | **长度与附录路由** | page pressure, hidden evidence, appendix routing |
| R7+ | **全面重审** | 所有维度一起审，验证前几轮修复无回归 |

### 收敛定义（硬约束）

```yaml
convergence:
  min_rounds: 5                     # 硬约束：至少跑满 5 轮
  consecutive_stable_required: 2    # 连续 2 轮无新 A/B/P0 findings
  max_rounds: 10                    # 硬上限
  stable_definition:
    - no_new_P0: true               # 无新 P0（一票否决）发现
    - no_new_A: true                # 无新 A（核心硬伤）发现
    - no_new_B: true                # 无新 B（需补充）发现
    # C 级和 Warning 不阻塞收敛（记录但不阻止关闭）
  diminishing_returns:              # 可选辅助退出
    threshold: 3                    # 连续 3 轮只发现 C/Warning → 提示用户
```

Quality Gate 引擎在 Rust 层强制执行 `min_rounds` 和 `consecutive_stable_required`——即使 supervisor（LLM）传入 `supervisor_decision: "close"`，round < min_rounds 时仍被拦截为 "active"。

### Subagent 约束

- **每次只 spawn 1 个 reviewer subagent**（不并行）
- reviewer subagent 不知道总轮数（渐进披露）
- reviewer subagent 不知道前几轮的 findings（防止锚定偏差）
- 主会话负责修复（不 spawn fixer subagent）
- 收敛后写 closeout record 供 goal-engine 验证

## Exit Criteria

- verdict 已输出（accept / revise / reject + claim-evidence ladder 完整）
- edit_scope 已门控（仅限 declared scope 内的文件）
- 用户已确认下一步动作（提交修改 / 补充实验 / 放弃）
