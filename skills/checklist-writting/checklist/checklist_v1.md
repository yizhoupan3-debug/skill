# <Checklist Title>

> 目标：<overall goal>
>
> 约束：<global constraints and non-goals>

## 当前状态快照

- <done / in-progress / blocked / open summary>
- <important baseline fact>
- <important known limitation>

---

## 并行任务总表

- 本轮可并行执行任务总数：**<N> 项**
- 并行原则：**默认把同级点视为并行；若存在依赖，必须显式写出**
- 串行原则：**只要属于同一个执行点，再长的串行链也写在同一点里**
- 完成定义：**<round-level done condition>**

| ID | 任务 | 主要目标 | 独占写入范围 |
|---|---|---|---|
| <T1> | <task title> | <goal> | <owned surface> |
| <T2> | <task title> | <goal> | <owned surface> |

---

## <T1>. <Task title>

### 当前状态
- <what is true now>

### 目标
- <ideal state for this point>

### 独占写入范围
- <owned files / surfaces / artifacts>

### 禁止越界
- <things this point must not modify>

### 串行步骤
1. <ordered step one>
2. <ordered step two>
3. <ordered step three>

### 退出条件
- <what must become true before this point may be declared finished>

### 停手条件
- <when to stop instead of pushing further>

### 交付物
- <artifact or outcome>
- <artifact or outcome>

### 验收标准
- <concrete completion condition>
- <concrete completion condition>

### 执行结果
- 状态：`[ ]` / `[~]` / `[x]`
- 结果：<not started / in progress / done / blocked / failed>
- 验证：<tool output, test, or evidence summary>

---

## <T2>. <Task title>

### 当前状态
- <what is true now>

### 目标
- <ideal state for this point>

### 独占写入范围
- <owned files / surfaces / artifacts>

### 禁止越界
- <things this point must not modify>

### 串行步骤
1. <ordered step one>
2. <ordered step two>

### 退出条件
- <what must become true before this point may be declared finished>

### 停手条件
- <when to stop instead of pushing further>

### 交付物
- <artifact or outcome>

### 验收标准
- <concrete completion condition>

### 执行结果
- 状态：`[ ]` / `[~]` / `[x]`
- 结果：<not started / in progress / done / blocked / failed>
- 验证：<tool output, test, or evidence summary>

---

## 不纳入本轮的内容
1. <explicit non-goal>
2. <explicit deferred item>

---

## 建议执行顺序

### 第一优先级
- <task or lane>

### 第二优先级
- <task or lane>

### 并行注意事项
- <lane dependency or read-only assumption>
- <boundary warning>

---

## 本轮总体验收线

当且仅当以下条件全部满足，可认为本轮完成：
1. <global acceptance condition>
2. <global acceptance condition>
3. <global acceptance condition>

## 当前完成统计模板

- 当前轮次总任务数：**<N>**
- 已完成：`<done>/<N>`
- 进行中：`<in-progress>/<N>`
- 未开始：`<not-started>/<N>`

可按以下格式持续更新：
- [ ] <T1> <task title>
- [~] <T2> <task title>
- [x] <T3> <task title>

## Agent Count

写完本文件后，必须明确告诉用户：
- 需要开启几个 agent = **并行点数量**
- 哪些点彼此并行
- 哪些步骤只是同一点内部的串行步骤，不要单独拆 agent
