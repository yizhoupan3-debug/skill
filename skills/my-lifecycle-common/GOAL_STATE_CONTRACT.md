# GOAL_STATE 写入契约（my-lifecycle 公共规范）

## 双轨接口

| 宿主 | 接口 | 说明 |
|------|------|------|
| Claude Desktop | MCP `goal_state_manage` | 通过 MCP 工具层调用，参数：operation / task_id / goal / note |
| CLI / Cursor / Codex | `framework_goal_drive` stdio | 通过 `router-rs --stdio-json` 调用 |

Claude Desktop 环境下优先使用 MCP `goal_state_manage`。

**适用于**: `discussx` / `planx` / `implementx` / `verifyx` 全阶段。
**路径**: `artifacts/current/<task_id>/GOAL_STATE.json`

## 禁止直写（HARD）

- **不得**以 `Write` / `StrReplace` / 任何文件系统手段直接修改 `GOAL_STATE.json`。
- 所有变更必须经 **`framework_goal_drive`**（`router-rs --stdio-json`, op `framework_goal_drive`）执行，以保持 `TASK_LEDGER.jsonl` 与 Stop hydration 一致。
- Hook 层仅 **读取** GOAL；不写入 checkpoint。

## 全局字段

| 字段 | 值 | 备注 |
|------|------|------|
| `lifecycle_profile` | `my-light` | 全阶段固定 |
| `drive_until_done` | `false`（discuss/plan/verify）· `true`（implement） | implement 阶段启动时显式设为 `true` |

## 操作语义

| operation | 阶段 | 说明 |
|-----------|------|------|
| `start` | discussx / implementx | 启动任务；同时写入 `active_task.json`（及默认 `focus_task.json`，`set_focus: false` 可跳过）。discussx 阶段 `drive_until_done: false`；implementx 阶段 `drive_until_done: true`。goal contract 字段来自 `REQUIREMENTS.md` |
| `checkpoint` | discussx / planx / implementx | 进度记录，附 `note`；implement 中 wave 进度通过 checkpoint 记录 |
| `pause` | discussx / planx | 暂停，终端过渡用 |
| `resume` | implementx | 恢复已暂停任务；同时写入 `active_task.json` / `focus_task.json` |
| `complete` | verifyx | 设置 `status: completed`, `drive_until_done: false`；自动删除指向该 `task_id` 的 `active_task.json` / `focus_task.json` |
| `clear` | verifyx（备用） | 同 complete，中性化指针 |

## 指针管理

- `start` / `resume`：写入 `artifacts/current/active_task.json` 及默认 `focus_task.json`。
- `complete` / `clear`：**删除**指针文件（不留空对象 `{}`）。
- **禁止**手动将指针写为 `{}` 作占位。

## implementx 启动示例

**MCP（Claude Desktop）**:

```
goal_state_manage(operation="start", goal="<from GOAL_STATE>", note="drive_until_done=true; status=running; lifecycle_profile=my-light")
```

**Stdio（CLI / Cursor / Codex）**:

```bash
# status=running, drive_until_done=true, lifecycle_profile=my-light
printf '%s\n' '{"id":1,"op":"framework_goal_drive","payload":{"operation":"start","repo_root":"<repo>","task_id":"<task_id>","goal":"<from GOAL_STATE>","drive_until_done":true,"status":"running","lifecycle_profile":"my-light"}}' | router-rs --stdio-json
```

## 各阶段差异速览

| 阶段 | 允许的操作 | 特殊说明 |
|------|-----------|----------|
| discussx | start, checkpoint, pause, complete | `drive_until_done: false`；hook 层仅读取 |
| planx | start, checkpoint, pause | `drive_until_done: false` |
| implementx | start, resume, checkpoint, complete | `drive_until_done: true`；无 Stop `GOAL_CONTINUE` hook 注入 |
| verifyx | complete, clear | 终态操作；closeout 记录写入后再标记 complete |
