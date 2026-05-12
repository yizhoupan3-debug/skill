# AGENTS.md 与 `.cursor/rules` 重叠叙事：维护者 rg 自检

**目的**：在修改 [`AGENTS.md`](../../AGENTS.md) 或 [`.cursor/rules/`](../../.cursor/rules/) 前，发现**可能**的重复表述。**命中≠必须删**：`.mdc` 可保留 Cursor 独有硬约束；仅收敛真正的跨宿主复读。

**不变量**：跨宿主协议以 `AGENTS.md` 为准；`.cursor/rules/*.mdc` 只补充 **Cursor 独有**差异（见 `AGENTS.md`「Execution Ladder」中关于 Cursor 的条目）。

**在仓库根执行**：

```bash
rg -n "Execution Ladder|REVIEW_GATE|fork_context|subagent" AGENTS.md .cursor/rules/
rg -n "CreatePlan|plan_profile|gitx plan" AGENTS.md .cursor/rules/
rg -n "hooks_context|SessionStart|hook-state" AGENTS.md .cursor/rules/
rg -l 'AGENTS\.md' .cursor/rules/
```

**解读**：长段策略若在两处字面重复，优先只留 `AGENTS.md`；`.mdc` 用「详见 `AGENTS.md` …」单句指针。已标明「仅 Cursor」且语义更窄的 `.mdc` 段落可保留。

**相关**：[`docs/README.md`](../README.md)「按主题」表；[`docs/plans/cursor_cross_workspace_operator_checklist.md`](cursor_cross_workspace_operator_checklist.md) 第 2.1 节。
