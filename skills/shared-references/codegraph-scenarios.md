# CodeGraph 操作式指南（共享引用）

CodeGraph 八工具经 `mcp-codegraph` MCP 暴露，所有 lane **只读**（不写索引 DB）。
MCP 进程启动时自动 incremental sync。详见 [`docs/operations/index.md`](../../docs/operations/index.md)。

---

## 规划阶段（planx）

| 时机 | 操作 | 产出 |
|------|------|------|
| **拆 lane 前** | 对候选核心符号调 `codegraph_impact["符号名", depth=2]` | 确认 lane scope 间调用链断开、无隐匿依赖，写入 PLAN_TRACE.md |
| **模块归属评估** | 调 `codegraph_callers["符号名", depth=1]` | 确认待改函数/类的调用者模块归属，避免归类错误 |
| **索引验证** | 计划产出前调 `codegraph_status` | 确认索引覆盖所有待改文件。索引不完整时在 plan 中标注风险 |

---

## 实施阶段（implementx）

以下高风险操作 **必须** 在操作前调用 codegraph：

| 操作 | 必调工具 | 产出 |
|------|---------|------|
| 删除/重命名公共符号 | `codegraph_callers["符号名", depth=1]` | 确认无遗漏调用者 |
| 重构核心函数/类型 | `codegraph_impact["符号名", depth=2]` | 影响半径报告，写入 lane scope 的调用链清单 |
| 跨模块修改 | `codegraph_callees["符号名", depth=2]` | 确认下游模块无破坏 |
| `scope_paths` 含 `core/` 或 `tools/` | `codegraph_impact["符号名", depth=3]` | 完整影响面报告 |
| 确认待改符号定义位置 | `codegraph_goto_definition["符号名"]` | 避免重名符号误改，确认目标文件

**subagent lane prompt 模板** 中追加：
> 在修改 `target_symbol` 前，调 `codegraph_impact["target_symbol"]` 获取影响半径，将结果写入 lane-notes。

---

## 验证阶段（verifyx）

| 时机 | 操作 | 产出 |
|------|------|------|
| **索引新鲜度** | 调 `codegraph_status`，确认 `indexed_at` 在本次 session 内更新过 | evidence 中的索引健康记录 |
| **符号路径校验** | 对 ROADMAP 中标记的核心 symbol，调 `codegraph_goto_definition["符号名"]` 确认路径无漂移 | evidence 条目 |
| **breaking change** | 对已修改的公共 API 符号，调 `codegraph_callers[symbol="<已修改的公共符号>", depth=3]` | 确认无意外 breaking change |
| **死代码确认** | （可选）调 `codegraph_dead_code[language=rust, min_lines=10]` | 确认无新增 orphan 函数 |

结果写入 `EVIDENCE_INDEX` 的 `artifacts[]` 行（command_preview + exit_code）。

---

## 代码审查（code-review-deep）

| 场景 | 操作 | 用途 |
|------|------|------|
| 怀疑死代码 | `codegraph_dead_code[min_lines=5]` → `codegraph_callers` 验证 | 确认可安全删除 |
| 数据流追溯 | 对 diff 中可疑符号调 `codegraph_callers[symbol="<可疑符号>", depth=8]` | 完整上下游追溯 |
| PR 影响评估 | PR 删除公共函数/接口时，调 `codegraph_impact[depth=3]` | 评估下游破坏 |
| 符号定位 | diff 中符号不在当前文件时，调 `codegraph_goto_definition["符号名"]` | 定位定义位置 |
| 重名消歧 | 同名符号跨多个文件时，调 `codegraph_goto_definition["符号名", file_path="目标路径"]` | 精确确定修改目标 |
