# Skill Framework Protocols

本文件是共享的**最小协议层**；skill 不应在各自 `SKILL.md` 中重复长篇 schema。

## 1. Finding Schema

`finding` 表示一个离散问题。

| Field | Required | Notes |
|---|---|---|
| `finding_id` | yes | 稳定 slug，如 `<skill>-<type>-<N>` |
| `severity` | yes | `critical` / `major` / `minor` / `info` |
| `category` | yes | 领域分类 |
| `title` | yes | 单行标题 |
| `description` | yes | 问题与影响 |
| `location` | no | 文件、行、段落、元素 |
| `suggestion` | no | 推荐修法 |
| `effort` | no | `trivial` / `small` / `medium` / `large` |

## 2. Execution Item Schema

`execution item` 表示一次待执行动作。

| Field | Required | Notes |
|---|---|---|
| `item_id` | yes | 如 `exec-1` |
| `finding_ids` | yes | 关联哪些 finding |
| `action` | yes | 要做什么 |
| `scope` | yes | 影响范围 |
| `priority` | yes | `P0` / `P1` / `P2` |

## 3. Verification Result Schema

| Field | Required | Notes |
|---|---|---|
| `item_id` | yes | 对应 execution item |
| `status` | yes | `pass` / `fail` / `partial` / `skipped` |
| `evidence` | no | 命令输出、截图、人工检查等 |
| `regression` | no | 新引入问题 |

## 4. Runtime Protocol

所有 runtime / route 默认按以下四步闭环执行：

`讨论 -> 规划 -> 执行 -> 验证`

规则：

1. `讨论`: 先收敛对象、动作、约束、交付物和成功标准。
2. `规划`: 选择最窄 owner、必要 gate / overlay、最小路径和验证路径。
3. `执行`: 按最小 delta 执行，不扩大抽象或替代 domain owner。
4. `验证`: 用测试、命令、截图、产物或明确 blocker 关闭任务。

补充约束：

1. 该协议默认存在，不靠 `gsd`、`推进到底` 或 controller trigger 启动。
2. 只携带 **delta**，不要整轮重述。
3. 已执行项必须有验证状态。
4. 若出现 regression，作为下一轮 finding。
5. `runtime verification gate` iteration loop 只编排验收轮次，不替代 domain owner。

## 5. Stop Rules

满足任一即停止：

1. `critical` / `major` 已清空
2. 轮次预算耗尽
3. 连续一轮无新 delta，且已完成 false-convergence challenge
4. 用户要求停止
5. 剩余问题均为 `info`

## 6. Self-Audit 最小维度

完成一轮后只需复核：

1. 路由是否正确
2. gate 是否先于 owner
3. token 使用是否成比例
4. 是否只携带 delta
5. 是否有验证证据
6. 是否产生 framework drift 或边界漂移
