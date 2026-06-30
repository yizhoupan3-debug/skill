<!--
  Output Contract Template — Structured Task Output

  Copy this section into your SKILL.md to declare what TASK_OUTPUT.json fields
  your skill produces and how downstream tasks should consume them.
-->

## Output Contract

本 Skill 的标准输出格式遵循 `task-output-v1` schema，所有产出物写入 `artifacts/current/<task_id>/TASK_OUTPUT.json`。

### 输出字段

| 字段 | 类型 | 说明 | 必填 |
|------|------|------|------|
| `outputs.changed_files` | `Vec<String>` | 本 task 修改的文件路径 | 是 |
| `outputs.commands_run` | `Vec<String>` | 执行的验证命令摘要 | 是 |
| `outputs.verification_status` | `"passed"` / `"failed"` / `"partial"` / `"not_run"` | 最终验证状态 | 是 |
| `outputs.summary` | `String` | 本 task 完成内容的一句话总结 | 是 |
| `closeout` | `Object` | 嵌入的 closeout record（closeout-record-v1） | 完成时 |
| `consumed_inputs` | `Vec<ConsumedInput>` | 从前置 task 拉取的输入 | 可选 |

### 消费方式

后续 task 通过以下任一方式消费本 task 的输出：

1. **自动传递**：使用 `task_chain_advance` 时，当前 task 的输出自动写入下一 task 的 `consumed_inputs`
2. **显式拉取**：使用 `task_output_pull(current_task_id, source_task_id, ["changed_files"])`

### 示例 closeout 记录

```json
{
  "schema_version": "closeout-record-v1",
  "task_id": "<your-task-id>",
  "summary": "描述了本次完成的内容",
  "verification_status": "passed",
  "changed_files": ["src/main.rs"],
  "commands_run": [
    {"command": "cargo test", "exit_code": 0}
  ]
}
```

### 创建完整输出（含 closeout）

```json
// 调用 task_output_write
{
  "task_id": "<your-task-id>",
  "status": "completed",
  "summary": "fixed deprecation warnings",
  "verification_status": "passed",
  "changed_files": ["src/lib.rs"],
  "commands_run": ["cargo test", "cargo clippy"],
  "closeout": {
    "schema_version": "closeout-record-v1",
    "task_id": "<your-task-id>",
    "summary": "...",
    "verification_status": "passed",
    "changed_files": [...],
    "commands_run": [...],
    "blockers": [],
    "risks": []
  }
}
```

### 验证

使用 `task_output_validate(task_id: "<...>")` 检查 TASK_OUTPUT.json 的字段完整性。
