# Design Artifact Contract

推荐最小工件集：

## Source of truth

- `DESIGN.md`
  - 设计语言唯一真源

## Current design lane

- `artifacts/current/design/design_prompt.md`
  - 当前轮用于生成或修改的设计提示词
- `artifacts/current/design/design_targets.md`
  - 当前轮页面目标、范围、守住什么、允许变什么
- `artifacts/current/design/design_audit.md`
  - 本轮设计验收结果
- `artifacts/current/design/design_verdict.json`
  - 机器可读结论，如 `pass` / `minor_drift` / `material_drift` / `hard_fail`

## Evidence registry

- `EVIDENCE_INDEX.json`
  - 记录 screenshot、render、HTML、对比图路径

## Ownership

- `design-md` 维护 `DESIGN.md`
- `design-prompt-enhancer` 维护 `design_prompt.md` 与 `design_targets.md`
- `visual-review` / browser evidence 负责产出截图与可视证据
- `design-output-auditor` 维护 `design_audit.md` 与 `design_verdict.json`

## Invariants

- 不允许多个文件同时充当设计系统真源
- 不允许把 audit 结论只留在聊天里
- 不允许 screenshot 路径只散落在文字总结里
