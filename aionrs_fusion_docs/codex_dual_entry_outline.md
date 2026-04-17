# Codex 双入口总蓝图

> 本文件是当前方案的总蓝图。它覆盖已经落地的战略前提：
> `Codex Desktop + codexcli` 双入口，framework core 保持单真源；
> `codex_dual_entry_parity_snapshot` 是主回归基线，desktop host alias 只保留兼容镜像角色。

## 1. 结论先行

新的正确路线不是继续“深度吸收进 `aionrs`”，而是改成：

- framework core = 唯一主控
- `Codex Desktop` = 交互宿主
- `codexcli` = headless / automation / batch 宿主
- shared contract = Desktop / CLI 共用的 outer contract
- `codex_dual_entry_parity_snapshot` = 主回归 artifact

一句话判断：

**双入口，单真源。**

## 2. 直接回答最关键的问题

### 2.1 主控是不是应该变成 `codexcli`？

不应该。

`codexcli` 应该是执行入口，不是框架真源。否则会出现三个问题：

1. Desktop 会退化成 CLI 的壳
2. thread / artifact / automation / memory 语义会被 CLI 私有语义反向绑定
3. 将来再扩别的 Codex 宿主时，仍然会遇到同样的宿主耦合问题

所以正确关系是：

- framework core 负责主控
- `codexcli` 负责 headless 执行
- `Codex Desktop` 负责交互执行

### 2.2 双入口下什么应该共用，什么应该分开？

必须共用：

- `framework_profile`
- routing / orchestration / artifact / memory / approval 真源
- tool policy / loadout policy / session continuity contract
- parity snapshots / regression baseline

应该分开：

- Desktop 的 thread / UI / automation bridge
- CLI 的 batch / cron / CI / non-interactive entrypoints
- 宿主私有 metadata 和轻量包装

### 2.3 旧的 `aionrs` / `AionUI` 资产怎么处理？

结论很明确：

- 不再作为目标架构继续扩
- 不必立刻暴力删除
- 先降级为 legacy migration debt
- 后续在 Codex-only 迁移完成后再决定收缩或移除

## 3. 推荐架构

### 3.1 Layer 0: Framework Core

这一层继续独立于任何具体宿主。

应该保留在这里的能力：

- 路由与编排内核
- 多 agent 状态机
- artifact contract
- memory 真源与记忆策略
- skill / rules 编译
- task / session / takeover / checkpoint 语义
- 宿主无关的 tool policy、approval policy、loadout policy

这一层的输出继续是统一的 `framework_profile`。

### 3.2 Layer 1: Codex Common Adapter

这是已经成立的共享投影层。

它负责把 `framework_profile` 编译成 Codex 平台共用语义，不区分先 Desktop 还是先 CLI。

它应该承接：

- session / policy / artifact / memory / MCP 的共通投影
- shared bridge / artifact layout
- Desktop / CLI 都要用到的 contract emission
- parity snapshot 的共用输入

它不应该承接：

- Desktop 私有 UI 交互细节
- CLI 私有启动参数细节
- framework core 的治理逻辑

### 3.3 Layer 2: Codex Desktop Adapter

这一层服务于交互宿主。

负责：

- 线程与交互态绑定
- artifact contract 的桌面端消费
- automation bridge / memory bridge / MCP 消费
- Desktop 私有的 session metadata 和 host capabilities

当前命名约束：

- `codex_desktop_adapter` 是 canonical desktop entrypoint
- `codex_desktop_host_adapter` 只保留为 compatibility-only mirror alias
- alias 只用于兼容下游消费，不再承载新语义增长

### 3.4 Layer 2: Codex CLI Adapter

这一层服务于 headless 宿主。

负责：

- batch / cron / CI / non-interactive run
- headless artifact emission
- CLI 侧 workspace bootstrap 和 execution envelope
- 与 Desktop 共用 contract，但保留 CLI 私有 entrypoint 语义

### 3.5 Layer 3: Legacy Migration Zone

这一层不再是目标架构，只是迁移债务区。

当前应被降级处理的资产：

- `aionrs_companion_adapter`
- `aionui_host_adapter`
- 围绕 `aionrs` 的 compatibility / upgrade / rollback 叙事
- 任何仍把 `upgrade_compatibility_matrix` 写成主回归基线的文档

原则：

- 不新增投资
- 不再把它们写成未来主线
- 在 Codex-only 主线稳定后再决定收缩或移除

## 4. 边界清单

### 4.1 必须留在 framework core 的能力

- 框架总路由器
- orchestrator / checkpoint / continuation contract
- artifact / memory / approval / loadout 真源
- 跨宿主 session continuity
- Desktop / CLI parity 定义

### 4.2 只能放在宿主 adapter 的能力

- Desktop 的 thread / UI / automation semantics
- CLI 的 batch / cron / non-interactive semantics
- 宿主私有 capability discovery
- 宿主层 metadata 注入

### 4.3 明确不该发生的退化

- 不让 `codexcli` 反向接管 framework core
- 不为 Desktop 和 CLI 创造两套 contract
- 不把双入口方案重新退化成“单入口 CLI + 一个薄 UI 壳”

## 5. 当前代码基线判断

截至现在，可复用的基线有：

- [x] `framework_profile` 已存在
- [x] host-neutral contract 语义已存在
- [x] `codex_common_adapter` 已存在
- [x] `codex_desktop_adapter` 已是正式 desktop surface
- [x] `codex_desktop_host_adapter` 仅保留 compatibility-only mirror alias
- [x] `codex_cli_adapter` 已存在
- [x] `generic_host_adapter` 可作为 common fallback 迁移起点
- [x] Rust `router-rs` 已具备 contract compiler / projection 基线
- [x] `codex_dual_entry_parity_snapshot` 已是主回归基线

当前与新战略不一致的部分有：

- [ ] `aionrs_companion_adapter` 仍在当前代码面存在
- [ ] `aionui_host_adapter` 仍在当前代码面存在
- [ ] 文档与 artifact 仍残留旧的 `aionrs` 主线叙事
- [ ] 仍有文档把 `codex_desktop_host_adapter` 写成正式 desktop adapter
- [ ] 仍有文档把 `upgrade_compatibility_matrix` 写成主回归基线

## 6. Rust 边界

Rust 继续保留，但边界要改写成：

- contract emission
- artifact layout parity
- snapshot / regression helpers
- Desktop / CLI shared projection

Rust 当前已对齐到：

- first-class Codex artifact emission
- parity snapshot JSON 输出
- parity-first regression story

Rust 不做：

- runtime kernel rewrite
- Desktop / CLI 宿主私有协议魔改
- 任何“把主控迁到 CLI”的下沉重构

## 7. 推荐落地顺序

### Phase A: 冻结主控边界

目标：

- 明确 framework core 仍是主控
- 明确 `codexcli` 只是执行入口

### Phase B: 稳定 `codex_common_adapter`

目标：

- 保持 Desktop / CLI 共享 contract 编译面稳定
- 减少残留的 host-specific duplication

### Phase C: Desktop / CLI 双适配收口

目标：

- 保持 `codex_desktop_adapter` 作为 canonical desktop entrypoint
- 保持 `codex_cli_adapter` 作为 formal headless entrypoint
- 把 `codex_desktop_host_adapter` 约束为 compatibility-only mirror alias
- 明确双入口 parity contract

### Phase D: Rust parity / artifact lane

目标：

- 继续输出 Desktop / CLI 共用 contract fragments
- 保持 `codex_dual_entry_parity_snapshot` 作为主 regression baseline
- 把 `upgrade_compatibility_matrix` 压到 secondary inventory / smoke 角色

### Phase E: Legacy deprecation

目标：

- 把旧 `aionrs` / `AionUI` 叙事降级到 migration zone
- 停止继续把它们当主线扩展

## 8. 验收标准

如果未来重构做对了，应该满足：

- 同一份 `framework_profile` 可以同时被 `Codex Desktop` 和 `codexcli` 消费
- framework core 始终是唯一主控
- `codexcli` 不是框架真源
- Desktop / CLI 共享 artifact / memory / session continuity contract
- `codex_desktop_adapter` 是正式 desktop surface，`codex_desktop_host_adapter` 只是 compatibility-only mirror
- `codex_dual_entry_parity_snapshot` 是主回归基线，`upgrade_compatibility_matrix` 不是主线
- Rust lane 只做 contract / parity / regression，不做 runtime rewrite

## 9. 最终判断

现在唯一合理的路线不是继续“aionrs 深度融合”，而是：

**framework core + codex common adapter + codex_desktop_adapter + codex_cli_adapter。**
