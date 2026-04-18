# Codex 双入口主清单

> 本文件现在是双入口 Rust 迁移的 compatibility-facing checklist，不再承担
> 仓库顶层执行主清单角色。当前主线执行由仓库根部的 `rust_checklist.md`
> 驱动；本文只保留双入口兼容视图下的收口状态，并统一服从
> `thin projection + Rust contract-first migration` 口径。

## 1. 本阶段目标

把方案收敛成一套可实施、可验证、可长期维护的 Codex-only 双入口主清单。

这一阶段真正的目标是：

1. 明确 framework core 是主控，不是 `codexcli`
2. 明确 `Codex Desktop + codexcli` 共用同一份 outer contract
3. 明确哪些现有 adapter / artifact 可以复用
4. 明确哪些旧 `aionrs` / `AionUI` 面需要降级
5. 明确 Rust 继续只做 contract / artifact / parity lane

## 2. 硬规则

- 不让 `codexcli` 接管 framework core
- 不为 Desktop 和 CLI 造两套 contract
- 不把宿主私有逻辑反向污染 framework core
- 不把 Rust lane 扩成 runtime rewrite
- 不再把 `aionrs` / `AionUI` 作为未来主线宿主

## 3. 当前基线

以下能力已经被代码证明存在，可直接作为 Codex-only 重构入口：

### 已验证的外层 contract 面

- [x] 已有统一的 `framework_profile` 契约
- [x] 已有 `approval_policy` / `loadout_policy` / `artifact_contract` / `host_capability_requirements`
- [x] 已支持 nested override merge
- [x] 已支持 host capability requirement resolution

### 已验证的可复用 adapter 面

- [x] 已有 `codex_common_adapter`
- [x] 已有 `codex_desktop_adapter`
- [x] 已有 `codex_cli_adapter`
- [x] `codex_desktop_host_adapter` 已作为 compatibility-only mirror alias 保留
- [x] 已有 `generic_host_adapter`
- [x] host adapters 共用同一份 `framework_profile`

### 已验证的 artifact / regression 面

- [x] 已能输出 adapter payload artifacts
- [x] 已有 Python -> Rust `framework_profile` handoff
- [x] Rust 已可编译 profile bundle
- [x] 已能输出 first-class Codex artifact files
- [x] `codex_dual_entry_parity_snapshot` 已是主回归基线

### 当前仍存在但应降级的 legacy 面

- [ ] `aionrs_companion_adapter`
- [ ] `aionui_host_adapter`
- [ ] 围绕 `aionrs` 的 compatibility matrix 主线叙事

## 4. 当前已经收口的核心面

- [x] `framework_profile` 仍可作为唯一真源
- [x] `Codex Desktop` 路径仍可作为正式宿主迁移起点
- [x] `generic_host_adapter` 仍可作为 common fallback 起点
- [x] Rust 已有第一条 profile compiler / projection 通道

## 5. 当前还没收口的核心空缺

- [x] alias retirement gate 已定义
- [x] checklist / outline 叙事已收口，已明确 `codex_desktop_host_adapter`
  只是 compatibility-only mirror
- [x] 旧清单不再把 `upgrade_compatibility_matrix` 写成主回归基线
- [~] 旧 `aionrs` 叙事在文档面已切到 compatibility / legacy debt 口径
  artifact / tests / code 侧仍待后续 lane 收口

## 6. 当前必须推进的五个核心任务

### Task 1: 冻结主控边界

- [x] 明确 framework core 是唯一主控
- [x] 明确 `codexcli` 是执行入口，不是真源
- [~] 把这一结论系统性写入 adapter / artifact / test 叙事
  文档 / contract 叙事已完成；adapter / tests 仍待后续 lane 收口

### Task 2: 建立 `codex_common_adapter`

- [x] Desktop / CLI 共享的 session / artifact / memory / MCP / policy 编译面已收敛到同一层
- [x] 共用 bridge / contract emission 已从 host-specific adapter 中抽离
- [x] common adapter 不承接框架核心治理

### Task 3: 建立双宿主适配约束

- [x] `codex_desktop_adapter` 已是正式 desktop adapter
- [x] `codex_cli_adapter` 已建成 formal headless entrypoint
- [x] `codex_desktop_host_adapter` 已降级为 compatibility-only mirror alias
- [ ] 保持 Desktop / CLI 共用同一 artifact / memory / approval 真源
- [ ] 保留 generic fallback 语义，但不再让它替代正式双入口设计

### Task 4: 把回归策略工程化

- [x] 已用 Desktop / CLI parity snapshot 替换旧 `aionrs` compatibility 主线
- [x] 已建立 shared artifact layout baseline
- [x] 已建立双入口回归而不是单宿主回归
- [~] 继续推动下游从 `upgrade_compatibility_matrix` 迁到 parity-snapshot-first
  文档口径已完成；artifact / tests / scripts 仍待后续 lane 收口

### Task 5: 重新定义 Rust 边界

- [x] 保持 Rust 继续消费外层 `framework_profile`
- [x] Rust 已输出 Desktop / CLI 共用 contract fragments
- [x] Rust 已输出 parity snapshot / regression helper
- [x] 禁止把 Rust lane 推成 runtime rewrite
- [ ] 继续压缩 bundle-only / compatibility-first 残留叙事

## 7. 当前 Rust 化状态

### 已落地

- [x] Rust `router-rs` 已支持加载 `framework_profile`
- [x] Rust `router-rs` 已支持输出 `profile bundle`
- [x] Python `RustRouteAdapter.compile_profile_bundle(...)` 已打通调用链
- [x] Python artifact emitter 已支持 `--include-rust-bundle`
- [x] Rust 已支持 first-class Codex artifact emission
- [x] Rust 已支持 parity snapshot JSON emission
- [x] Python artifact emitter 已外显 `rust_python_artifact_parity_report`，用于持续校验 Python / Rust 一等 artifact 对齐

### 现在要改写的定位

- [x] 已从旧的 companion projection 叙事，改成 Codex shared contract emission
- [x] 已从旧的 upgrade compatibility 叙事，改成 Desktop / CLI parity snapshot
- [x] 已清掉残留文档里的 compatibility-first 表述

### 当前不做

- [x] 不做 runtime kernel 大爆炸 rewrite
- [x] 不让 Rust 接管宿主私有控制面
- [x] 不让 Rust 成为“CLI 主控化”的技术借口

### 下一轮要做

- [ ] 继续把路由/编译权威面向 Rust 收口，减少 Python / Rust 双维护
- [ ] 在不改 framework truth 的前提下，实现更强的 runtime control plane
  语义
- [ ] 用 DeerFlow 2.0 runtime benchmark 校准 run-manager / stream bridge /
  unified persistence seam 的设计
- [ ] 保持 runtime 深化是增量式内核收口，而不是一次性重写

## 8. 当前阶段验收标准

- [x] 能清楚回答“主控不是 `codexcli`”
- [x] 同一份 `framework_profile` 可以同时被 `Codex Desktop` 和 `codexcli` 正式消费
- [x] Desktop / CLI 共享同一份 artifact / memory / session continuity contract
- [x] `codex_dual_entry_parity_snapshot` 已是主 regression baseline
- [x] `codex_desktop_host_adapter` 已只是 compatibility-only mirror alias
- [~] 旧 `aionrs` / `AionUI` 路线已在文档面明确降级为 legacy debt
  代码 / tests / artifacts 仍待后续 lane 收口
- [x] Rust lane 的输出口径已对准 Codex-only 双入口 compatibility view，
  并受 `thin projection + Rust contract-first migration` 主线约束

## 9. 明确不做的事

- [x] 不把 `codexcli` 提升为框架真源
- [x] 不把 Desktop 降级成 CLI 的 UI 壳
- [x] 不在这一阶段继续扩 `aionrs` / `AionUI` 宿主能力
- [x] 不在这一阶段直接进入底层 runtime 魔改

## 10. 这一阶段的最终判断

本阶段唯一推荐路线：

**framework core + codex common adapter + codex_desktop_adapter + codex_cli_adapter。**

不推荐路线：

**`codexcli` 变主控，Desktop 退化为 CLI 壳。**
