---
allowed_tools:
- shell
- git
approval_required_tools: []
description: Guaranteed entry point for Goal management — create, checkpoint, amend, resume goals.
metadata:
  platforms:
  - supported
  tags:
  - goal
  - goalx
  - goal-management
  - framework-command
  version: '1.0.0'
name: goalx
network_access: conditional
risk: low
routing_gate: none
routing_layer: L1
routing_owner: owner
routing_priority: P1
session_start: n/a
short_description: Guaranteed entry point for Goal management with goal_state_manage.
source: runtime
trigger_hints:
- /goalx
- goalx
- goal:
- Goal:
- 目标：
- 目标:
- goal management
- goal 管理
- goal 模式
- 进入 goal
---

# goalx

`goalx` 是 Goal 管理的可靠快捷入口。与其他 framework_command 不同，goalx 保证即使路由系统异常也能通过 `goal_state_manage` MCP 工具直接进入 goal 模式。

## When to use

- 用户显式输入 `/goalx` 或口语 `goalx`
- 用户想要创建一个新的 Goal 契约（Goal / Non-goals / Done when / Validation commands）
- 用户要对已有 goal 做 checkpoint、amend、pause、resume、complete 操作
- 用户说"进入 goal 模式"、"创建一个 goal"、"设置一个目标"
- 当前 session 的 GOAL_STATE.json stale 或丢失，需要重新创建
- 路由系统的 goal_auto_detect 因复杂度不足未触发，但用户明确想要 goal

## Do not use

- 用户只是描述一个具体任务（用普通 task 流程即可）
- 用户提问或闲聊（直接回答，不需要 goal）
- 用户要的是 review/closeout 而非 goal 管理

## Default contract

1. **先读当前 goal 状态**：调 `goal_state_read` 检查是否有活跃的 GOAL_STATE
2. **如果有活跃 goal**：
   - 显示当前 goal 状态（goal / status / done_when / checkpoint）
   - 询问用户要做什么：continue / checkpoint / amend / pause / resume / complete / clear
   - 调 `goal_state_manage` 执行对应操作
3. **如果无活跃 goal 或 stale**：
   - 引导用户定义新 goal：goal / non-goals / done when items / validation commands
   - 调 `goal_state_manage` operation=start 创建新 goal
4. **完成后总结**：告知当前 goal 状态和后续可用操作

## Usage

```text
/goalx                              # 进入 goal 管理（检查状态）
/goalx create                       # 创建新 goal
/goalx amend                        # 修改当前 goal
/goalx checkpoint                   # 记录检查点
/goalx pause|resume|complete|clear  # 状态管理
```

## Implementation notes

- `goal_state_manage` MCP 工具的 operation 参数支持：start / checkpoint / pause / resume / complete / clear / block / amend
- `task_id` 为必填参数
- Goal state 仅作用于当前对话 session，不做跨对话持久化

## set_goal 协议（自动路由触发用）

当系统注入 `[Goal Auto-Detect]` 上下文时，立即执行 set_goal 流程。这是一个主动创建契约的过程，**不是复述用户原话**。

### 流程

**① 调研分析**：阅读相关代码/文档/架构，必要时用 WebSearch 搜索外部信息，充分理解任务范围、约束和上下文。不要跳过调研直接套模板。

**② 提炼结构化 Goal 契约**：
- **Goal**（核心目标）：一句话描述"做什么、为什么、到什么程度"。从用户散乱描述中提取精华。
- **Non-goals**（非目标）：明确排除什么。用户没说的要推断——如果某个相关方向明显超出范围，主动列出来。
- **Done when**（完成条件）：可验证的列表。每项应是可测试/可观察的。≥3 项。
- **Validation commands**（验证命令）：至少一条可执行的测试/检查命令。

**③ 创建 Goal**：调用 `goal_state_manage`：
```
goal_state_manage(
  operation="start",
  task_id="<kebab-case-task-id>",
  goal="<核心目标>",
  done_when=["<条件1>", "<条件2>", ...],
  non_goals=["<非目标1>", ...],
  validation_commands=["<命令1>", ...]
)
```

**④ 确认**：创建后回复用户当前 Goal 状态。

### 规则

- **set_goal ≠ 复述**：goal 是分析和提炼后的产物。用户说"改一下这个模块的性能"，你可能提炼成"优化 core/foo 模块：缓存热路径 + 减少内存分配 + 关键路径 benchmark ≥20% 提升"。
- **允许调研**：对不熟悉的领域，先读代码、搜文档、查资料，再设定 goal。
- **不做不可验证的承诺**：goal 和 done_when 必须是可测试/可观察的。
- **不跨 session**：当前 session 创建的 goal 仅当前 session 有效。
