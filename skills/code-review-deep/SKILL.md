---
name: code-review-deep
description: |
  Deep adversarial-style code review (review-only). Default visible output is a compact, severity-sorted findings list; narrative sections only when explicitly requested.
  Model selects lenses from an extensible catalog (core + optional: first principles/subtraction, dead-code signals, stale docs); exhaustive within chosen lenses.
  Broad/deep/PR-level work authorizes read-only independent reviewer subagents (fork_context=false) before main-thread synthesis. Does not silently rewrite implementation
  unless the user explicitly exits review-only posture.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: preferred
user-invocable: true
disable-model-invocation: false
trigger_hints:
  - $code-review-deep
  - code-review-deep
  - review
  - code review
  - 代码审查
  - 帮我 review
  - deep code review
  - 深度 code review
  - 深度代码审查
  - 严苛代码评审
  - security code review
  - security-focused code review
  - threat model review
  - adversarial code review
  - 只允许审不改
  - review-only 代码审查
  - CVE 审查
  - dependency audit PR
  - supply chain review
  - 供应链安全
metadata:
  version: "1.2.3"
  platforms: [supported]
  tags: [code-review, security, correctness, delegation, adversarial-review]
framework_roles:
  - detector
  - planner
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: true
risk: medium
source: local
---

# Code review (deep owner)

Judgment-focused review for code and change sets **without** rewriting by default. Portable across repositories: do **not** assume framework-specific files or audit commands exist unless the workspace is this skill/harness repo and the user’s scope includes it.

## Default posture

- **Findings-only by default (hard stop)**: On a review request, **do not** edit files, add tests, run fix commits, open PRs, or continue into implement / `/implementx` / gitx / loop unless the user **explicitly** exits review-only in the same or a follow-up message (e.g. fix these findings, implement, merge, commit). End with findings (+ optional one-line verdict), not execution.
- Assume a **hostile but fair** reviewer: maximize plausible failure under real abuse, regressions,
  flaky ops, closest prior API expectations, dependency churn, or incomplete tests.
- **Analysis standard is unchanged**: still choose lenses internally, still exhaust findings **within each lens you selected**, still apply the severity evidence gate below. **Compact default output means less prose in chat, not shallower reasoning.**
- **Lens catalog, not a fixed runway**: choose lenses from [`references/review-dimensions.md`](references/review-dimensions.md). **Do not** treat every review as “must run every row.” **Do** systematically exhaust findings **within each lens you selected**.
- When the user explicitly asks to **cover all dimensions** / **exhaust every lens** / **全维度**, apply the full catalog **and** use the **full report profile** (see Deliverable shape); evidence rules for P0/P1 stay the same.

## Compact envelope（硬性，宿主可见）

Rules for **everything the host/user sees** in chat under **default compact**—not for **internal** lens reasoning.

- **Severity line prefixes**: Except for a single-line **`Caveat:`** row (see below), **every** finding line **must** start with **`[P0]`**, **`[P1]`**, or **`[P2]`**. A **caveat / open question** may use **`[P2]`** plus a short parenthetical that evidence was downgraded **or** one line starting **`Caveat:`**—**equivalent** for “first finding line” and ordering (**P2 / caveat** bucket) below.
- **Prefix block (only before the first `[P0]` / `[P1]` / `[P2]` / `Caveat:`)**:
  - **With `Scope:`**: **Exactly one** line `Scope: …`. Optionally **one** more line **`Out of scope: …`** (single line only). The **very next** line **must** be the first finding (`[P0]` / `[P1]` / `[P2]` / `Caveat:`). **No** third prelude line, **no** tables, **no** “小结 / 分类 / 属于哪一类” headings between `Out of scope:` and that finding.
  - **Without `Scope:`**: The **first** host-visible line **must** be the first finding (`[P0]` / `[P1]` / `[P2]` / `Caveat:`). Do **not** use a standalone **`Out of scope:`** line ahead of findings—fold that note into the first finding or into your single `Scope:` line if you add one.
- **Forbidden before the first `[P*` / `Caveat:`** (other than the **`Scope:`** / optional **`Out of scope:`** lines above): Markdown **tables**; section headings whose role is **summary / 小结 / 分类 / 属于哪一类 / taxonomy** plus long prose; multi-sentence “scene setting.” **Lens work stays implicit** unless the user asks for grouping, lens tables, or **full report profile**.
- **Verdict**: at most **one line**, **only after** the complete findings list in the same reply. Optional aggregate **`test/repro gap`** stays **≤ one line** after verdict—or folded into residual-risk—not as preamble.
- **Exception**: Only in **full report profile**—user explicitly asks for PR narrative, lens-by-lens tables, categorical summaries, **`Scope/Lenses/Omitted`**, audit-style sections, etc.

## Output profiles

### Default compact output (unless the user asks for narrative / lenses table / PR-style report)

- **Envelope**: Obey the **Compact envelope** section above.
- One list sorted **globally** as **P0 → P1 → P2 → caveat / open question** (within each level, rank by blast radius / confidence / affected surface).
- **Do not default** to a separate **Scope / Lenses / Omitted** block. **Prefix** rules: optional **one** line `Scope: …`; **if** you use `Scope:`, you may add **at most one** line `Out of scope: …`, then **immediately** the first **`[P*]` / `Caveat:`** line (see **Compact envelope**). **Without** `Scope:`, do **not** lead with standalone `Out of scope:`.
- **Verdict**: optional **at most one line** (`blocked | revise before merge | ship with caveats`), **after** findings **only** (never leading the reply).
- **Do not group findings by lens** in chat unless the user asks for grouping by lens or full audit trail.
- **Each finding**: one tight line plus optional indented evidence; minimal structure —
  **`[Pn] path:anchor`** — issue — impact / exploitability — smallest verification or missing test (aligns with **Severity evidence gate**). **Caveat / open question** lines: prefer **`[P2]`** with downgrade note, or **`Caveat:`** as defined in **Compact envelope**—same evidence rules.

### Full report profile (explicit triggers only)

Use **only** when the user asks for **`Scope/Lenses/Omitted`**, **lens-by-lens sections**, **PR / 述职叙事**, categorical deliverables (**类型 + 说明** matrices), **`属于哪一类` taxonomy**, **Markdown summary tables** as the artifact, **exhaust every lens**, **audit-style report**, or other **explicit narrative**. Vague 「有什么问题」「全面review」**alone** stays **compact**—do **not** treat them as opting into this profile.

Then you may use a preamble (**Scope**, **Lenses**, **Omitted**), **`verdict`**, findings **grouped by lens**, then **`test / repro gap`**, optional **`external calibration`**, **`next move`**—same rigor, richer packaging.

## Lane contracts

### Model contract (Cursor / Task)

- **`Task` / parallel subagents**：**omit** `model` (inherit parent session). **Do not** set `claude-*` / `sonnet*` / `anthropic` unless the parent session already uses that provider.
- If subagents fail with `Model not available` / `not supported in your region`: retry with explicit `Task` (no `model`) on lanes below, or main-thread review; do not treat as permanent «subagent unavailable».
- Applies to **`explore`** and all lanes—not only deep gate lanes.

For broad/deep/PR-level code review, use **spawn-first pairing**: before the main thread’s first tool call, spawn **at least one** parallel read-only reviewer (`fork_context=false`, lane ∈ `deep_gate_lanes`; Cursor 可选 `Task` + `subagent_type=deep-reviewer`，见 [`.cursor/agents/deep-reviewer.md`](../../.cursor/agents/deep-reviewer.md)）。 If the main thread will run **explore**/research, still spawn a **separate** reviewer lane—**explore does not count** as review evidence. For breadth/PR/cross-module prompts, prefer **≥2** lanes (≥3 when breadth signals stack), split by disjoint lens bundles, before main-thread compact synthesis. Hooks inject only a **one-line** spawn-first pointer (registry `review_gate.spawn_first_nudge`); on **Cursor**, a separate **model-inherit** line may also appear unless spawn-first already includes it (dedup by spawn-first line only); details stay in this file.

**Narrow scope** (single-path `review ./file`, `small_task`, or explicit「不用子代理」): **no** multi-lane requirement; hosts skip arming `review_required`—**must not** Stop-block for missing subagents.

**REVIEW_GATE clearance (Cursor)**: requires countable reviewer evidence per wave-2 (`start_count≥1`, multiset drained, no compact-alone forgery)—**not** raised to `≥2`. Main thread delivers **compact** findings only; optional `artifacts/current/<task_id>/review-lanes/<lane_id>.md` (soft for Cursor—missing files **do not** block Stop; hard requirement for Antigravity). Keep reviewers read-only and artifact-disjoint. `lifecycle_profile: my-light` does **not** hard-block Stop on REVIEW_GATE.

**CODEX_REVIEW_GATE clearance (Codex CLI, wave-2 partial)**: PostTool 深度 lane（`deep_gate_lanes` + `fork_context=false`，缺字段推断用 **`ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`**，默认 on）→ 可数证据、`phase≥2`；Stop 上 compact findings **仅在有可数证据时** 升 `phase=3`；`rg_clear` / bounded reject token 亦可清门；**compact alone 不得清门**。无 subagentStart/Stop multiset。`my-light` / `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` 关闭硬拦。

**CLAUDE_REVIEW_GATE clearance (Claude Code)**: `PostToolUse` 上观察到 `claude_reviewer_lanes`（`deep_gate_lanes` 四拼写 + `review`/`reviewer`/`critic`/`code-review`）且 **`fork_context` 解析为逻辑 `false`**（JSON 布尔；可选 `ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE=1` 在字段缺失时推断 `false`）→ 置 `independent_reviewer_seen`；**re-arm** review 时重置该标志。`Stop` 在 `independent_reviewer_seen` 前硬拦。**无** Cursor/Codex 式 `rg_clear`、wave-2 compact phase 或 subagent multiset；**`explore` 不计入**。`lifecycle_profile: my-light` / `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE=1` 关闭硬拦。

**ANTIGRAVITY_REVIEW_GATE clearance（分宿主）**:

- **`claude-code`（终端 hooks）**: Codex 式 — `PostToolUse` 可数 deep lane + `Stop` compact；**无** Cursor `subagentStart`/`subagentStop` shell 事件。Stop 短码/env 前缀 `ANTIGRAVITY_CLI_REVIEW_GATE` / `ROUTER_RS_ANTIGRAVITY_CLI_REVIEW_GATE_*`（见 `antigravity_cli_hooks`）。`my-light` 关闭硬拦。
- **`claude-desktop`（MCP / Desktop）**: **无** shell hook 表。Findings **必须**写入 `artifacts/current/<task_id>/review-lanes/*.md`（或 `closeout_gate` + `review_evidence_attested=true`）；MCP `complete`/`closeout_gate` 可触发 `[Antigravity Hard Block]`。`my-light` 关闭硬拦。

**Host countable evidence (Cursor / Codex `REVIEW_GATE` / Codex Stop ledger)** matches `hook_common::is_deep_review_gate_lane_normalized`: the subagent lane (after host normalization) must be in `configs/framework/RUNTIME_REGISTRY.json` → `review_gate.deep_gate_lanes` only (`general-purpose` / `best-of-n-runner` / `deep-reviewer` and normalized equivalents — see `docs/host_adapter_contract.md` §0.1). **`explore`, `ci-investigator`, `cursor-guide`, `review`/`reviewer`/`critic`/`code-review`, and other custom lane names not listed in registry do not count** on Cursor/Codex—even with **`fork_context=false`**. **Claude Code** uses `review_gate.claude_reviewer_lanes` (superset); do not assume those extra strings satisfy Cursor/Codex hooks.

Lane outputs must cite **locations** (paths + anchors / symbols where possible).

**Framework-repo optional evidence** (only when this workspace is this harness/skill framework repository and scope touches it): you may cite local checklists or `router-rs framework maint` audit-style commands as **read-only** evidence—never as a dependency for reviews of other codebases.

## External / network research lane (optional but recommended)

Use only when the user allows network/tools or the scope touches third-party crates/services or known vulnerability classes. When marking work “deep external,” prefer the **full report profile** for the calibrated section.

**If you stay in default compact** (user did **not** opt into **full report profile**): do **not** place **Claims / Contradiction / Unknowns / Retrieval_trace** (or RFV §A–B **headings**) **before** the first **`[P0]` / `[P1]` / `[P2]` / `Caveat:`** line. After the findings list begins, external material **may** appear only as **(a)** indented bullets **under** the specific **`[P*]` / `Caveat:`** line they support, or **(b)** plain continuation (no new H1/H2) **immediately after the last finding line** and **before** the optional **one-line** `verdict`—still **no** standalone “Claims / Contradiction …” **section headers** and **no** Markdown tables in that gap. **Do not** insert a four-part **Claims / Contradiction / Unknowns / Retrieval** chapter between findings and `verdict` unless the user has opted into **full report profile**.

When marking work “deep external” **and** the user accepts **full report profile**, you may use the heading block in the preamble per that profile.

### External checklist (full report template only)

The following bullets apply **only** in **full report profile** (or an explicit preamble the user requested for external calibration)—**not** as a default tail to paste after compact findings:

- Produce **Claims** backed by citations (changelog URL, GitHub Advisory ID, CVE, release notes DOI/issue).
- **Contradiction sweep**: cite evidence that contradicts or limits each high-confidence Claim.
- **Unknowns**: what still cannot be asserted from reachable evidence alone.
- **Retrieval_trace** (minimal): queries / sources scanned, inclusion/exclusion heuristic, stale assumptions rejected.

Structured output expectations align with
[`docs/references/rfv-loop/reasoning-depth-contract.md`](../../docs/references/rfv-loop/reasoning-depth-contract.md) §A–B (same headings whenever you mark work as “deep external,” even outside an RFV ledger).

## Severity evidence gate

- **P0/P1 requires evidence**: include at least one of a concrete call chain, a repro path, a checked test gap, or a cited external advisory/source. Without that, downgrade to P2, caveat, or open question.
- **No hollow findings**: every finding must include path + symbol/line anchor, user or operational impact, and the smallest verification or missing test that would confirm it.
- **Testing honesty**: if tests were not run, say so compactly once (footer of findings or residual-risk line) and name the residual risk.
- **Security claims**: state exploitability or blast radius; speculative abuse without a reachable path is a caveat/open question, not a blocker.

## 安全审计维度

参考 Trail of Bits 安全审计方法论，在代码审查中增加安全检查。

### OWASP Top 10 检查清单
- 注入（SQL/NoSQL/OS/LDAP）：检查用户输入是否参数化或转义
- 认证缺陷：session 管理、密码存储、MFA 绕过
- 敏感数据暴露：日志中是否打印 secrets/API keys/tokens
- XXE：XML 解析是否禁用外部实体
- 访问控制缺陷：IDOR、权限提升、水平越权
- 安全配置错误：默认密码、debug 模式、CORS 策略
- XSS：输出编码、CSP 头、DOM 操作
- 不安全反序列化：untrusted data 反序列化
- 含已知漏洞的组件：依赖版本检查
- 日志和监控不足：安全事件是否被记录

### 依赖安全
- package.json / requirements.txt / Cargo.toml 已知漏洞
- 锁定版本 vs 范围版本
- 供应链攻击防护（lockfile 完整性）

### Secret Detection
- 硬编码 credentials、API keys、tokens
- .env 文件是否在 .gitignore
- 配置文件中的敏感值

### 报告格式
- 安全发现按严重程度分级：Critical / High / Medium / Low / Info
- 每个发现包含：位置、描述、影响、修复建议
- 与代码质量发现分开报告

## Deliverable shape

**Default (compact)** — **top to bottom** for host-visible text:

1. **Optional prefix** (see **Compact envelope**): **zero to two** lines only—**`Scope:`** (optional), then optionally **one** **`Out of scope:`** line **only if** you already used `Scope:`. **No** other lines before findings.
2. **`Findings`**: single list, severity order **P0 → P1 → P2 → caveats**, each item evidence-gated as above; the first **`[P*` / `Caveat:`** line must come **immediately after** the prefix (no tables, no “小结/分类” sections in between).
3. Optional **one-line** `verdict` **after** that list.
4. Optional **one-line** `test/repro gap`; omit if each finding already carries verification.

**Full report profile** — explicit triggers only (see **Output profiles**):

0. Scope / Lenses / Omitted (or equivalent narrative opener when user asked for taxonomy).
1. `verdict` (one line).
2. Findings grouped by applied lens with P0–P2 tags.
3. `test / repro gap`.
4. `external calibration` (if external lane used).
5. `next move` (implementer handoff).

## Integration / boundaries

- If the task is repo closeout Git operations, `$gitx` still owns staging history; reuse this lane for substantive diff critique only.
- If the artifact is screenshots or rendered UI decks, `$visual-review` complements but does not replace correctness/security lanes.
- If the user needs **paper/manuscript** judgment or **GitHub PR comment triage** as the primary task, prefer the narrower owners (`paper-workbench`, `gh-address-comments`, etc.) when routing applies.
