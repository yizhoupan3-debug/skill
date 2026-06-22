---
last_verified: "2026-06-22"
depends_on:
  - ../spec.md
  - ../spec-cross-host.md
---

# ADR-008: 跨宿主一致性统一

## Status

Accepted (2026-06-22).

## Context

框架面向 4 个宿主（Claude Desktop、Cursor、Codex、OpenCode），但宿主间基础设施协议存在严重碎片化：

1. **4 套 session_key**：每宿主独立生成 session key，格式/过期策略/存储位置各不同，导致日志关联和调试困难。
2. **3 套 stdin 协议**：Claude 使用 JSON-RPC over stdio，Cursor 使用 LSP-inspired 协议，Codex 使用自有序列化格式，OpenCode 使用插件 hook 双通道。无统一 stdin 抽象。
3. **4 种文件锁机制**：各宿主对 SKILL_ROUTING_RUNTIME.json 等共享文件的锁策略不同（flock、lockfile、mutex、无锁），存在并发竞争风险。
4. **Bootstrap 缺失**：无标准化的框架启动流程，每宿主自行实现配置加载、依赖初始化、插件发现，导致启动行为不一致。

## Decision

实施四项统一（X1-X4）：

### X1. 统一 session key（`sessionKey` plant）

- 规范：UUIDv7（时间有序，便于排序和分片）
- 格式：`{host_id}-{uuid_v7}`（例：`claude-01J7XYZ...`）
- 生成时机：连接建立时由 Host Layer 生成
- 传播方式：自动注入所有跨层调用和日志 span
- 移除：各宿主独立 session key 实现

### X2. 统一 stdin 协议（`stdinTransport` plant）

- 规范：JSON-RPC 2.0 over stdio（已选型）
- 适配器模式：各宿主实现 `StdinTransport` trait，将自有协议映射到 JSON-RPC
- 框架内部：只消费 JSON-RPC，不感知宿主原始协议
- MCP 工具通道：保持独立，不作统一

### X3. 统一文件锁（`fileLock` plant）

- 规范：基于 `fs2` crate 的跨平台 `flock`（macOS/Linux）和 `LockFile`（Windows）
- 作用域：所有共享文件读写操作（SKILL_ROUTING_RUNTIME.json、GOAL_STATE.json、EVIDENCE_INDEX 等）
- 策略：读共享锁（shared）、写排他锁（exclusive），超时 5 秒
- 抽象：`LockableFile` wrapper 统一处理锁获取/释放/超时

### X4. 统一 bootstrap（`bootstrap` plant）

- 规范：`framework::bootstrap::BootstrapConfig` + `BootstrapStep` 枚举
- 启动步骤：① 配置加载 → ② 文件锁初始化 → ③ 插件发现 → ④ session 创建 → ⑤ skill 路由热加载 → ⑥ 工具注册 → ⑦ 就绪信号
- 每步骤可跳过/自定义（通过 `BootstrapConfig.skip_steps`）
- 钩子：`on_before_step` / `on_after_step` 支持宿主注入自定义初始化

## Consequences

- **优势**：
  - 日志/追踪可跨宿主关联（统一 session key）
  - 新增宿主只需实现 `StdinTransport` trait，无需重新实现协议层
  - 文件锁统一消除并发竞争，SKILL_ROUTING_RUNTIME.json 多进程安全
  - bootstrap 标准化减少宿主间行为漂移
- **代价**：
  - X2 要求现有宿主修改 stdin 适配层（Cursor LSP 协议需包装器）
  - X3 引入 `fs2` 依赖，Windows 上锁行为需验证
  - Bootstrap 步骤在轻量宿主（如 OpenCode 插件）上可能冗余，需 skip 支持
  - Trait 统一是必要但不充分的一步：即使接口一致，各宿主宿主环境差异仍可能引入行为差异
- **不纳入范围**：hook 事件集、权限策略、路由矩阵——这些属于宿主语义差异，由 Host Layer 和 Router Layer 处理。

## Related

- `docs/spec.md` §2 — 架构规约
- `docs/spec-cross-host.md` — 跨宿主规约
- `docs/cross-host-architecture.md` — 跨宿主架构实现
- `artifacts/current/roadmap-v8.md` §5 — 跨宿主基础设施 Wave
