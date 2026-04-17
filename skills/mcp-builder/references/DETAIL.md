# MCP Builder — Detailed Guidance

## Recommended Workflow Details

### 1. Understand the Target System

先搞清楚：
- 这个外部系统提供什么能力
- 常见使用场景是什么
- 哪些操作是高频只读，哪些是危险写入
- 是否有分页、限流、认证、幂等要求

如果连目标 API / 数据模型都没搞清楚，不要急着写工具。

### 2. Define the Tool Surface

先列候选操作，再做筛选。

建议分类：
- read-only discovery tools
- entity detail tools
- mutation tools
- workflow tools
- admin / destructive tools

筛选时优先：
- 高频
- 边界清楚
- 参数稳定
- 返回值可结构化

### 3. Choose the Right Transport

常见选择：

- `stdio`
  适合本地工具、本地开发、CLI 集成、桌面场景

- Streamable HTTP / HTTP
  适合远程服务、团队共享、可部署 server

简单规则：
- 本地优先 `stdio`
- 远程多实例服务优先 HTTP

不要为了"看起来现代"盲目上复杂 transport。

### 4. Design Tool Contracts

每个工具至少明确：
- tool name
- description
- input schema
- output schema
- side effect
- auth requirement

输入 schema 要做到：
- 类型明确
- 约束明确
- 字段命名清楚
- 必要时带例子

输出 schema 要做到：
- 尽量结构化
- 字段稳定
- 支持 agent 继续链式调用

### 5. Build Shared Infrastructure

不要在每个工具里重复写这些逻辑：
- API client
- auth handling
- pagination
- retries
- error mapping
- response normalization

这些应集中到公共层。

### 6. Implement Tools Conservatively

实现时优先：
- 类型完整
- 明确只读 / 写入属性
- 对危险工具加防护
- 正确映射上游错误
- 返回结构化数据和简短摘要

如果 SDK 支持，优先使用：
- `outputSchema`
- `structuredContent`
- annotations / hints

### 7. Review for Agent Usability

写完后不要只问"能不能跑"，还要问：
- agent 能否从名称看出它的用途
- description 是否够具体
- 参数是否容易填对
- 返回结果是否适合下一步工具调用
- 遇错时是否容易恢复

## Tool Design Rules

### Naming

- 用一致前缀
- 动词开头
- 一眼能看出对象和动作

推荐模式：
- `<domain>_list_<entity>`
- `<domain>_get_<entity>`
- `<domain>_create_<entity>`
- `<domain>_update_<entity>`

### Parameters

- 尽量避免含糊参数名
- 能枚举就不要 free-form
- 能过滤就不要默认返回全量
- 对大列表操作必须提供分页或限制

### Output

优先返回：
- 结构化对象
- 关键信息摘要
- 可继续调用的标识符

避免：
- 冗长无结构日志
- 原始大对象直接倾倒
- 没有分页的海量列表

### Safety Annotations

对每个工具明确：
- 是否只读
- 是否 destructive
- 是否 idempotent
- 是否会触达外部世界

## Language Guidance

### TypeScript

优先级通常更高，适合：
- 新建项目
- 需要强 schema
- 需要更强类型约束
- 团队主要用 Node / TS

常见搭配：
- MCP TypeScript SDK
- Zod

### Python

适合：
- 目标系统 Python 生态强
- 需要快速接已有 Python SDK
- 数据处理和自动化脚本较多

常见搭配：
- FastMCP / Python MCP SDK
- Pydantic

## Evaluation Guidance

MCP server 做完后，最好评估"agent 能不能真用起来"。

评测问题应满足：
- 真实
- 独立
- 需要多步调用
- 只读优先
- 答案可验证
- 不易随时间漂移

不要只测"单工具单调用"，那样很难反映真实可用性。

## Failure Modes

常见失败模式：
- 工具名太泛，模型找不到
- 一个工具承担太多职责
- 参数设计模糊
- 返回体过大，导致上下文污染
- 错误信息无恢复指引
- 只照搬 API endpoint，没有考虑 agent 工作流
- 没有分页，列表工具直接淹没上下文
- 写入工具缺少安全边界
