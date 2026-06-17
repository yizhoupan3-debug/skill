# CodeGraph 场景表（共享引用）

CodeGraph 七工具经 `mcp-codegraph` MCP 暴露，所有 lane **只读**（不写索引 DB）。
MCP 进程启动时自动 incremental sync。详见 [`docs/operations/b10-codegraph.md`](../../docs/operations/b10-codegraph.md)。

| 场景 | 工具 |
|------|------|
| 索引就绪 | `codegraph_status` |
| 符号定位 | `codegraph_search` / `codegraph_node` |
| 调用链 | `codegraph_callers` / `codegraph_callees` |
| 影响半径 | `codegraph_impact` |
| 死代码检测 | `codegraph_dead_code` |
