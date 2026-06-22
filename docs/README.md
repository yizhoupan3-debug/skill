# 文档体系

## 文档地图

| 类别 | 文档 | 说明 |
|------|------|------|
| **架构** | [adr/010-ideal-architecture-v10.md](adr/010-ideal-architecture-v10.md) | 当前权威架构规约：六层模型、DAG、L4 拆分计划 |
| **宿主** | [hosts/_common.md](hosts/_common.md) | 四宿主共享内容（身份、路由、Python、进程管理） |
| | [hosts/hook-hosts.md](hosts/hook-hosts.md) | Hook 宿主手册（Claude/Cursor/Codex 事件矩阵、锁序） |
| | [hosts/opencode.md](hosts/opencode.md) | OpenCode 宿主操作手册 |
| **运维** | [operations/index.md](operations/index.md) | 运维中枢：安装/升级、模块操作、状态管理、工具安装、安全策略 |
| **科研** | [research-harness.md](research-harness.md) | 科研 Harness 系统总览、研究工作区、日志体系 |
| | [research/routing-contracts.md](research/routing-contracts.md) | research-discovery / execution 路由契约 |

## 按角色阅读

| 角色 | 推荐阅读顺序 |
|------|-------------|
| **框架开发者** | 本索引 → [adr/010-ideal-architecture-v10.md](adr/010-ideal-architecture-v10.md)（架构） → [hosts/hook-hosts.md](hosts/hook-hosts.md)（宿主） → [../AGENTS.md](../AGENTS.md)（策略） |
| **Skill 作者** | [../README.md](../README.md) §系统包含内容 → skills 目录 → [../CONTRIBUTING.md](../CONTRIBUTING.md) §Skill 贡献 |
| **普通用户** | [../README.md](../README.md) → [operations/index.md](operations/index.md)（安装/升级） → [hosts/hook-hosts.md](hosts/hook-hosts.md) |
| **宿主实现者** | [hosts/_common.md](hosts/_common.md) → [hosts/hook-hosts.md](hosts/hook-hosts.md) → [hosts/opencode.md](hosts/opencode.md) |
| **顶级策略** | [AGENTS.md](../AGENTS.md) | 跨宿主代理策略（生命周期、语言、CodeGraph、行为差异） |

## 已合并/删除文档记录

以下文件内容已在本次重构中合并：

| 删前路径 | 合并到 | 说明 |
|----------|--------|------|
| `references/hook_lock_order.md` | `hosts/hook-hosts.md` §Hook Lock Order | 锁序技术参考 |
| `references/review-protocol.md` | `spec.md` §Review 通用协议 | Review 幻觉分类与约束 |
| `operations/getting-started.md` | `operations/index.md` | 安装/升级/跨项目引导（所有独特点已内联） |
| `operations/state-management.md` | `operations/index.md` §状态管理运维 | TTL、GOAL_STATE、TASK_STATE |
| `references/codegraph-rules.md` | `AGENTS.md` §CodeGraph 自动触发规则 | 合回 AGENTS.md 恢复连续性 |
| `operations/backup-restore.md` | `operations/index.md` §备份、恢复与卸载 | 备份优先级、恢复流程 |
| `operations/security.md` | `operations/index.md` §安全运维 | SSRF、MCP 策略、沙箱 |
| `spec/research-harness.md` | `research-harness.md`（同级） | 提升到 docs/ 根级 |
| `spec.md` | `adr/010-ideal-architecture-v10.md` + `AGENTS.md` + `CONTRIBUTING.md` | 架构部分→ADR-010，Review 协议→AGENTS.md，契约漂移规则→CONTRIBUTING.md |
