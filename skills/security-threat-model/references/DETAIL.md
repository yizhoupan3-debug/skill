# Security Threat Model — Detailed Workflow

## Step 1. Define Scope

先明确范围：
- 整个仓库，还是子目录
- 运行时系统，还是只看某个服务
- 是否包含 CI / 构建 / 管理后台 / 运维工具

若用户只点名某个子系统，不要把整个仓库都算进来。

## Step 2. Extract the System Model

从仓库中梳理：
- 主要组件
- 数据存储
- 外部集成
- 运行方式：server、CLI、worker、library、desktop app、MCP server
- 主要入口：HTTP endpoint、upload surface、parser、job trigger、admin command

强制区分：
- 运行时代码
- CI / build / dev tooling
- test fixtures / examples / demo code

不要把测试夹具误判成真实攻击面。

## Step 3. Identify Trust Boundaries

重点找边界，而不是只列组件名。

常见边界：
- 浏览器 / 客户端 与服务端
- 公网请求 与内部服务
- worker 与数据库
- 管理接口 与普通用户接口
- 模型调用层 与外部工具 / MCP / 第三方 API
- 文件上传区 与执行或解析区

每个边界尽量说明：
- 协议
- 鉴权方式
- 输入验证
- 速率限制
- 是否加密

## Step 4. Enumerate Assets

资产不是"所有东西"，而是攻击成功后真正有价值的目标。

常见资产：
- 凭证、token、API keys
- PII、用户数据、商业数据
- 完整性关键状态
- 配置与模型参数
- 构建产物与发布流程
- 审计日志、监控数据
- 可用性关键资源

如果资产不明确，风险优先级基本无法排准。

## Step 5. Calibrate Attacker Capabilities

只讨论现实攻击者能力，不要无限上纲。

至少说明：
- 攻击者是未认证公网用户、普通租户、恶意内部用户，还是供应链攻击者
- 他能触达哪些接口或输入面
- 他不能做什么

明确 non-capabilities 很重要，否则容易把所有问题都判得太高。

## Step 6. Enumerate Threats as Abuse Paths

不要只列漏洞类别，优先写"攻击路径"。

好的 threat 形式：
- 攻击者通过什么入口
- 跨过什么边界
- 操纵什么对象
- 最终影响什么资产

优先关注：
- 敏感数据外泄
- 权限提升 / 横向越权
- 完整性破坏
- 关键服务 DoS
- 密钥或 token 泄露
- 供应链或构建链污染
- sandbox escape / tool abuse / prompt-to-action escalation

## Step 7. Prioritize Explicitly

每条 threat 至少给出：
- likelihood: low / medium / high
- impact: low / medium / high
- priority: low / medium / high / critical

并说明：
- 为什么这个攻击现实
- 什么假设影响了判断
- 现有控制是否已降低风险

## Step 8. Recommend Mitigations

缓解措施必须尽量落到具体位置。

好例子：
- "在上传网关强制 schema 和文件类型校验"
- "在管理路由增加 server-side authZ，而不是只靠前端隐藏"
- "对 webhook / callback 增加签名校验和幂等键"

坏例子：
- "加强安全"
- "验证输入"
- "增加监控"

优先区分：
- 已存在的控制
- 建议新增的控制
- 依赖用户确认后才能下结论的条件性建议

## Step 9. Validate Assumptions with the User

如果关键上下文缺失，会显著影响排序时，应暂停并向用户确认。

典型缺失点：
- 是否公网暴露
- 是否多租户
- 数据敏感级别
- authn / authz 模型
- 部署环境

若用户没有补充，也要把残留假设写进最终报告。

## Output Structure Template

```text
# Threat Model: <scope>

## Scope
## System Summary
## Trust Boundaries
## Assets
## Entry Points
## Attacker Capabilities and Assumptions
## Threats
## Existing Controls
## Recommended Mitigations
## Open Questions
```

Threats 部分建议按表或短条目列出：
- id
- abuse path
- impacted assets
- likelihood
- impact
- priority
- evidence / assumption
- mitigation

## Quality Gates

完成前检查：
- 是否覆盖了所有主要入口
- 是否每个关键 trust boundary 都映射到了威胁
- 是否区分了 runtime 与 CI / dev
- 是否把证据与推断分开写
- 是否写明关键假设
- 是否避免空泛 checklist 语言

## Codex Notes

- 高优先级是"读仓库并抽取系统模型"，不是先写结论
- 如果仓库信息不够，不要硬编系统结构
- 对 agent / MCP / tool-calling 系统，要额外留意：
  - prompt injection 到 action escalation
  - tool permission boundary
  - secret exposure through tool outputs
  - external connector trust assumptions
- 若用户要求输出文件，默认文件名可用 `<repo-or-dir-name>-threat-model.md`
