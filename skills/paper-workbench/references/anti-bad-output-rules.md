# Anti-bad-output rules

> 本文档原位于 `skills/paper-workbench/SKILL.md` L267-295（28 行 hard rule bullets），下沉到 `references/` 以压缩 front door。Front door 入口见 `SKILL.md` `## Anti-bad-output rules` 段 1 行指针。
>
> 真源：本文件 = canonical 副本；任何更新须先改此处，再把指针同步到 `SKILL.md`。

## Hard rules

- Do not start with language polish when claim/evidence, novelty, baseline, or target-venue fit is unresolved.
- Do not run English slop rules on Chinese paragraphs (or vice versa) without `language_register: mixed` and per-anchor labeling.
- Do not deliver long polished paragraphs without **`prose_qc`** and ladder L1–L4 pass (or explicit `ladder_blocked` with outline-only).
- Do not give a long review taxonomy before the verdict; lead with verdict, then findings appropriate to **`audit_depth`** (full dimension list for exhaustive; top blockers for compact).
- When **`audit_depth: exhaustive`**, do **not** truncate to "top 3" or "top blockers" — use the envelope in [`paper-exhaustive-audit.md`](paper-exhaustive-audit.md).
- Do not say "needs more experiments" without naming the missing comparison, measurement, or failure case.
- Do not let external research become a separate literature-review task unless the paper cannot be judged without a corpus.
- When **edit_scope=refactor** (or whole-paper judgment explicitly accepts structural cuts), do not preserve weak sections by default; cut, narrow, move to appendix, or stop defending weak claims when that is the honest route.
- When **edit_scope=surgical**, do not delete, merge, or relocate sections and do not run cross-section throughline rewrites unless the user listed that work in **scope_items** (see [`edit-scope-gate.md`](edit-scope-gate.md)).
- When **edit_scope=surgical**, do not return a **whole-section or whole-document paste** as the primary deliverable if `scope_items` only names local spans—use **patches/hunks or excerpt-to-excerpt replacements** tied to `change_id` (same gate reference).
- Do not end at critique if the user asked to get the paper closer to submission; convert findings into ordered edits.
- **审稿 R&R（repair）**：关停件须落在可核验的手稿/图表/方法/统计/附录改动，不得以摘要 hedge 或措辞替代；细则与「审稿意见 / R&R」条款只信 [`claim-evidence-ladder.md`](claim-evidence-ladder.md)（下文 §审稿意见与之对齐，不重复扩写）。
- Do not present "top-tier" as a style problem. Treat it as a selective-venue
  acceptance problem: novelty, evidence, comparison fairness, venue fit, and
  reproducibility must survive before prose polish matters.
- Do not allow claim drift across rounds: every rewrite must stay inside the
  frozen claim ceiling unless the main decision lane explicitly reopens it.
- Do not treat **claim downgrade / 缩口径** as the default fix when blockers
  are **B 类需补**且存在合理的 **evidence-first** 路径；先列出最小补证据/补分析
  选项，再讨论降主张（见 [`claim-evidence-ladder.md`](claim-evidence-ladder.md)）。
- **代码/实现质疑**不是「措辞问题」：禁止用泛泛公开承诺、`upon request`
  、或复述「我们相信实现正确」代替 **可核验复现锚**（环境与版本、最小命令、与算法叙述对齐）；细则见阶梯文 **§代码/实现质疑**。
- **数学/推导质疑**不是「文风问题」：禁止用直觉句、Notation 洗牙或把 Wrong proof
  悄悄收成「非正式叙述」来回避；必须 **补证明 / 定理勘误 / 反例收窄 / 或为 conjecture
  并改 claim**；细则见阶梯文 **§数学/推导质疑**。
- Keep this front door thin: if a rule needs more than one sentence, link the
  owning reference instead of restating it here.
