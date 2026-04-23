# Design Round Protocol

## Round Order

1. `DESIGN.md`
   - 先冻结设计语言
2. `design_prompt.md`
   - 当前轮 prompt 必须显式消费设计系统
3. `EVIDENCE_INDEX.json`
   - 记录 screenshot / render / html 等证据
4. `design_audit.md`
   - 审计漂移、反模式、系统一致性
5. `design_verdict.json`
   - 输出结论和下一轮动作

## Verdict Enum

- `pass`
- `minor_drift`
- `material_drift`
- `hard_fail`

## Continue Rules

- `pass`: 结束或切到下一页
- `minor_drift`: 允许局部修正后复审
- `material_drift`: 先重写 prompt 或局部重构，再复审
- `hard_fail`: 回到 `DESIGN.md` 或 prompt 约束层重开

## Suggested `design_verdict.json`

```json
{
  "status": "material_drift",
  "summary": "Palette discipline held, but component signatures drifted.",
  "must_fix": [
    "Unify card corner radius",
    "Remove secondary accent glow"
  ],
  "next_owner_skill": "frontend-design"
}
```
