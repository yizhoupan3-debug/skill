# Internal lane map（maintainer-only）

> 本文档原位于 `skills/paper-workbench/SKILL.md` L316-345（30 行 maintainer 视图 + 6 条指针），下沉到 `references/` 以压缩 front door。Front door 入口见 `SKILL.md` `## Internal lane map` 段 1 行指针。
>
> 本节是 **maintainer 视图**：列出每种任务形态如何路由到 `paper-workbench` 内部的 lane / mode / 下游 reference；**不是**用户菜单。L0 用户入口见 `SKILL.md` `## Use this when` / `## Do not use`。

## 路由图

- strict submission judgment -> `@lane:reviewer`
- claim / novelty / evidence pressure test -> `logic mode` under `@lane:reviewer`
- target-journal ref corpus and story-norm extraction -> source-backed paper context here, then `@lane:writer`
- external calibration during review -> keep the main owner here or in
  `@lane:reviewer`; keep full corpus / novelty sweeps inside this paper front door
- findings-driven manuscript changes -> this front door's inline revision (respect **`edit_scope`**)
- local prose rewrite after scope is frozen -> `@lane:writer` (default **`surgical`** unless user escalates to **`refactor`**); **须**先设 **`language_register`** 并走 [`prose-quality-gate.md`](prose-quality-gate.md)（见下 §Prose quality intake）
- figures / tables / captions / rendered presentation -> `figure-table mode`
- notation / abbreviations / formula references -> `notation sweep`
- page/word budget -> `length budget mode`

## 协议与工作流指针

Use the gate-protocol workflow when the work needs
filesystem-backed whole-paper state, frozen gate decisions, or bounded parallel
lanes (protocol spec in `SKILL.md` §Gate protocol).

For target-journal ref-first writing, use
[`ref-first-writing-workflow.md`](ref-first-writing-workflow.md)
as the compact workflow contract.

For the compact lane map, use
[`paper-lanes.md`](paper-lanes.md).

For the user-phrase → lane reverse lookup (maintainer reference; not a
user-facing menu), use
[`user-phrases-to-lanes.md`](user-phrases-to-lanes.md).

For the full manuscript stack map and progressive reading order, use
[`RESEARCH_PAPER_STACK.md`](RESEARCH_PAPER_STACK.md).
