# CodeGraph 操作式指南（共享引用）

CodeGraph 八工具经 `mcp-codegraph` MCP 暴露，所有 lane **只读**（不写索引 DB）。
MCP 进程启动时自动 incremental sync。详见 [`docs/operations/index.md`](../../docs/operations/index.md)。

---

## 代码审查（code-review-deep）

| 场景 | 操作 | 用途 |
|------|------|------|
| 怀疑死代码 | `codegraph_dead_code[min_lines=5]` → `codegraph_callers` 验证 | 确认可安全删除 |
| 数据流追溯 | 对 diff 中可疑符号调 `codegraph_callers[symbol="<可疑符号>", depth=8]` | 完整上下游追溯 |
| PR 影响评估 | PR 删除公共函数/接口时，调 `codegraph_impact[depth=3]` | 评估下游破坏 |
| 符号定位 | diff 中符号不在当前文件时，调 `codegraph_goto_definition["符号名"]` | 定位定义位置 |
| 重名消歧 | 同名符号跨多个文件时，调 `codegraph_goto_definition["符号名", file_path="目标路径"]` | 精确确定修改目标 |
